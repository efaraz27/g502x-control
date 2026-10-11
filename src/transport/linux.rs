use super::Endpoint;
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};

pub fn discover() -> Result<Vec<Endpoint>> {
    let mut found = Vec::new();
    for entry in fs::read_dir("/sys/class/hidraw")? {
        let entry = entry?;
        let dev = entry.path().join("device");
        let uevent = fs::read_to_string(dev.join("uevent"))?;
        let field = |key: &str| {
            uevent
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .unwrap_or("")
                .to_owned()
        };
        let ids = field("HID_ID=");
        let ids: Vec<_> = ids.split(':').collect();
        if ids.len() != 3 || u32::from_str_radix(ids[1], 16).ok() != Some(0x046d) {
            continue;
        }
        let product = u16::from_str_radix(ids[2], 16)?;
        if ![0xc095, 0xc547, 0x4099].contains(&product) {
            continue;
        }
        let descriptor = fs::read(dev.join("report_descriptor")).unwrap_or_default();
        if !descriptor.windows(2).any(|b| b == [0x85, 0x11]) {
            continue;
        }
        found.push(Endpoint {
            path: Path::new("/dev").join(entry.file_name()),
            name: field("HID_NAME="),
            serial: field("HID_UNIQ="),
            product,
            index: if product == 0xc095 { 0xff } else { 1 },
        });
    }
    found.sort_by_key(|e| {
        (
            match e.product {
                0xc095 => 0,
                0x4099 => 1,
                _ => 2,
            },
            e.path.clone(),
        )
    });
    Ok(found)
}

pub struct RawTransport(File);
impl RawTransport {
    pub fn open(endpoint: &Endpoint) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(&endpoint.path)
            .with_context(|| {
                format!(
                    "Cannot open {}. Install the supplied udev rule if permission is denied.",
                    endpoint.path.display()
                )
            })?;
        Ok(Self(file))
    }
    pub fn write(&mut self, packet: &[u8]) -> Result<usize> {
        Ok(self.0.write(packet)?)
    }
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let mut pollfd = libc::pollfd {
                fd: self.0.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll receives one valid pollfd for the lifetime of this call.
            let rc = unsafe { libc::poll(&mut pollfd, 1, remaining.as_millis().min(1200) as i32) };
            if rc < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted && !remaining.is_zero() {
                    continue;
                }
                return Err(e.into());
            }
            ensure!(
                pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) == 0,
                "Mouse disconnected"
            );
            if rc == 0 {
                return Ok(0);
            }
            match self.0.read(buf) {
                Ok(0) => anyhow::bail!("Mouse disconnected"),
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(0),
                Err(e) => return Err(e.into()),
            }
        }
    }
}
