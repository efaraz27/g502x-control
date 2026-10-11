//! Singleton battery monitor with an optional native tray, without a GPU window.
use crate::{
    alerts,
    device::{Device, Snapshot},
    preferences::{self, Preferences},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Default, Serialize, Deserialize)]
pub struct Health {
    pub last_checked: u64,
    pub connected: bool,
    pub battery: Option<u8>,
    #[serde(default)]
    pub charging: bool,
    pub error: Option<String>,
}
pub fn running() -> Result<bool> {
    let guard = alerts::private_lock("monitor.lock")?;
    match guard.try_lock() {
        Ok(()) => Ok(false),
        Err(std::fs::TryLockError::WouldBlock) => Ok(true),
        Err(e) => Err(e.into()),
    }
}
pub fn health() -> Option<Health> {
    let path = crate::device::state_dir().ok()?.join("monitor-status.json");
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
pub fn tray_error() -> Option<String> {
    let path = crate::device::state_dir().ok()?.join("tray-error.txt");
    fs::read_to_string(path).ok()
}
pub fn start() -> Result<()> {
    if running()? {
        return Ok(());
    }
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .context("Starting background battery monitor")?;
    // Reap the child if the GUI remains open; the monitor survives if it closes.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub fn quit() -> Result<()> {
    if running()? {
        preferences::atomic_write(&crate::device::state_dir()?.join("monitor-stop"), b"stop")?;
    }
    Ok(())
}
pub fn wait_stopped() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while running()? {
        anyhow::ensure!(
            Instant::now() < deadline,
            "Monitor is still finishing an operation. Try again shortly."
        );
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
pub fn run(path: Option<PathBuf>) -> Result<()> {
    let guard = alerts::private_lock("monitor.lock")?;
    match guard.try_lock() {
        Ok(()) => (),
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    if !Preferences::load()?.background_enabled {
        return Ok(());
    }
    let _ = fs::remove_file(crate::device::state_dir()?.join("monitor-stop"));
    let _ = fs::remove_file(crate::device::state_dir()?.join("monitor-status.json"));
    let _ = fs::remove_file(crate::device::state_dir()?.join("tray-error.txt"));
    let stop = Arc::new(AtomicBool::new(false));
    #[cfg(feature = "gui")]
    {
        let worker_stop = stop.clone();
        let worker = thread::spawn(move || {
            let result = poll(path, &worker_stop);
            worker_stop.store(true, Ordering::Relaxed);
            result
        });
        let result = crate::tray::run(stop.clone());
        // A missing tray host must not silently kill battery monitoring.
        if let Err(ref e) = result {
            let _ = preferences::atomic_write(
                &crate::device::state_dir()?.join("tray-error.txt"),
                format!("{e:#}").as_bytes(),
            );
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));
            }
        } else {
            let _ = fs::remove_file(crate::device::state_dir()?.join("tray-error.txt"));
        }
        stop.store(true, Ordering::Relaxed);
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Battery worker stopped unexpectedly"))??;
        Ok(())
    }
    #[cfg(not(feature = "gui"))]
    poll(path, &stop)
}
fn poll(path: Option<PathBuf>, stop: &AtomicBool) -> Result<()> {
    let mut cached: Option<Snapshot> = None;
    let mut next = Instant::now();
    loop {
        let prefs = Preferences::load()?;
        if !prefs.background_enabled
            || stop.load(Ordering::Relaxed)
            || crate::device::state_dir()?.join("monitor-stop").exists()
        {
            break;
        }
        if Instant::now() >= next {
            let result = (|| -> Result<(u8, bool, Option<String>)> {
                let mut device = Device::connect_cached(path.as_deref(), cached.as_ref())?;
                if cached.as_ref().is_none_or(|s| {
                    s.unit_id != device.unit_id || s.endpoint.path != device.transport.endpoint.path
                }) {
                    cached = Some(device.snapshot()?);
                }
                let (percent, charge) = device.battery()?;
                let unit = device.unit_id.clone();
                drop(device); // Never retain the HID lock while notifying or sleeping.
                let error = alerts::check(&unit, percent, charge, &prefs)
                    .err()
                    .map(|e| format!("{e:#}"));
                Ok((percent, (1..=4).contains(&charge), error))
            })();
            let health = match result {
                Ok((battery, charging, error)) => Health {
                    last_checked: alerts::now(),
                    connected: true,
                    battery: Some(battery),
                    charging,
                    error,
                },
                Err(e) => {
                    cached = None;
                    Health {
                        last_checked: alerts::now(),
                        error: Some(format!("{e:#}")),
                        ..Default::default()
                    }
                }
            };
            let _ = preferences::atomic_json(
                &crate::device::state_dir()?.join("monitor-status.json"),
                &health,
            );
            next = Instant::now() + Duration::from_secs(60);
        }
        thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
