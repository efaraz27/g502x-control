use crate::{
    profile::{Profile, seal, valid_crc},
    transport::{self, Endpoint, Transport},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Feature {
    pub id: u16,
    pub index: u8,
    pub flags: u8,
    pub version: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Layout {
    pub memory: u8,
    pub format: u8,
    pub macro_format: u8,
    pub profiles: u8,
    pub buttons: u8,
    pub sectors: u8,
    pub size: u16,
    pub g_shift: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub battery: u8,
    pub charge_status: u8,
    pub dpi: u16,
    pub rate_ms: u8,
    pub mode: u8,
    pub active_sector: u16,
    pub dpi_index: u8,
    pub wheel: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub index: u8,
    pub id: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u8,
    pub unit_id: String,
    pub endpoint: Endpoint,
    pub protocol: String,
    pub features: Vec<Feature>,
    pub layout: Layout,
    pub status: Status,
    pub dpi_choices: Vec<u16>,
    pub rate_choices: Vec<u8>,
    pub effects: Vec<Effect>,
    pub led_ids: Vec<u8>,
    pub directory_sector: u16,
    pub directory: Vec<u8>,
    pub profiles: Vec<Profile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backup {
    pub schema: u8,
    pub unit_id: String,
    pub layout: Layout,
    pub created_unix: u64,
    pub sectors: BTreeMap<u16, Vec<u8>>,
}

pub struct Device {
    pub transport: Transport,
    pub unit_id: String,
    pub features: Vec<Feature>,
    pub protocol: String,
    _lock: File,
}
fn lock() -> Result<File> {
    let dir = state_dir()?;
    fs::create_dir_all(&dir)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let f = options.open(dir.join("device.lock"))?;
    f.try_lock()
        .context("Another G502 X Control operation is in progress, or its lock is unavailable")?;
    Ok(f)
}
pub fn state_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unset")?);
    #[cfg(target_os = "macos")]
    let base = PathBuf::from(std::env::var_os("HOME").context("HOME is unset")?)
        .join("Library/Application Support");
    #[cfg(target_os = "linux")]
    let base = if let Some(p) = std::env::var_os("XDG_STATE_HOME") {
        PathBuf::from(p)
    } else {
        PathBuf::from(std::env::var_os("HOME").context("HOME is unset")?).join(".local/state")
    };
    Ok(base.join("g502x-control"))
}

impl Device {
    pub fn connect(path: Option<&Path>) -> Result<Self> {
        let guard = lock()?;
        let endpoints = transport::discover()?;
        let mut errors = Vec::new();
        for endpoint in endpoints
            .into_iter()
            .filter(|e| path.is_none_or(|p| p == e.path))
        {
            let trial = (|| -> Result<_> {
                let mut t = Transport::open(endpoint.clone())?;
                let ping = t.request(0, 1, &[0, 0, 0x5a])?;
                ensure!(ping[0] >= 2 && ping[2] == 0x5a, "Not a HID++ 2+ device");
                let fw = t.request(0, 0, &0x0003u16.to_be_bytes())?[0];
                ensure!(fw > 0, "No identity feature");
                let identity = t.request(fw, 0, &[])?;
                ensure!(
                    identity[7..11] == [0x40, 0x99, 0xc0, 0x95],
                    "Connected receiver device is not a G502 X Plus"
                );
                let unit_id = identity[1..5].iter().map(|b| format!("{b:02X}")).collect();
                let feature_set = t.request(0, 0, &[0, 1])?[0];
                ensure!(feature_set > 0, "No feature set");
                let count = t.request(feature_set, 0, &[])?[0];
                ensure!(count < 128, "Invalid feature count");
                let mut features = vec![Feature {
                    id: 0,
                    index: 0,
                    flags: 0,
                    version: 0,
                }];
                for index in 1..=count {
                    let r = t.request(feature_set, 1, &[index])?;
                    features.push(Feature {
                        id: u16::from_be_bytes([r[0], r[1]]),
                        index,
                        flags: r[2],
                        version: r[3],
                    });
                }
                Ok((t, unit_id, features, format!("{}.{}", ping[0], ping[1])))
            })();
            match trial {
                Ok((transport, unit_id, features, protocol)) => {
                    return Ok(Self {
                        transport,
                        unit_id,
                        features,
                        protocol,
                        _lock: guard,
                    });
                }
                Err(e) => errors.push(format!("{}: {e:#}", endpoint.path.display())),
            }
        }
        bail!(
            "No responding G502 X Plus. Wake the mouse or connect its USB cable.\n{}",
            errors.join("\n")
        )
    }
    pub fn call(&mut self, id: u16, function: u8, p: &[u8]) -> Result<[u8; 16]> {
        let index = self
            .features
            .iter()
            .find(|f| f.id == id)
            .with_context(|| format!("Mouse does not expose feature {id:04x}"))?
            .index;
        self.transport.request(index, function, p)
    }
    pub fn layout(&mut self) -> Result<Layout> {
        let r = self.call(0x8100, 0, &[])?;
        let l = Layout {
            memory: r[0],
            format: r[1],
            macro_format: r[2],
            profiles: r[3],
            buttons: r[5],
            sectors: r[6],
            size: u16::from_be_bytes([r[7], r[8]]),
            g_shift: r[9] & 3 == 2,
        };
        ensure!(
            l.memory == 1
                && l.format == 5
                && l.buttons == 11
                && (1..=5).contains(&l.profiles)
                && (6..=64).contains(&l.sectors)
                && (255..=4096).contains(&l.size),
            "Unsupported onboard layout: {l:?}"
        );
        Ok(l)
    }
    pub fn status(&mut self) -> Result<Status> {
        let battery = self.call(0x1004, 1, &[])?;
        let dpi = self.call(0x2201, 2, &[0])?;
        let mode = self.call(0x8100, 2, &[])?[0];
        let active = self.call(0x8100, 4, &[])?;
        Ok(Status {
            battery: battery[0],
            charge_status: battery[2],
            dpi: if dpi[1..3] == [0, 0] {
                u16::from_be_bytes([dpi[3], dpi[4]])
            } else {
                u16::from_be_bytes([dpi[1], dpi[2]])
            },
            rate_ms: self.call(0x8060, 1, &[])?[0],
            mode,
            active_sector: u16::from_be_bytes([active[0], active[1]]),
            dpi_index: self.call(0x8100, 11, &[])?[0],
            wheel: self.call(0x2121, 1, &[])?[0],
        })
    }
    pub fn read_sector(&mut self, sector: u16, size: u16) -> Result<Vec<u8>> {
        ensure!((16..=4096).contains(&size), "Invalid sector size");
        let mut data = vec![0; size as usize];
        let mut offset = 0usize;
        while offset < data.len() {
            let start = offset.min(data.len() - 16);
            let mut p = sector.to_be_bytes().to_vec();
            p.extend_from_slice(&(start as u16).to_be_bytes());
            let reply = self.call(0x8100, 5, &p)?;
            data[start..start + 16].copy_from_slice(&reply);
            offset = start + 16;
        }
        Ok(data)
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let layout = self.layout()?;
        let mut directory_sector = 0;
        let mut directory = self.read_sector(0, layout.size)?;
        if directory[..4] == [0, 0, 0, 0] || directory[..4] == [255; 4] {
            directory_sector = 0x100;
            directory = self.read_sector(0x100, layout.size)?;
        }
        ensure!(
            valid_crc(&directory),
            "Profile directory checksum is invalid; refusing to edit"
        );
        let mut profiles = Vec::new();
        let mut seen = Vec::new();
        for entry in directory[..directory.len() - 2]
            .chunks_exact(4)
            .take(layout.profiles as usize + 1)
        {
            let sector = u16::from_be_bytes([entry[0], entry[1]]);
            if sector == 0xffff {
                break;
            }
            ensure!(
                profiles.len() < layout.profiles as usize
                    && !seen.contains(&sector)
                    && (sector > 0 && sector < layout.sectors as u16
                        || (0x101..=0x105).contains(&sector)),
                "Invalid profile directory entry {sector}"
            );
            seen.push(sector);
            let p = Profile {
                sector,
                enabled: entry[2] != 0,
                raw: self.read_sector(sector, layout.size)?,
            };
            p.validate_shape()?;
            ensure!(
                valid_crc(&p.raw),
                "Profile {sector} checksum invalid; refusing to edit"
            );
            profiles.push(p);
        }
        ensure!(!profiles.is_empty(), "No profiles found");
        let dpi_reply = self.call(0x2201, 1, &[0, 0, 0])?;
        let dpi_choices = parse_dpi_list(&dpi_reply[1..])?;
        let mask = self.call(0x8060, 0, &[])?[0];
        let rate_choices = (1u8..=8).filter(|n| mask & (1 << (n - 1)) != 0).collect();
        let zone = self.call(0x8071, 0, &[0, 255, 0])?;
        ensure!(zone[4] < 64, "Invalid RGB effect count");
        let mut effects = Vec::new();
        for index in 0..zone[4] {
            let r = self.call(0x8071, 0, &[0, index, 0])?;
            effects.push(Effect {
                index,
                id: u16::from_be_bytes([r[2], r[3]]),
            });
        }
        let mut bitmap = Vec::new();
        for i in 0..3 {
            bitmap.extend_from_slice(&self.call(0x8081, 0, &[0, i])?[2..]);
        }
        let led_ids = (1u8..=254)
            .filter(|&i| bitmap[i as usize / 8] & (1 << (i % 8)) != 0)
            .collect();
        Ok(Snapshot {
            schema: 1,
            unit_id: self.unit_id.clone(),
            endpoint: self.transport.endpoint.clone(),
            protocol: self.protocol.clone(),
            features: self.features.clone(),
            layout,
            status: self.status()?,
            dpi_choices,
            rate_choices,
            effects,
            led_ids,
            directory_sector,
            directory,
            profiles,
        })
    }
    pub fn backup(&mut self) -> Result<Backup> {
        let layout = self.layout()?;
        let mut sectors = BTreeMap::new();
        for n in 0..layout.sectors as u16 {
            sectors.insert(n, self.read_sector(n, layout.size)?);
        }
        Ok(Backup {
            schema: 1,
            unit_id: self.unit_id.clone(),
            layout,
            created_unix: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            sectors,
        })
    }
    pub fn automatic_backup(&mut self) -> Result<PathBuf> {
        let b = self.backup()?;
        let dir = state_dir()?.join("backups");
        fs::create_dir_all(&dir)?;
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = dir.join(format!("{}-{nanos}.json", self.unit_id));
        write_json_new(&path, &b)?;
        Ok(path)
    }
    fn write_sector(&mut self, sector: u16, bytes: &[u8], layout: &Layout) -> Result<()> {
        ensure!(
            sector < layout.sectors as u16 && bytes.len() == layout.size as usize,
            "Invalid flash write range"
        );
        let mut args = sector.to_be_bytes().to_vec();
        args.extend_from_slice(&[0, 0]);
        args.extend_from_slice(&layout.size.to_be_bytes());
        self.call(0x8100, 6, &args)?;
        // Flash writes must never be retried blindly: a timed-out write may have succeeded.
        for chunk in bytes.chunks(16) {
            self.call(0x8100, 7, chunk).context(
                "Flash write interrupted. Keep the backup and reconnect before restoring",
            )?;
        }
        self.call(0x8100, 8, &[])?;
        ensure!(
            self.read_sector(sector, layout.size)? == bytes,
            "Flash verification failed for sector {sector}. Restore the automatic backup"
        );
        Ok(())
    }
    pub fn save_profile(
        &mut self,
        baseline: &Snapshot,
        edited: &Profile,
    ) -> Result<Option<PathBuf>> {
        ensure!(
            baseline.schema == 1 && baseline.unit_id == self.unit_id,
            "Profile belongs to another mouse"
        );
        let current = self.snapshot()?;
        ensure!(
            baseline.layout == current.layout
                && baseline.directory == current.directory
                && baseline.directory_sector == current.directory_sector,
            "Mouse profile layout changed; refresh first"
        );
        ensure!(
            current.directory_sector == 0,
            "Factory-only profile directory is read-only; initialize onboard profiles with an existing supported tool first"
        );
        let old = baseline
            .profiles
            .iter()
            .find(|p| p.sector == edited.sector)
            .context("Unknown profile")?;
        let actual = current
            .profiles
            .iter()
            .find(|p| p.sector == edited.sector)
            .context("Profile disappeared")?;
        ensure!(
            old == actual,
            "Profile changed on the mouse since it was read. Refresh before saving"
        );
        ensure!(
            edited.sector > 0 && edited.sector < current.layout.sectors as u16,
            "Factory profile is read-only"
        );
        edited.validate_settings(&current.dpi_choices, &current.rate_choices)?;
        let mut raw = edited.raw.clone();
        ensure!(
            raw.len() == current.layout.size as usize,
            "Wrong profile length"
        );
        seal(&mut raw);
        let mut directory = current.directory.clone();
        let pos = current
            .profiles
            .iter()
            .position(|p| p.sector == edited.sector)
            .unwrap();
        if old.enabled != edited.enabled {
            directory[pos * 4 + 2] = u8::from(edited.enabled);
            seal(&mut directory);
        }
        ensure!(
            current
                .profiles
                .iter()
                .any(|p| if p.sector == edited.sector {
                    edited.enabled
                } else {
                    p.enabled
                }),
            "At least one profile must remain enabled"
        );
        ensure!(
            edited.enabled || current.status.active_sector != edited.sector,
            "Activate another profile before disabling this one"
        );
        if raw == actual.raw && directory == current.directory {
            return Ok(None);
        }
        let backup = self.automatic_backup()?;
        (|| -> Result<()> {
            if raw != actual.raw {
                self.write_sector(edited.sector, &raw, &current.layout)?;
            }
            if directory != current.directory {
                self.write_sector(0, &directory, &current.layout)?;
            }
            if current.status.mode == 1 && current.status.active_sector == edited.sector {
                self.activate(edited.sector)?;
            }
            Ok(())
        })()
        .with_context(|| format!("Save failed. Backup: {}", backup.display()))?;
        Ok(Some(backup))
    }
    pub fn restore(&mut self, b: &Backup) -> Result<PathBuf> {
        let layout = self.layout()?;
        validate_backup(b, &self.unit_id, &layout)?;
        let before = self.status()?;
        let backup = self.automatic_backup()?;
        // Data first, directory last. Raw macro/animation sectors are restored unchanged.
        (|| -> Result<()> {
            for n in (1..layout.sectors as u16).chain(std::iter::once(0)) {
                let bytes = &b.sectors[&n];
                if self.read_sector(n, layout.size)? != *bytes {
                    self.write_sector(n, bytes, &layout)?;
                }
            }
            if before.mode == 1 {
                let restored = self.snapshot()?;
                let selected = restored
                    .profiles
                    .iter()
                    .find(|p| p.enabled && p.sector == before.active_sector)
                    .or_else(|| restored.profiles.iter().find(|p| p.enabled))
                    .context("Restored memory has no enabled profile")?
                    .sector;
                self.activate(selected)?;
            }
            Ok(())
        })()
        .with_context(|| format!("Restore failed. Pre-restore backup: {}", backup.display()))?;
        Ok(backup)
    }
    pub fn activate(&mut self, sector: u16) -> Result<()> {
        let s = self.snapshot()?;
        ensure!(
            s.profiles.iter().any(|p| p.sector == sector && p.enabled),
            "Profile is absent or disabled"
        );
        self.call(0x8100, 1, &[1])?;
        self.call(0x8100, 3, &sector.to_be_bytes())?;
        let after = self.status()?;
        ensure!(
            after.mode == 1 && after.active_sector == sector,
            "Profile activation did not take effect"
        );
        Ok(())
    }
    pub fn set_mode(&mut self, onboard: bool) -> Result<()> {
        let mode = if onboard { 1 } else { 2 };
        self.call(0x8100, 1, &[mode])?;
        ensure!(
            self.call(0x8100, 2, &[])?[0] == mode,
            "Mode verification failed"
        );
        Ok(())
    }
    pub fn set_dpi(&mut self, dpi: u16) -> Result<()> {
        let s = self.snapshot()?;
        ensure!(s.dpi_choices.contains(&dpi), "Unsupported DPI {dpi}");
        ensure!(
            s.status.mode == 2,
            "Switch to Software mode for live DPI, or edit an onboard profile"
        );
        let [hi, lo] = dpi.to_be_bytes();
        self.call(0x2201, 3, &[0, hi, lo])?;
        ensure!(self.status()?.dpi == dpi, "DPI readback did not match");
        Ok(())
    }
    pub fn set_rate(&mut self, ms: u8) -> Result<()> {
        let s = self.snapshot()?;
        ensure!(s.rate_choices.contains(&ms), "Unsupported polling rate");
        ensure!(
            s.status.mode == 2,
            "Switch to Software mode for live polling rate, or edit an onboard profile"
        );
        self.call(0x8060, 2, &[ms])?;
        ensure!(self.status()?.rate_ms == ms, "Rate readback did not match");
        Ok(())
    }
    pub fn set_dpi_index(&mut self, index: u8) -> Result<()> {
        let s = self.snapshot()?;
        let p = s
            .profiles
            .iter()
            .find(|p| p.sector == s.status.active_sector)
            .context("No active profile")?;
        ensure!(
            s.status.mode == 1 && index < 5 && p.dpis()[index as usize] > 0,
            "Select an enabled DPI stage in onboard mode"
        );
        self.call(0x8100, 12, &[index])?;
        ensure!(
            self.status()?.dpi_index == index,
            "DPI stage readback did not match"
        );
        Ok(())
    }
    pub fn set_wheel(&mut self, reverse: bool, hires: bool) -> Result<()> {
        let old = self.call(0x2121, 1, &[])?[0];
        let new = (old & !6) | if reverse { 4 } else { 0 } | if hires { 2 } else { 0 };
        self.call(0x2121, 2, &[new])?;
        ensure!(
            self.call(0x2121, 1, &[])?[0] == new,
            "Wheel readback did not match"
        );
        Ok(())
    }
    pub fn set_rgb(&mut self, effect: [u8; 11]) -> Result<()> {
        let s = self.snapshot()?;
        ensure!(
            s.status.mode == 2,
            "Switch to Software mode for live lighting, or save lighting in an onboard profile"
        );
        let index = s
            .effects
            .iter()
            .find(|e| e.id == effect[0] as u16)
            .context("Effect is not supported by this mouse")?
            .index;
        self.call(0x8071, 5, &[1, 3, 5])?;
        let mut p = vec![0, index];
        p.extend_from_slice(&effect[1..]);
        p.push(1);
        self.call(0x8071, 1, &p)?;
        Ok(())
    }
    pub fn set_leds(&mut self, colors: &[[u8; 3]]) -> Result<()> {
        let s = self.snapshot()?;
        ensure!(
            colors.len() == s.led_ids.len(),
            "Provide one color for each LED"
        );
        self.set_rgb(crate::profile::effect_bytes(1, [0, 0, 0], 5000, 100)?)?;
        // Artanis preparation from Solaar: stop the firmware effects engine.
        let mut prep = vec![0xff, s.effects.len() as u8];
        prep.extend_from_slice(&[0; 10]);
        prep.push(1);
        self.call(0x8071, 1, &prep)?;
        for _ in 0..2 {
            for (led, color) in s.led_ids.iter().zip(colors) {
                self.call(0x8081, 1, &[*led, color[0], color[1], color[2]])?;
            }
            self.call(0x8081, 7, &[0])?;
        }
        Ok(())
    }
    pub fn release_rgb(&mut self) -> Result<()> {
        self.call(0x8071, 5, &[1, 0, 0])?;
        Ok(())
    }
}

pub fn parse_dpi_list(bytes: &[u8]) -> Result<Vec<u16>> {
    let words: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let n = words[i];
        if n == 0 {
            break;
        }
        if n & 0xe000 == 0xe000 {
            let step = n & 0x1fff;
            let last = *words.get(i + 1).context("Truncated DPI range")?;
            let first = *out.last().context("DPI range has no start")?;
            ensure!(step > 0 && last > first, "Invalid DPI range");
            let mut next = first as u32 + step as u32;
            while next <= last as u32 {
                out.push(next as u16);
                next += step as u32;
            }
            i += 2;
        } else {
            out.push(n);
            i += 1;
        }
    }
    ensure!(
        !out.is_empty() && out.len() < 10000,
        "Invalid DPI capabilities"
    );
    Ok(out)
}
pub fn validate_backup(b: &Backup, unit: &str, layout: &Layout) -> Result<()> {
    ensure!(
        b.schema == 1 && b.unit_id == unit && &b.layout == layout,
        "Backup is for a different mouse or profile layout"
    );
    ensure!(
        b.sectors.len() == layout.sectors as usize,
        "Incomplete backup"
    );
    for n in 0..layout.sectors as u16 {
        ensure!(
            b.sectors
                .get(&n)
                .is_some_and(|s| s.len() == layout.size as usize),
            "Backup sector {n} is missing or truncated"
        );
    }
    let dir = &b.sectors[&0];
    if dir[..4] != [255; 4] && dir[..4] != [0; 4] {
        ensure!(valid_crc(dir), "Invalid directory checksum in backup");
        let mut ended = false;
        let mut seen = Vec::new();
        let mut enabled = 0;
        for e in dir[..dir.len() - 2]
            .chunks_exact(4)
            .take(layout.profiles as usize + 1)
        {
            let n = u16::from_be_bytes([e[0], e[1]]);
            if n == 0xffff {
                ended = true;
                break;
            }
            ensure!(
                seen.len() < layout.profiles as usize
                    && n > 0
                    && n < layout.sectors as u16
                    && !seen.contains(&n),
                "Invalid profile reference in backup"
            );
            seen.push(n);
            ensure!(
                valid_crc(&b.sectors[&n]),
                "Invalid profile checksum in backup"
            );
            if e[2] != 0 {
                enabled += 1;
            }
        }
        ensure!(
            ended && enabled > 0,
            "Backup directory has no terminator or enabled profile"
        );
    }
    Ok(())
}
pub fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let mut file = options.open(path).with_context(|| {
        format!(
            "Creating {} (existing files are never overwritten)",
            path.display()
        )
    })?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("g502x-test-{}-{unique}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn json_export_never_overwrites_existing_data() {
        let dir = TestDirectory::new();
        let path = dir.0.join("profile.json");
        write_json_new(&path, &vec![1, 2, 3]).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(write_json_new(&path, &vec![9, 8, 7]).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(
            serde_json::from_slice::<Vec<u8>>(&original).unwrap(),
            vec![1, 2, 3]
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn json_export_rejects_symlink_destination() {
        let dir = TestDirectory::new();
        let target = dir.0.join("private.json");
        fs::write(&target, b"original").unwrap();
        let link = dir.0.join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(write_json_new(&link, &vec![1]).is_err());
        assert_eq!(fs::read(target).unwrap(), b"original");
    }
    #[test]
    fn expands_device_dpi_range() {
        assert_eq!(
            parse_dpi_list(&[0, 100, 0xe0, 50, 1, 44, 0, 0]).unwrap(),
            [100, 150, 200, 250, 300]
        );
        assert!(parse_dpi_list(&[0, 100, 0xe0, 0, 1, 44]).is_err());
    }
    #[test]
    fn backup_rejects_wrong_identity_and_missing_sectors() {
        let l = Layout {
            memory: 1,
            format: 5,
            macro_format: 1,
            profiles: 5,
            buttons: 11,
            sectors: 16,
            size: 255,
            g_shift: true,
        };
        let b = Backup {
            schema: 1,
            unit_id: "a".into(),
            layout: l.clone(),
            created_unix: 0,
            sectors: BTreeMap::new(),
        };
        assert!(validate_backup(&b, "b", &l).is_err());
        assert!(validate_backup(&b, "a", &l).is_err());
    }
}
