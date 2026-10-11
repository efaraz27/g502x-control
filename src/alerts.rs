//! Native notifications with per-device, persistent deduplication.
use crate::preferences::{self, Preferences};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct AlertState {
    warned: bool,
    threshold: u8,
    last_attempt: u64,
    last_delivered: u64,
}
impl AlertState {
    fn eligible(&mut self, percent: u8, charge: u8, threshold: u8, time: u64) -> bool {
        if percent > 100 || charge > 4 {
            return false;
        }
        if self.threshold != threshold {
            self.warned = false;
            self.threshold = threshold;
        }
        if (1..=4).contains(&charge) || percent >= threshold.saturating_add(5) {
            self.warned = false;
            return false;
        }
        percent <= threshold
            && !self.warned
            && (self.last_attempt == 0 || time.saturating_sub(self.last_attempt) >= 60)
            && (self.last_delivered == 0 || time.saturating_sub(self.last_delivered) >= 600)
    }
}

pub fn private_lock(name: &str) -> Result<File> {
    let dir = crate::device::state_dir()?;
    fs::create_dir_all(&dir)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    Ok(options.open(dir.join(name))?)
}

pub fn check(unit: &str, percent: u8, charge: u8, prefs: &Preferences) -> Result<()> {
    if !prefs.battery_alerts {
        return Ok(());
    }
    let guard = private_lock("alerts.lock")?;
    // Foreground and background processes may both read battery, but only one alerts.
    match guard.try_lock() {
        Ok(()) => (),
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    let path = crate::device::state_dir()?.join("alert-state.json");
    let mut states: BTreeMap<String, AlertState> = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).context("Reading battery alert state")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.into()),
    };
    let previous = states.clone();
    let state = states.entry(unit.to_owned()).or_default();
    let time = now();
    if !state.eligible(percent, charge, prefs.low_battery_percent, time) {
        if previous != states {
            preferences::atomic_json(&path, &states)?;
        }
        return Ok(());
    }
    state.last_attempt = time;
    preferences::atomic_json(&path, &states)?;
    let result = show(
        "G502 X Plus battery is low",
        &format!("{}% remaining. Connect your mouse to charge.", percent),
    );
    if result.is_ok() {
        let state = states.get_mut(unit).unwrap();
        state.warned = true;
        state.last_delivered = time;
        preferences::atomic_json(&path, &states)?;
    }
    result
}

pub fn show(title: &str, body: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        static APPLICATION: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();
        APPLICATION
            .get_or_init(|| {
                mac_notification_sys::set_application(preferences::APP_ID)
                    .map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(|e| anyhow::anyhow!("Registering notification application: {e}"))?;
        mac_notification_sys::send_notification(title, None, body, None)
            .context("Sending desktop notification (check OS notification permissions)")?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "windows")]
        {
            use winreg::{RegKey, enums::*};
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(format!(
                "Software\\Classes\\AppUserModelId\\{}",
                preferences::APP_ID
            ))?;
            key.set_value("DisplayName", &"G502 X Control")?;
            key.set_value(
                "IconUri",
                &std::env::current_exe()?.to_string_lossy().as_ref(),
            )?;
        }
        let mut notification = notify_rust::Notification::new();
        notification
            .appname("G502 X Control")
            .summary(title)
            .body(body)
            .timeout(10000);
        #[cfg(target_os = "linux")]
        notification
            .icon("input-mouse")
            .hint(notify_rust::Hint::DesktopEntry("g502x-control".into()));
        #[cfg(target_os = "windows")]
        notification.app_id(preferences::APP_ID);
        notification
            .show()
            .context("Sending desktop notification (check OS notification permissions)")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn low_alert_only_once_until_recharged() {
        let mut s = AlertState::default();
        assert!(!s.eligible(21, 0, 20, 1000));
        assert!(s.eligible(20, 0, 20, 1000));
        s.warned = true;
        s.last_delivered = 1000;
        assert!(!s.eligible(19, 0, 20, 2000));
        assert!(!s.eligible(22, 0, 20, 2000)); // threshold jitter does not rearm
        assert!(!s.eligible(26, 0, 20, 2000));
        assert!(s.eligible(20, 0, 20, 2100));
    }
    #[test]
    fn charging_invalid_data_and_rapid_reconnect_do_not_alert() {
        let mut s = AlertState::default();
        for status in 1..=6 {
            assert!(!s.eligible(5, status, 20, 1000));
        }
        assert!(!s.eligible(255, 0, 20, 1000));
        s.warned = true;
        s.last_delivered = 1000;
        assert!(!s.eligible(10, 1, 20, 1010));
        assert!(!s.eligible(10, 0, 20, 1020));
        assert!(s.eligible(10, 0, 20, 1601));
    }
    #[test]
    fn persisted_state_survives_restart_and_failed_delivery_retries() {
        let s = AlertState {
            warned: true,
            threshold: 20,
            last_delivered: 1000,
            ..Default::default()
        };
        let mut restored: AlertState =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(!restored.eligible(10, 0, 20, 5000));
        restored.warned = false;
        restored.last_attempt = 5000;
        assert!(!restored.eligible(10, 0, 20, 5020));
        assert!(restored.eligible(10, 0, 20, 5060));
    }
}
