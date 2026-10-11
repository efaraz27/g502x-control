//! Local preferences and per-user login startup integration.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.efaraz27.g502x-control";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Preferences {
    pub live_updates: bool,
    pub refresh_seconds: u64,
    pub battery_alerts: bool,
    pub low_battery_percent: u8,
    pub background_enabled: bool,
    pub launch_at_login: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            live_updates: true,
            refresh_seconds: 2,
            battery_alerts: true,
            low_battery_percent: 20,
            background_enabled: false,
            launch_at_login: false,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=10).contains(&self.refresh_seconds),
            "Refresh interval must be 1–10 seconds"
        );
        ensure!(
            (5..=50).contains(&self.low_battery_percent),
            "Battery threshold must be 5–50%"
        );
        ensure!(
            !self.launch_at_login || self.background_enabled,
            "Startup requires background monitoring"
        );
        Ok(())
    }
    pub fn load() -> Result<Self> {
        let path = crate::device::state_dir()?.join("preferences.json");
        let result: Self = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("Reading preferences")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.into()),
        };
        result.validate()?;
        Ok(result)
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        atomic_json(&crate::device::state_dir()?.join("preferences.json"), self)
    }
}

pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).context("Saving settings atomically")?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn apply(next: &Preferences) -> Result<()> {
    next.validate()?;
    let old = Preferences::load()?;
    let executable = std::env::current_exe()?;
    // Configure startup first. If settings cannot be saved, restore the old entry.
    set_startup(next.launch_at_login, &executable)?;
    if let Err(error) = next.save() {
        let _ = set_startup(old.launch_at_login, &executable);
        return Err(error);
    }
    if next.background_enabled
        && let Err(error) = crate::monitor::start()
    {
        let _ = old.save();
        let _ = set_startup(old.launch_at_login, &executable);
        return Err(error);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn startup_path() -> Result<PathBuf> {
    // Useful for portable setups and isolated installer tests.
    if let Some(dir) = std::env::var_os("G502X_AUTOSTART_DIR") {
        return Ok(PathBuf::from(dir).join(if cfg!(target_os = "linux") {
            "g502x-control.desktop"
        } else {
            "io.github.efaraz27.g502x-control.plist"
        }));
    }
    #[cfg(target_os = "linux")]
    {
        let config = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(path) => PathBuf::from(path),
            None => {
                PathBuf::from(std::env::var_os("HOME").context("HOME is unset")?).join(".config")
            }
        };
        Ok(config.join("autostart/g502x-control.desktop"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(
            PathBuf::from(std::env::var_os("HOME").context("HOME is unset")?)
                .join("Library/LaunchAgents/io.github.efaraz27.g502x-control.plist"),
        )
    }
}

pub fn startup_enabled() -> Result<bool> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        Ok(startup_path()?.is_file())
    }
    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::*};
        match RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        {
            Ok(key) => Ok(key.get_value::<String, _>("G502 X Control").is_ok()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

pub fn set_startup(enabled: bool, executable: &Path) -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let path = startup_path()?;
        if enabled {
            let exe = executable.to_str().context("Startup path is not UTF-8")?;
            atomic_write(&path, startup_document(exe).as_bytes())?;
        } else if let Err(e) = fs::remove_file(path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e.into());
        }
    }
    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::*};
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
        if enabled {
            let exe = executable.to_str().context("Startup path is not UTF-8")?;
            ensure!(!exe.contains('"'), "Invalid executable path");
            key.set_value("G502 X Control", &format!("\"{exe}\" --background"))?;
        } else if let Err(e) = key.delete_value("G502 X Control")
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e.into());
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn startup_document(exe: &str) -> String {
    let exec: String = exe
        .chars()
        .flat_map(|c| match c {
            '%' => vec!['%', '%'],
            '\\' => vec!['\\', '\\', '\\', '\\'],
            '"' | '$' | '`' => vec!['\\', '\\', c],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            _ => vec![c],
        })
        .collect();
    format!(
        "[Desktop Entry]\nType=Application\nName=G502 X Control\nComment=Background battery alerts\nExec=\"{exec}\" --background\nIcon=g502x-control\nTerminal=false\nStartupNotify=false\nX-GNOME-Autostart-enabled=true\n"
    )
}
#[cfg(target_os = "macos")]
fn startup_document(exe: &str) -> String {
    let escaped = exe
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>{APP_ID}</string>
<key>AssociatedBundleIdentifiers</key><array><string>{APP_ID}</string></array>
<key>ProgramArguments</key><array><string>{escaped}</string><string>--background</string></array>
<key>RunAtLoad</key><true/><key>ProcessType</key><string>Background</string>
</dict></plist>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_preferences_receive_safe_defaults() {
        let p: Preferences = serde_json::from_str("{}").unwrap();
        assert!(p.live_updates && p.battery_alerts);
        assert!(!p.background_enabled && !p.launch_at_login);
        assert!(p.validate().is_ok());
    }
    #[test]
    fn reject_invalid_polling_and_startup_combinations() {
        let mut p = Preferences {
            refresh_seconds: 0,
            ..Preferences::default()
        };
        assert!(p.validate().is_err());
        p.refresh_seconds = 2;
        p.launch_at_login = true;
        assert!(p.validate().is_err());
    }
    #[test]
    fn atomic_settings_replace_leaves_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        atomic_json(&path, &vec![1, 2]).unwrap();
        atomic_json(&path, &vec![3]).unwrap();
        assert_eq!(
            serde_json::from_slice::<Vec<u8>>(&fs::read(path).unwrap()).unwrap(),
            vec![3]
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entry_quotes_spaces_and_percent_codes() {
        let text = startup_document("/home/a b/100%/g502x-control");
        assert!(text.contains("Exec=\"/home/a b/100%%/g502x-control\" --background"));
        assert!(!text.contains("Terminal=true"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn launch_agent_escapes_xml_paths() {
        assert!(
            startup_document("/Applications/A & B.app/Contents/MacOS/app")
                .contains("A &amp; B.app")
        );
    }
}
