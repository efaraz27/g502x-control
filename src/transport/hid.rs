//! Native Windows HID and shared-access macOS HID via hidapi.
use super::Endpoint;
use anyhow::{Context, Result};
use hidapi::{HidApi, HidDevice};
use std::{ffi::CString, path::PathBuf, time::Duration};

pub fn discover() -> Result<Vec<Endpoint>> {
    let api = HidApi::new().context("Enumerating USB HID devices")?;
    let mut found = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != 0x046d || ![0xc095, 0xc547, 0x4099].contains(&info.product_id()) {
            continue;
        }
        // Windows exposes individual top-level collections. HID++ long
        // reports use vendor page FF00, usage 2. macOS can enumerate several
        // collections on the same interface; only vendor collections qualify.
        #[cfg(target_os = "windows")]
        let compatible = info.usage_page() == 0xff00 && info.usage() == 2;
        #[cfg(target_os = "macos")]
        let compatible = info.usage_page() == 0xff00 && [1, 2].contains(&info.usage());
        if !compatible {
            continue;
        }
        let path = PathBuf::from(info.path().to_str().context("Non-UTF-8 HID device path")?);
        if found.iter().any(|e: &Endpoint| e.path == path) {
            continue;
        }
        found.push(Endpoint {
            path,
            name: info.product_string().unwrap_or("G502 X Plus").to_owned(),
            serial: info.serial_number().unwrap_or_default().to_owned(),
            product: info.product_id(),
            index: if info.product_id() == 0xc095 { 0xff } else { 1 },
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

pub struct RawTransport {
    device: HidDevice,
    // Keep the API context alive for the device lifetime. macOS access is
    // shared (Cargo feature), so opening the app does not seize the pointer.
    _api: HidApi,
}
impl RawTransport {
    pub fn open(endpoint: &Endpoint) -> Result<Self> {
        let api = HidApi::new()?;
        let path = CString::new(endpoint.path.to_str().context("Invalid HID path")?)?;
        let device = api.open_path(&path).with_context(|| format!(
            "Cannot open {}. Close G HUB or other mouse configuration apps and reconnect the mouse. On macOS, check Input Monitoring permissions.",
            endpoint.path.display()))?;
        Ok(Self { device, _api: api })
    }
    pub fn write(&mut self, packet: &[u8]) -> Result<usize> {
        let written = self.device.write(packet)?;
        #[cfg(target_os = "windows")]
        {
            // hidapi's native Windows backend returns 0 for synchronous
            // WriteFile success, or the padded report length for overlapped
            // success. The matching HID++ response confirms device delivery.
            if written == 0 || written >= packet.len() {
                return Ok(packet.len());
            }
        }
        Ok(written)
    }
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        let milliseconds = if timeout.is_zero() {
            0
        } else {
            timeout.as_millis().clamp(1, 1200) as i32
        };
        Ok(self.device.read_timeout(buf, milliseconds)?)
    }
}
