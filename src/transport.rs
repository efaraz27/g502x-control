use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod hid;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use hid as platform;
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
compile_error!("G502 X Control supports Linux, Windows and macOS");
pub use platform::discover;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub path: PathBuf,
    pub name: String,
    pub serial: String,
    pub product: u16,
    pub index: u8,
}

pub struct Transport {
    raw: platform::RawTransport,
    pub endpoint: Endpoint,
    software_id: u8,
}
impl Transport {
    pub fn open(endpoint: Endpoint) -> Result<Self> {
        let raw = platform::RawTransport::open(&endpoint)?;
        Ok(Self {
            raw,
            endpoint,
            software_id: 7,
        })
    }
    pub fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<[u8; 16]> {
        ensure!(function < 16 && params.len() <= 16, "Invalid HID++ request");
        self.software_id = self.software_id % 15 + 1;
        let address = function << 4 | self.software_id;
        let mut packet = [0u8; 20];
        packet[..4].copy_from_slice(&[0x11, self.endpoint.index, feature, address]);
        packet[4..4 + params.len()].copy_from_slice(params);
        // Drain this handle only. Bound the drain so continuous notifications
        // cannot starve a request. Normal pointer input is never grabbed.
        let mut buf = [0u8; 64];
        for _ in 0..256 {
            if self.raw.read_timeout(&mut buf, Duration::ZERO)? == 0 {
                break;
            }
        }
        let written = self.raw.write(&packet).context("Writing HID++ report")?;
        ensure!(written == packet.len(), "Incomplete HID++ write");
        let deadline = Instant::now() + Duration::from_millis(1200);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!(
                    "Mouse timed out (feature index {feature:02x}, function {function:x}); wake it or reconnect, then refresh"
                );
            }
            let n = self.raw.read_timeout(&mut buf, remaining)?;
            if n == 0 {
                continue;
            }
            if let Some(reply) = match_reply(&buf[..n], self.endpoint.index, feature, address)? {
                return Ok(reply);
            }
        }
    }
}

pub fn match_reply(buf: &[u8], index: u8, feature: u8, address: u8) -> Result<Option<[u8; 16]>> {
    if buf.len() < 4 || ![0x10, 0x11].contains(&buf[0]) || buf[1] != index {
        return Ok(None);
    }
    if buf.len() >= 7 && [0x8f, 0xff].contains(&buf[2]) && buf[3] == feature && buf[4] == address {
        bail!(
            "HID++ error 0x{:02x} for feature index {feature:02x}, function {:x}",
            buf[5],
            address >> 4
        );
    }
    if buf[2] != feature || buf[3] != address {
        return Ok(None);
    }
    let expected = if buf[0] == 0x11 { 20 } else { 7 };
    ensure!(buf.len() >= expected, "Truncated HID++ reply");
    let mut reply = [0u8; 16];
    reply[..expected - 4].copy_from_slice(&buf[4..expected]);
    Ok(Some(reply))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignores_notifications_other_devices_and_old_transactions() {
        for (idx, feat, addr) in [(2, 7, 0x28), (1, 8, 0x28), (1, 7, 0x20), (1, 7, 0x29)] {
            let mut p = [0; 20];
            p[..4].copy_from_slice(&[0x11, idx, feat, addr]);
            assert!(match_reply(&p, 1, 7, 0x28).unwrap().is_none());
        }
    }
    #[test]
    fn surfaces_protocol_errors() {
        assert!(match_reply(&[0x11, 1, 0xff, 7, 0x28, 2, 0], 1, 7, 0x28).is_err());
    }
    #[test]
    fn rejects_truncated_reply() {
        assert!(match_reply(&[0x11, 1, 7, 0x28, 0], 1, 7, 0x28).is_err());
    }
}
