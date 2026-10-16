use crate::{
    device::{self, Device, Snapshot},
    profile::{self, Profile},
};
use anyhow::Result;
use eframe::egui::{self, Color32, RichText, Sense, Stroke, vec2};
#[path = "design.rs"]
mod design;
use design::{ACCENT, DIM, Icon, MUTED, TEXT};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};
#[derive(Clone)]
enum Job {
    Poll {
        cached: Option<Box<Snapshot>>,
        full: bool,
        live: bool,
    },
    Refresh,
    Save(Box<Snapshot>, Profile),
    Activate(u16),
    Mode(bool),
    Dpi(u16),
    Rate(u8),
    Stage(u8),
    Wheel(bool, bool),
    Rgb([u8; 11]),
    Leds(Vec<[u8; 3]>),
    ReleaseRgb,
    Backup,
    Restore(PathBuf),
    Export(Profile, PathBuf),
    Import(PathBuf),
}
struct Reply {
    snapshot: Snapshot,
    message: String,
    reset: bool,
    imported: Option<Profile>,
    full: bool,
    notification_error: Option<String>,
}
struct WorkerReply {
    automatic: bool,
    result: Result<Reply, String>,
}
#[derive(Clone, Copy)]
enum FileAction {
    Import,
    Export,
    Restore,
}
type PickerReply = Result<Option<(PathBuf, FileAction)>, String>;
struct App {
    picker: Option<Receiver<PickerReply>>,
    capture_requested: bool,
    capture_settle: u8,
    profile_tab: usize,
    details_open: bool,
    function_data: u8,
    tx: Sender<Job>,
    rx: Receiver<WorkerReply>,
    save_base: Option<Snapshot>,
    preferences: crate::preferences::Preferences,
    saved_preferences: crate::preferences::Preferences,
    settings_message: String,
    notification: Option<Receiver<Result<(), String>>>,
    poll_pending: bool,
    next_poll: Instant,
    last_full: Instant,
    last_success: Option<Instant>,
    poll_failures: u32,
    connection_error: Option<String>,
    background_running: bool,
    next_background_check: Instant,
    snapshot: Option<Snapshot>,
    draft: Option<Profile>,
    name: String,
    selected: usize,
    tab: usize,
    busy: bool,
    message: String,
    error: bool,
    shifted: bool,
    edit_button: Option<usize>,
    binding_type: usize,
    key: u8,
    modifiers: u8,
    media: u16,
    function: u8,
    mouse_button: u8,
    color: [u8; 3],
    effect: u16,
    period: u16,
    intensity: u8,
    leds: Vec<[u8; 3]>,
    live_dpi: u16,
    live_rate: u8,
    reverse: bool,
    hires: bool,
    file_path: String,
    restore_confirm: bool,
    close_confirm: bool,
}
fn perform(path: Option<&std::path::Path>, job: Job) -> Result<Reply> {
    if let Job::Poll { cached, full, live } = job {
        let mut d = Device::connect_cached(path, cached.as_deref())?;
        let full = full
            || cached.as_ref().is_none_or(|s| {
                s.unit_id != d.unit_id || s.endpoint.path != d.transport.endpoint.path
            });
        let mut snapshot = if full {
            d.snapshot()?
        } else {
            *cached.unwrap()
        };
        if !full {
            if live {
                snapshot.status = d.status()?;
            } else {
                let (battery, charge) = d.battery()?;
                snapshot.status.battery = battery;
                snapshot.status.charge_status = charge;
            }
        }
        drop(d);
        let notification_error = crate::preferences::Preferences::load()
            .and_then(|p| {
                crate::alerts::check(
                    &snapshot.unit_id,
                    snapshot.status.battery,
                    snapshot.status.charge_status,
                    &p,
                )
            })
            .err()
            .map(|e| format!("{e:#}"));
        return Ok(Reply {
            snapshot,
            message: String::new(),
            reset: false,
            imported: None,
            full,
            notification_error,
        });
    }
    let mut d = Device::connect(path)?;
    let mut reset = false;
    let mut imported = None;
    let message = match job {
        Job::Poll { .. } => unreachable!(),
        Job::Refresh => {
            reset = true;
            "Settings loaded from your mouse.".into()
        }
        Job::Save(b, p) => {
            let file = d.save_profile(&b, &p)?;
            reset = true;
            file.map_or("Already saved; no flash write.".into(), |p| {
                format!("Saved and verified. Backup: {}", p.display())
            })
        }
        Job::Activate(n) => {
            d.release_rgb()?;
            d.activate(n)?;
            format!("Profile {n} is active.")
        }
        Job::Mode(onboard) => {
            if onboard {
                d.release_rgb()?;
            }
            d.set_mode(onboard)?;
            if onboard {
                "Onboard mode active."
            } else {
                "Software mode active. Live settings last until reconnect or a mode/profile change."
            }
            .into()
        }
        Job::Dpi(n) => {
            d.set_dpi(n)?;
            format!("{n} DPI verified.")
        }
        Job::Rate(ms) => {
            d.set_rate(ms)?;
            format!("{} Hz verified.", 1000 / ms as u16)
        }
        Job::Stage(n) => {
            d.set_dpi_index(n)?;
            format!("DPI stage {} selected.", n + 1)
        }
        Job::Wheel(r, h) => {
            d.set_wheel(r, h)?;
            "Wheel settings verified.".into()
        }
        Job::Rgb(effect) => {
            d.set_rgb(effect)?;
            "Lighting acknowledged by mouse. Live lighting has no readback.".into()
        }
        Job::Leds(colors) => {
            d.set_leds(&colors)?;
            "Eight segment colors sent. Live lighting has no readback.".into()
        }
        Job::ReleaseRgb => {
            d.release_rgb()?;
            "Lighting returned to firmware.".into()
        }
        Job::Backup => format!("Full memory backup: {}", d.automatic_backup()?.display()),
        Job::Restore(p) => {
            let b: device::Backup = serde_json::from_slice(&std::fs::read(p)?)?;
            let backup = d.restore(&b)?;
            reset = true;
            format!(
                "Memory restored and verified. Pre-restore backup: {}",
                backup.display()
            )
        }
        Job::Export(p, path) => {
            device::write_json_new(
                &path,
                &crate::ProfileFile {
                    schema: 1,
                    unit_id: d.unit_id.clone(),
                    layout: d.layout()?,
                    profile: p,
                },
            )?;
            format!("Exported draft: {}", path.display())
        }
        Job::Import(p) => {
            let p: crate::ProfileFile = serde_json::from_slice(&std::fs::read(p)?)?;
            let s = d.snapshot()?;
            anyhow::ensure!(
                p.schema == 1 && p.unit_id == s.unit_id && p.layout == s.layout,
                "Profile file is for another mouse or layout"
            );
            p.profile
                .validate_settings(&s.dpi_choices, &s.rate_choices)?;
            anyhow::ensure!(
                s.profiles.iter().any(|x| x.sector == p.profile.sector),
                "Profile does not exist"
            );
            imported = Some(p.profile);
            "Imported into editor. Use Save to mouse to write it.".into()
        }
    };
    Ok(Reply {
        snapshot: d.snapshot()?,
        message,
        reset,
        imported,
        full: true,
        notification_error: None,
    })
}
pub fn run(path: Option<PathBuf>, settings: bool) -> Result<()> {
    let guard = crate::alerts::private_lock("gui.lock")?;
    match guard.try_lock() {
        Ok(()) => (),
        Err(std::fs::TryLockError::WouldBlock) => {
            crate::preferences::atomic_write(
                &device::state_dir()?.join("open-window"),
                if settings { b"settings" } else { b"open" },
            )?;
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }
    let _ = std::fs::remove_file(device::state_dir()?.join("open-window"));
    let (preferences, mut settings_message) = match crate::preferences::Preferences::load() {
        Ok(p) => (p, String::new()),
        Err(e) => (
            crate::preferences::Preferences::default(),
            format!("Could not load preferences: {e:#}"),
        ),
    };
    if preferences.background_enabled
        && let Err(e) = crate::monitor::start()
    {
        settings_message = format!("Could not start background monitor: {e:#}");
    }
    let icon =
        image::load_from_memory(include_bytes!("../packaging/g502x-control.png"))?.into_rgba8();
    let icon = egui::IconData {
        width: icon.width(),
        height: icon.height(),
        rgba: icon.into_raw(),
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("G502 X Control")
        .with_app_id("g502x-control")
        .with_icon(icon)
        .with_inner_size([1240.0, 860.0])
        .with_min_inner_size([840.0, 640.0]);
    if std::env::var_os("G502X_CAPTURE").is_some() {
        viewport = viewport.with_active(false).with_mouse_passthrough(true);
    }
    // Deterministic capture dimensions, without changing the user's window-manager settings.
    if std::env::var_os("G502X_CAPTURE").is_some()
        && let Ok(size) = std::env::var("G502X_CAPTURE_SIZE")
        && let Some((w, h)) = size.split_once('x')
        && let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>())
    {
        let size = [w.clamp(840, 2560) as f32, h.clamp(640, 1600) as f32];
        viewport = viewport
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_resizable(false);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "G502 X Control",
        options,
        Box::new(move |cc| {
            design::setup(&cc.egui_ctx);
            let (tx, jobs) = mpsc::channel();
            let (replies, rx) = mpsc::channel();
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let automatic = matches!(job, Job::Poll { .. });
                    let result = perform(path.as_deref(), job).map_err(|e| format!("{e:#}"));
                    if replies.send(WorkerReply { automatic, result }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            let app = App::new(tx, rx, preferences, settings_message, settings);
            app.tx.send(Job::Refresh)?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Could not open native window: {e}"))
}
impl App {
    fn new(
        tx: Sender<Job>,
        rx: Receiver<WorkerReply>,
        preferences: crate::preferences::Preferences,
        settings_message: String,
        settings: bool,
    ) -> Self {
        Self {
            picker: None,
            capture_requested: false,
            capture_settle: 0,
            profile_tab: std::env::var("G502X_CAPTURE_SECTION")
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or(0),
            details_open: false,
            function_data: 0,
            tx,
            rx,
            save_base: None,
            saved_preferences: preferences.clone(),
            preferences,
            settings_message,
            notification: None,
            poll_pending: false,
            next_poll: Instant::now(),
            last_full: Instant::now(),
            last_success: None,
            poll_failures: 0,
            connection_error: None,
            background_running: false,
            next_background_check: Instant::now(),
            snapshot: None,
            draft: None,
            name: String::new(),
            selected: 0,
            tab: std::env::var("G502X_CAPTURE_TAB")
                .ok()
                .and_then(|x| x.parse().ok())
                .unwrap_or(if settings { 4 } else { 0 }),
            busy: true,
            message: "Connecting to your mouse…".into(),
            error: false,
            shifted: false,
            edit_button: None,
            binding_type: 0,
            key: 4,
            modifiers: 0,
            media: 0xcd,
            function: 7,
            mouse_button: 1,
            color: [205, 244, 130],
            effect: 1,
            period: 5000,
            intensity: 100,
            leds: vec![[205, 244, 130]; 8],
            live_dpi: 800,
            live_rate: 1,
            reverse: false,
            hires: false,
            file_path: String::new(),
            restore_confirm: false,
            close_confirm: false,
        }
    }

    fn send(&mut self, job: Job) {
        if self.busy {
            return;
        }
        if self.tx.send(job).is_ok() {
            self.busy = true;
            self.error = false;
            self.message = "Updating your mouse. Keep it connected.".into();
        }
    }
    fn dirty(&self) -> bool {
        match (&self.save_base, &self.draft) {
            (Some(s), Some(p)) => {
                s.profiles.get(self.selected) != Some(p) || edit_name(p) != self.name
            }
            _ => false,
        }
    }
    fn select(&mut self, i: usize) {
        if let Some(s) = &self.snapshot {
            if s.profiles.is_empty() {
                return;
            }
            self.save_base = Some(s.clone());
            self.selected = i.min(s.profiles.len() - 1);
            let p = s.profiles[self.selected].clone();
            self.name = edit_name(&p);
            self.draft = Some(p);
            self.edit_button = None;
        }
    }
    fn accept(&mut self, r: Reply, automatic: bool) {
        if self.snapshot.is_none() {
            self.selected = r
                .snapshot
                .profiles
                .iter()
                .position(|p| p.sector == r.snapshot.status.active_sector)
                .unwrap_or(0);
        }
        let keep = !r.reset && (self.dirty() || automatic && self.edit_button.is_some());
        let old = self.snapshot.as_ref().map(|s| &s.status);
        merge_live(
            &mut self.live_dpi,
            old.map(|s| s.dpi),
            r.snapshot.status.dpi,
            automatic,
        );
        merge_live(
            &mut self.live_rate,
            old.map(|s| s.rate_ms),
            r.snapshot.status.rate_ms,
            automatic,
        );
        merge_live(
            &mut self.reverse,
            old.map(|s| s.wheel & 4 != 0),
            r.snapshot.status.wheel & 4 != 0,
            automatic,
        );
        merge_live(
            &mut self.hires,
            old.map(|s| s.wheel & 2 != 0),
            r.snapshot.status.wheel & 2 != 0,
            automatic,
        );
        self.snapshot = Some(r.snapshot);
        if !keep && (r.full || self.draft.is_none()) {
            self.select(self.selected);
        }
        if let Some(p) = r.imported {
            self.selected = self
                .snapshot
                .as_ref()
                .unwrap()
                .profiles
                .iter()
                .position(|x| x.sector == p.sector)
                .unwrap();
            self.save_base = self.snapshot.clone();
            self.name = edit_name(&p);
            self.draft = Some(p);
            self.tab = 1;
        }
        if !automatic {
            self.message = r.message;
            self.error = false;
        }
        if let Some(e) = r.notification_error {
            self.settings_message = e;
        }
    }
    fn choose_file(&mut self, action: FileAction, ctx: &egui::Context) {
        if self.picker.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.picker = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| -> PickerReply {
                let title = match action {
                    FileAction::Export => "Export G502 X profile",
                    FileAction::Import => "Import G502 X profile",
                    FileAction::Restore => "Restore G502 X backup",
                };
                #[cfg(target_os = "linux")]
                let mut path = {
                    let mut command = std::process::Command::new("zenity");
                    command.args(["--file-selection", "--file-filter=JSON files | *.json"]);
                    command.arg(format!("--title={title}"));
                    if matches!(action, FileAction::Export) {
                        command.args(["--save", "--filename=g502x-profile.json"]);
                    }
                    let output = command.output().map_err(|_| "The native file chooser is unavailable. Install zenity or enter a full path under Advanced file path instead.".to_owned())?;
                    if !output.status.success() {
                        return Ok(None);
                    }
                    let path = String::from_utf8(output.stdout)
                        .map_err(|_| "Could not read the selected file path".to_owned())?;
                    let path = PathBuf::from(path.trim_end_matches(['\n', '\r']));
                    if path.as_os_str().is_empty() {
                        return Ok(None);
                    }
                    path
                };
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                let mut path = {
                    let dialog = rfd::FileDialog::new()
                        .set_title(title)
                        .add_filter("JSON", &["json"]);
                    let path = if matches!(action, FileAction::Export) {
                        dialog.set_file_name("g502x-profile.json").save_file()
                    } else {
                        dialog.pick_file()
                    };
                    let Some(path) = path else {
                        return Ok(None);
                    };
                    path
                };
                if matches!(action, FileAction::Export) && path.extension().is_none() {
                    path.set_extension("json");
                }
                Ok(Some((path, action)))
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn selected_file(&mut self, path: PathBuf, action: FileAction) {
        self.file_path = path.display().to_string();
        match action {
            FileAction::Import => self.send(Job::Import(path)),
            FileAction::Restore => self.restore_confirm = true,
            FileAction::Export => {
                if let Some(mut p) = self.draft.clone() {
                    match apply_name(&mut p, &self.name) {
                        Ok(()) => self.send(Job::Export(p, path)),
                        Err(e) => {
                            self.error = true;
                            self.message = e.to_string();
                        }
                    }
                }
            }
        }
    }
    fn save(&mut self) {
        let (Some(s), Some(p)) = (&self.save_base, &self.draft) else {
            return;
        };
        let mut p = p.clone();
        match apply_name(&mut p, &self.name)
            .and_then(|_| p.validate_settings(&s.dpi_choices, &s.rate_choices))
        {
            Ok(()) => self.send(Job::Save(Box::new(s.clone()), p)),
            Err(e) => {
                self.message = e.to_string();
                self.error = true;
            }
        }
    }
    fn open_binding(&mut self, i: usize) {
        let Some(p) = &self.draft else { return };
        let b = p.binding(i, self.shifted);
        match (b[0], b[1]) {
            (0x80, 1) => {
                let n = u16::from_be_bytes([b[2], b[3]]);
                if n.is_power_of_two() {
                    self.binding_type = 0;
                    self.mouse_button = n.trailing_zeros() as u8 + 1;
                } else {
                    self.binding_type = 5;
                }
            }
            (0x80, 2) => {
                self.binding_type = 1;
                self.modifiers = b[2];
                self.key = b[3];
            }
            (0x80, 3) => {
                self.binding_type = 2;
                self.media = u16::from_be_bytes([b[2], b[3]]);
            }
            (0x90, 0) | (0x80, 0) => self.binding_type = 4,
            (0x90, f) => {
                self.binding_type = 3;
                self.function = f;
                self.function_data = b[3];
            }
            _ => self.binding_type = 5,
        }
        self.edit_button = Some(i);
    }
    fn overview(&mut self, ui: &mut egui::Ui) {
        let s = self.snapshot.as_ref().unwrap().clone();
        design::title(ui, "G502 X Plus", "Device overview");
        design::panel(ui, |ui| {
            ui.columns(3, |cols| {
                design::metric(
                    &mut cols[0],
                    "Sensitivity",
                    &s.status.dpi.to_string(),
                    "DPI",
                    "Current resolution",
                );
                design::metric(
                    &mut cols[1],
                    "Polling rate",
                    &(1000 / s.status.rate_ms.max(1) as u16).to_string(),
                    "Hz",
                    &format!("{} ms report interval", s.status.rate_ms),
                );
                design::metric(
                    &mut cols[2],
                    "Battery",
                    &s.status.battery.to_string(),
                    "%",
                    if (1..=4).contains(&s.status.charge_status) {
                        "Charging"
                    } else {
                        "On battery"
                    },
                );
            });
        });
        ui.add_space(12.0);
        ui.strong("Configuration");
        design::panel(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong("Control mode");
                    design::muted(ui, "Use saved profiles or live software settings.");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if design::tab(ui, "Software", s.status.mode == 2).clicked()
                        && s.status.mode != 2
                    {
                        self.send(Job::Mode(false));
                    }
                    if design::tab(ui, "Onboard", s.status.mode == 1).clicked()
                        && s.status.mode != 1
                    {
                        self.send(Job::Mode(true));
                    }
                });
            });
            design::separator(ui);
            if s.status.mode == 1 {
                ui.horizontal(|ui| {
                    ui.label(format!("Profile {}", s.status.active_sector));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(design::secondary("Edit profile")).clicked() {
                            if !self.dirty() {
                                let i = s
                                    .profiles
                                    .iter()
                                    .position(|p| p.sector == s.status.active_sector)
                                    .unwrap_or(0);
                                self.select(i);
                            }
                            self.tab = 1;
                        }
                    });
                });
                if let Some(p) = s
                    .profiles
                    .iter()
                    .find(|p| p.sector == s.status.active_sector)
                {
                    ui.horizontal_wrapped(|ui| {
                        design::muted(ui, "DPI stages");
                        for (i, dpi) in p.dpis().iter().enumerate() {
                            if *dpi > 0
                                && design::tab(ui, &format!("{dpi}"), s.status.dpi_index == i as u8)
                                    .clicked()
                            {
                                self.send(Job::Stage(i as u8));
                            }
                        }
                    });
                }
                design::muted(
                    ui,
                    "Onboard settings remain available when this app is closed.",
                );
            } else {
                ui.horizontal_wrapped(|ui| {
                    design::muted(ui, "Sensitivity");
                    ui.add(
                        egui::DragValue::new(&mut self.live_dpi)
                            .speed(50)
                            .range(100..=25600)
                            .suffix(" DPI"),
                    );
                    if ui.add(design::secondary("Apply DPI")).clicked() {
                        self.send(Job::Dpi(self.live_dpi));
                    }
                    ui.add_space(10.0);
                    rate_combo(ui, "live-rate", &mut self.live_rate, &s.rate_choices);
                    if ui.add(design::secondary("Apply rate")).clicked() {
                        self.send(Job::Rate(self.live_rate));
                    }
                });
                design::muted(
                    ui,
                    "Software settings may reset after reconnecting or changing modes.",
                );
            }
        });
        ui.add_space(12.0);
        ui.strong("Scroll wheel");
        design::panel(ui, |ui| {
            if ui.available_width() >= 660.0 {
                ui.columns(2, |cols| {
                    preference_toggle(&mut cols[0], &mut self.reverse, "Reverse direction");
                    preference_toggle(&mut cols[1], &mut self.hires, "High-resolution scrolling");
                });
            } else {
                preference_toggle(ui, &mut self.reverse, "Reverse direction");
                design::separator(ui);
                preference_toggle(ui, &mut self.hires, "High-resolution scrolling");
            }
            design::separator(ui);
            ui.horizontal(|ui| {
                design::muted(
                    ui,
                    "Use the physical wheel switch for free-spin or ratchet mode.",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            self.reverse != (s.status.wheel & 4 != 0)
                                || self.hires != (s.status.wheel & 2 != 0),
                            design::secondary("Apply changes"),
                        )
                        .clicked()
                    {
                        self.send(Job::Wheel(self.reverse, self.hires));
                    }
                });
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            design::muted(
                ui,
                format!(
                    "{} onboard profiles · {} assignable buttons",
                    s.profiles.len(),
                    s.layout.buttons
                ),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("Manage profiles →").color(ACCENT))
                            .frame(false),
                    )
                    .clicked()
                {
                    self.tab = 1;
                }
            });
        });
    }
    fn profiles(&mut self, ui: &mut egui::Ui) {
        let s = self.snapshot.as_ref().unwrap().clone();
        let dirty = self.dirty();
        design::title(
            ui,
            "Onboard profiles",
            "Stored in your mouse. Select a slot to edit its settings.",
        );
        let w = (ui.available_width() - 24.0) / s.profiles.len().max(1) as f32;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                for (i, p) in s.profiles.iter().enumerate() {
                    let chosen = i == self.selected;
                    let active = s.status.mode == 1 && s.status.active_sector == p.sector;
                    let label = format!(
                        "{}  {}",
                        i + 1,
                        if active {
                            "Active"
                        } else if p.enabled {
                            "Ready"
                        } else {
                            "Disabled"
                        }
                    );
                    let button =
                        egui::Button::new(RichText::new(label).size(12.0).color(if chosen {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .fill(if chosen {
                            design::SELECTED
                        } else {
                            design::SURFACE
                        })
                        .stroke(Stroke::new(
                            1.0,
                            if chosen { ACCENT } else { design::BORDER },
                        ))
                        .corner_radius(5);
                    if ui
                        .add_enabled(!dirty || chosen, button.min_size(vec2(w, 40.0)))
                        .on_disabled_hover_text(
                            "Save or discard your current edits before changing profiles.",
                        )
                        .clicked()
                    {
                        self.select(i);
                    }
                }
            });
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let p = self.draft.as_mut().unwrap();
            design::muted(ui, "Name");
            let width = (ui.available_width() - 150.0).clamp(150.0, 380.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .hint_text(format!("Profile {}", self.selected + 1))
                    .desired_width(width)
                    .margin(vec2(10.0, 8.0)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                design::toggle(ui, &mut p.enabled, "Enable this profile");
                design::muted(ui, "Enabled");
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (i, label) in ["Sensitivity", "Buttons", "Lighting", "Power"]
                .iter()
                .enumerate()
            {
                if design::section_tab(ui, label, self.profile_tab == i).clicked() {
                    self.profile_tab = i;
                }
            }
        });
        design::separator(ui);
        ui.add_space(4.0);
        match self.profile_tab {
            0 => self.sensitivity(ui, &s),
            1 => self.buttons(ui),
            2 => self.profile_lighting(ui, &s),
            _ => self.power(ui),
        }
    }
    fn sensitivity(&mut self, ui: &mut egui::Ui, s: &Snapshot) {
        let p = self.draft.as_mut().unwrap();
        design::panel(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                ui.strong("DPI stages");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design::muted(ui, "100–25,600 DPI");
                });
            });
            design::muted(
                ui,
                "Enable up to five stages for your DPI buttons to cycle through.",
            );
            design::separator(ui);
            let mut dpis = p.dpis();
            for (i, dpi) in dpis.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        let mut enabled = *dpi > 0;
                        if design::toggle(ui, &mut enabled, &format!("Enable stage {}", i + 1))
                            .changed()
                        {
                            *dpi = if enabled { 800 } else { 0 };
                        }
                        ui.label(RichText::new(format!("{}", i + 1)).color(MUTED));
                        ui.add_enabled_ui(enabled, |ui| {
                            ui.add_sized(
                                [90.0, 30.0],
                                egui::DragValue::new(dpi)
                                    .range(if enabled { 100..=25600 } else { 0..=0 })
                                    .speed(50)
                                    .suffix(" DPI"),
                            );
                            ui.spacing_mut().slider_width =
                                (ui.available_width() - 136.0).max(40.0);
                            // A disabled stage stays zero; drawing its slider must not enable it.
                            let mut value = (*dpi).max(100);
                            if ui
                                .add(
                                    egui::Slider::new(&mut value, 100..=25600)
                                        .logarithmic(true)
                                        .step_by(50.0)
                                        .show_value(false),
                                )
                                .on_hover_text(
                                    "Drag to set DPI; edit the number for an exact value",
                                )
                                .changed()
                            {
                                *dpi = value;
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if i == p.raw[2] as usize {
                                design::pill(ui, "Shift", MUTED);
                            }
                            if i == p.raw[1] as usize {
                                design::pill(ui, "Default", ACCENT);
                            }
                        });
                    });
                    if i < 4 {
                        design::separator(ui);
                    }
                });
            }
            p.set_dpis(dpis);
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                design::muted(ui, "Default stage");
                stage_combo(ui, "default-stage", &mut p.raw[1]);
                ui.add_space(8.0);
                design::muted(ui, "DPI-shift stage");
                stage_combo(ui, "shift-stage", &mut p.raw[2]);
            });
        });
        ui.add_space(4.0);
        design::panel(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong("Polling rate");
                    design::muted(ui, format!("{} ms between position updates", p.raw[0]));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    rate_combo(ui, "profile-rate", &mut p.raw[0], &s.rate_choices);
                });
            });
        });
    }
    fn buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            design::muted(ui, "Select a button to change its assignment.");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if design::tab(ui, "G-Shift", self.shifted).clicked() {
                    self.shifted = true;
                }
                if design::tab(ui, "Standard", !self.shifted).clicked() {
                    self.shifted = false;
                }
            });
        });
        let p = self.draft.as_ref().unwrap().clone();
        let mut clicked = None;
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            let left = (width * 0.32).clamp(160.0, 260.0);
            ui.allocate_ui_with_layout(
                vec2(left, 350.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(left, 300.0), Sense::click());
                    let hover = response
                        .hover_pos()
                        .and_then(|pos| design::mouse_hit(rect, pos));
                    design::mouse(
                        ui.painter(),
                        rect,
                        &[Color32::from_rgb(150, 180, 230); 8],
                        true,
                        hover.or(self.edit_button),
                    );
                    if response.clicked() {
                        clicked = hover;
                    }
                    design::muted(ui, "Button positions");
                    if self.shifted {
                        design::muted(ui, "While G-Shift is held");
                    }
                },
            );
            ui.vertical(|ui| {
                ui.set_width((width - left - 10.0).max(240.0));
                design::card().inner_margin(12).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.spacing_mut().interact_size.y = 26.0;
                    ui.spacing_mut().button_padding.y = 4.0;
                    for (i, name) in profile::BUTTON_NAMES.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:02}", i + 1))
                                    .monospace()
                                    .size(11.0)
                                    .color(DIM),
                            );
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(*name).size(12.0));
                                ui.label(
                                    RichText::new(profile::binding_label(
                                        p.binding(i, self.shifted),
                                    ))
                                    .size(11.0)
                                    .color(MUTED),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Edit").size(11.0).color(ACCENT),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text(format!("Edit {name}"))
                                        .clicked()
                                    {
                                        clicked = Some(i);
                                    }
                                },
                            );
                        });
                        if i + 1 < profile::BUTTON_NAMES.len() {
                            design::separator(ui);
                        }
                    }
                });
            });
        });
        if let Some(i) = clicked {
            self.open_binding(i);
        }
    }
    fn profile_lighting(&mut self, ui: &mut egui::Ui, s: &Snapshot) {
        design::panel(ui, |ui| {
            let p = self.draft.as_ref().unwrap();
            let stored = effect_name(p.lighting()[0] as u16);
            ui.horizontal(|ui| {
                ui.strong(format!("Saved lighting · {stored}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design::pill(
                        ui,
                        if p.raw[252] != 0 {
                            "Custom animation"
                        } else {
                            "Onboard"
                        },
                        MUTED,
                    );
                });
            });
            design::muted(
                ui,
                if p.raw[252] != 0 {
                    "This profile uses a custom animation. Choosing an effect replaces it."
                } else {
                    "Choose an effect to keep with this profile."
                },
            );
            lighting_editor(ui, preview_colors(self.effect, self.color), |ui| {
                lighting_controls(
                    ui,
                    &mut self.effect,
                    &mut self.color,
                    &mut self.period,
                    &mut self.intensity,
                    &s.effects,
                );
                ui.add_space(8.0);
                if ui
                    .add(design::secondary("Use effect in this profile"))
                    .clicked()
                {
                    match profile::effect_bytes(
                        self.effect,
                        self.color,
                        self.period,
                        self.intensity,
                    ) {
                        Ok(b) => self.draft.as_mut().unwrap().set_lighting(b),
                        Err(e) => {
                            self.error = true;
                            self.message = e.to_string();
                        }
                    }
                }
                design::muted(ui, "Applied when you save to mouse.");
            });
        });
        design::panel(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong("Profile indicator");
                    design::muted(ui, "The color associated with this profile.");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let p = self.draft.as_mut().unwrap();
                    let mut c = [p.raw[13], p.raw[14], p.raw[15]];
                    if ui.color_edit_button_srgb(&mut c).changed() {
                        p.raw[13..16].copy_from_slice(&c);
                    }
                });
            });
        });
    }
    fn power(&mut self, ui: &mut egui::Ui) {
        let p = self.draft.as_mut().unwrap();
        design::panel(ui, |ui| {
            ui.strong("Power management");
            design::muted(ui, "Idle behavior saved with this profile.");
            design::separator(ui);
            for (off, title, sub) in [
                (
                    false,
                    "Dim lighting",
                    "Time before entering power-saving mode.",
                ),
                (
                    true,
                    "Sleep",
                    "Time before entering the deeper power-off state.",
                ),
            ] {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(title);
                        design::muted(ui, sub);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut sec = p.timeout(off);
                        if ui
                            .add(egui::DragValue::new(&mut sec).speed(5).suffix(" sec"))
                            .changed()
                        {
                            p.set_timeout(off, sec);
                        }
                    });
                });
            }
        });
        design::panel(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong("Angle snapping");
                    design::muted(ui, "Assist with straighter pointer movements.");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut snap = p.raw[17] != 0;
                    if design::toggle(ui, &mut snap, "Angle snapping").changed() {
                        p.raw[17] = u8::from(snap);
                    }
                });
            });
        });
        design::muted(ui, "These advanced settings depend on your mouse firmware.");
    }
    fn lighting(&mut self, ui: &mut egui::Ui) {
        let s = self.snapshot.as_ref().unwrap().clone();
        design::title(
            ui,
            "Lighting",
            "Preview and apply software-controlled lighting.",
        );
        if s.status.mode != 2 {
            egui::Frame::new()
                .fill(design::SELECTED)
                .stroke(Stroke::new(1.0, design::BORDER))
                .corner_radius(6)
                .inner_margin(16)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.strong("Onboard lighting is active");
                            design::muted(ui, "Enable software control to apply live effects.");
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(design::primary("Use software mode")).clicked() {
                                self.send(Job::Mode(false));
                            }
                        });
                    });
                });
        }
        design::panel(ui, |ui| {
            lighting_editor(ui, preview_colors(self.effect, self.color), |ui| {
                ui.strong("Effect");
                lighting_controls(
                    ui,
                    &mut self.effect,
                    &mut self.color,
                    &mut self.period,
                    &mut self.intensity,
                    &s.effects,
                );
                ui.add_space(12.0);
                if ui
                    .add_enabled(s.status.mode == 2, design::primary("Apply live effect"))
                    .clicked()
                {
                    match profile::effect_bytes(
                        self.effect,
                        self.color,
                        self.period,
                        self.intensity,
                    ) {
                        Ok(b) => self.send(Job::Rgb(b)),
                        Err(e) => {
                            self.error = true;
                            self.message = e.to_string();
                        }
                    }
                }
                design::muted(ui, "Save persistent effects in Profiles → Lighting.");
            });
        });
        design::panel(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Individual segments");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design::pill(ui, "8 segments", MUTED);
                });
            });
            design::muted(ui, "Set a color for each of the eight lighting segments.");
            ui.columns(self.leds.len(), |columns| {
                for (i, c) in self.leds.iter_mut().enumerate() {
                    columns[i].push_id(i, |ui| {
                        egui::Frame::new().inner_margin(8).show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.color_edit_button_srgb(c);
                                ui.label(
                                    RichText::new(format!("{}", (b'A' + i as u8) as char))
                                        .size(10.0)
                                        .color(MUTED),
                                );
                            });
                        });
                    });
                }
            });
            if ui
                .add_enabled(
                    s.status.mode == 2,
                    design::secondary("Apply segment colors"),
                )
                .clicked()
            {
                self.send(Job::Leds(self.leds.clone()));
            }
            ui.label(
                RichText::new(
                    "Draft colors shown here. The mouse cannot report its current LED colors.",
                )
                .size(11.0)
                .color(DIM),
            );
        });
        if ui
            .add(design::secondary("Return lighting to onboard control"))
            .clicked()
        {
            self.send(Job::ReleaseRgb);
        }
    }
    fn files(&mut self, ui: &mut egui::Ui) {
        let s = self.snapshot.as_ref().unwrap().clone();
        design::title(
            ui,
            "Backups & device",
            "Save a copy of your configuration or restore an earlier setup.",
        );
        design::panel(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
                ui.painter().rect_filled(r, 6, design::SELECTED);
                design::icon(ui.painter(), r.shrink(10.0), Icon::Shield, ACCENT);
                ui.vertical(|ui| {
                    ui.strong("Automatic backups");
                    design::muted(
                        ui,
                        "Your profiles, macros and animations are backed up before changes.",
                    );
                });
            });
            ui.add_space(4.0);
            if ui.add(design::primary("Create backup")).clicked() {
                self.send(Job::Backup);
            }
            ui.collapsing("Backup location", |ui| {
                if let Ok(p) = device::state_dir() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(p.join("backups").display().to_string())
                                .size(12.0)
                                .color(MUTED),
                        )
                        .wrap(),
                    );
                }
            });
        });
        design::panel(ui, |ui| {
            ui.strong("Import & export");
            design::muted(ui, "Export a profile to keep, or bring back a saved setup.");
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(self.picker.is_none(), design::secondary("Export profile…"))
                    .clicked()
                {
                    self.choose_file(FileAction::Export, ui.ctx());
                }
                if ui
                    .add_enabled(
                        !self.dirty() && self.picker.is_none(),
                        design::secondary("Import profile…"),
                    )
                    .clicked()
                {
                    self.choose_file(FileAction::Import, ui.ctx());
                }
                if ui
                    .add_enabled(
                        !self.dirty() && self.picker.is_none(),
                        design::secondary("Restore backup…"),
                    )
                    .clicked()
                {
                    self.choose_file(FileAction::Restore, ui.ctx());
                }
            });
            if self.picker.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    design::muted(ui, "Choose a file in the file picker.");
                });
            }
            egui::CollapsingHeader::new("Advanced file path")
                .default_open(!self.file_path.is_empty())
                .show(ui, |ui| {
                    design::muted(ui, "You can also drop a JSON file into this window.");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.file_path)
                            .hint_text("Full path to a profile or backup JSON file")
                            .desired_width(f32::INFINITY)
                            .margin(vec2(12.0, 10.0)),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let has_path = !self.file_path.trim().is_empty();
                        if ui
                            .add_enabled(has_path, design::secondary("Export here"))
                            .clicked()
                        {
                            self.selected_file(self.file_path.clone().into(), FileAction::Export);
                        }
                        if ui
                            .add_enabled(
                                has_path && !self.dirty(),
                                design::secondary("Import file"),
                            )
                            .clicked()
                        {
                            self.selected_file(self.file_path.clone().into(), FileAction::Import);
                        }
                        if ui
                            .add_enabled(
                                has_path && !self.dirty(),
                                design::secondary("Restore file"),
                            )
                            .clicked()
                        {
                            self.selected_file(self.file_path.clone().into(), FileAction::Restore);
                        }
                    });
                });
            ui.label(
                RichText::new(
                    "Imported profiles open as a draft. Existing files are never overwritten.",
                )
                .size(11.0)
                .color(DIM),
            );
        });
        design::panel(ui, |ui| {
            ui.strong("Device information");
            ui.spacing_mut().interact_size.y = 20.0;
            egui::Grid::new("device-info")
                .num_columns(2)
                .spacing([40.0, 8.0])
                .show(ui, |ui| {
                    for (k, v) in [
                        ("Model", "G502 X Plus".to_owned()),
                        ("Connection", connection(&s).to_owned()),
                        ("Onboard memory", format!("{} profiles", s.profiles.len())),
                        ("Buttons", format!("{} + G-Shift layer", s.layout.buttons)),
                        ("Lighting", format!("{} segments", s.led_ids.len())),
                    ] {
                        ui.label(RichText::new(k).size(13.0).color(MUTED));
                        ui.label(v);
                        ui.end_row();
                    }
                });
            ui.collapsing("Technical details", |ui| {
                ui.monospace(format!(
                    "{} · HID++ {} · Unit {}",
                    s.endpoint.path.display(),
                    s.protocol,
                    s.unit_id
                ));
                for f in &s.features {
                    ui.monospace(format!(
                        "{:04X}  v{}  {}",
                        f.id,
                        f.version,
                        feature_name(f.id)
                    ));
                }
            });
        });
        design::muted(
            ui,
            "Existing macros and custom animations are preserved. Creating them is not supported yet.",
        );
    }
    fn binding_window(&mut self, ctx: &egui::Context) {
        let Some(index) = self.edit_button else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        egui::Window::new(format!("{} · {}",profile::BUTTON_NAMES[index],if self.shifted{"G-Shift layer"}else{"Standard layer"})).open(&mut open).resizable(false).collapsible(false).default_width(470.0).show(ctx,|ui|{
            design::muted(ui,"Choose what this button does. Save the profile when you're done.");
            ui.horizontal_wrapped(|ui|{for (i,name) in ["Mouse","Shortcut","Media","Device","Disable"].iter().enumerate(){if design::tab(ui,name,self.binding_type==i).clicked(){self.binding_type=i;}}});
            design::separator(ui);
            match self.binding_type {
                0=>{ui.label("Mouse action");egui::ComboBox::from_id_salt("mouse-action").selected_text(mouse_action(self.mouse_button)).width(280.0).show_ui(ui,|ui|{for n in 1..=16{ui.selectable_value(&mut self.mouse_button,n,mouse_action(n));}});},
                1=>{ui.label("Key");egui::ComboBox::from_id_salt("shortcut-key").selected_text(profile::key_name(self.key)).width(280.0).show_ui(ui,|ui|{for k in 4..=82{ui.selectable_value(&mut self.key,k,profile::key_name(k));}});ui.label(RichText::new("Combine with").size(12.0).color(MUTED));ui.horizontal_wrapped(|ui|{for (bit,name) in [(1,"Ctrl"),(2,"Shift"),(4,"Alt"),(8,"Super")]{let mut v=self.modifiers&bit!=0;if ui.checkbox(&mut v,name).changed(){if v{self.modifiers|=bit;}else{self.modifiers&=!bit;}}}});ui.collapsing("Advanced key options",|ui|{ui.horizontal(|ui|{ui.label("USB HID usage");ui.add(egui::DragValue::new(&mut self.key));});ui.horizontal_wrapped(|ui|{for (bit,name) in [(16,"Right Ctrl"),(32,"Right Shift"),(64,"Right Alt"),(128,"Right Super")]{let mut v=self.modifiers&bit!=0;if ui.checkbox(&mut v,name).changed(){if v{self.modifiers|=bit;}else{self.modifiers&=!bit;}}}});});},
                2=>{ui.label("Media action");egui::ComboBox::from_id_salt("media-action").selected_text(media_action(self.media)).width(280.0).show_ui(ui,|ui|{for code in [0xcd,0xb5,0xb6,0xb7,0xe2,0xe9,0xea]{ui.selectable_value(&mut self.media,code,media_action(code));}});ui.collapsing("Custom consumer usage",|ui|{ui.add(egui::DragValue::new(&mut self.media));});},
                3=>{ui.label("Mouse function");egui::ComboBox::from_id_salt("device-action").selected_text(profile::function_name(self.function)).width(280.0).show_ui(ui,|ui|{for f in 0..=17{ui.selectable_value(&mut self.function,f,profile::function_name(f));}});if self.function==13{ui.horizontal(|ui|{ui.label("Profile");ui.add(egui::DragValue::new(&mut self.function_data).range(1..=5));});}},
                4=>{design::muted(ui,"This button will have no action in the selected layer.");},
                _=>{design::muted(ui,"This button has a custom assignment. Choose a category above to replace it, or close this window to keep it.");}
            }
            ui.add_space(8.0);design::separator(ui);ui.horizontal(|ui|{if ui.add(design::primary("Use assignment")).clicked(){apply=true;}ui.label(RichText::new("Changes stay in your draft until saved").size(11.0).color(MUTED));});
        });
        if apply {
            let binding = match self.binding_type {
                0 => {
                    let b = (1u16 << (self.mouse_button - 1)).to_be_bytes();
                    [0x80, 1, b[0], b[1]]
                }
                1 => [0x80, 2, self.modifiers, self.key],
                2 => {
                    let b = self.media.to_be_bytes();
                    [0x80, 3, b[0], b[1]]
                }
                3 => [
                    0x90,
                    self.function,
                    0xff,
                    if self.function == 13 {
                        self.function_data.clamp(1, 5)
                    } else {
                        0
                    },
                ],
                4 => [0x80, 0, 0xff, 0xff],
                _ => self.draft.as_ref().unwrap().binding(index, self.shifted),
            };
            if let Some(p) = self.draft.as_mut()
                && let Err(e) = p.set_binding(index, self.shifted, binding)
            {
                self.error = true;
                self.message = e.to_string();
            }
            self.edit_button = None;
        }
        if !open {
            self.edit_button = None;
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        design::title(ui, "Settings", "Preferences for this computer.");
        ui.spacing_mut().item_spacing.y = 8.0;
        self.live_preferences(ui);
        ui.add_space(4.0);
        self.alert_preferences(ui, ctx);
        ui.add_space(4.0);
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.strong("Background & startup");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    design::pill(
                        ui,
                        if self.background_running {
                            "Running"
                        } else {
                            "Stopped"
                        },
                        if self.background_running {
                            design::SUCCESS
                        } else {
                            MUTED
                        },
                    );
                });
            });
            ui.add_space(4.0);
            if preference_toggle(
                ui,
                &mut self.preferences.background_enabled,
                "Keep running in the system tray",
            ) && !self.preferences.background_enabled
            {
                self.preferences.launch_at_login = false;
            }
            design::muted(
                ui,
                "Check the battery once a minute when the window is closed.",
            );
            ui.add_space(4.0);
            design::separator(ui);
            ui.add_space(4.0);
            if preference_toggle(
                ui,
                &mut self.preferences.launch_at_login,
                "Launch when I sign in",
            ) && self.preferences.launch_at_login
            {
                self.preferences.background_enabled = true;
            }
            design::muted(ui, "Start in the tray, with no window.");
            if self.background_running || self.saved_preferences.background_enabled {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if self.background_running {
                        if ui.add(design::secondary("Quit monitor for now")).clicked() {
                            self.settings_message = match crate::monitor::quit() {
                                Ok(()) => {
                                    "Monitor stopping. Your startup preference is saved.".into()
                                }
                                Err(e) => format!("{e:#}"),
                            };
                        }
                    } else if ui.add(design::secondary("Start monitor")).clicked() {
                        self.settings_message = match crate::monitor::start() {
                            Ok(()) => "Starting the background monitor…".into(),
                            Err(e) => format!("{e:#}"),
                        };
                    }
                    if self.background_running
                        && let Some(h) = crate::monitor::health()
                    {
                        design::muted(
                            ui,
                            format!(
                                "Last check {}s ago",
                                crate::alerts::now().saturating_sub(h.last_checked)
                            ),
                        );
                        if let Some(error) = h.error {
                            ui.colored_label(design::ERROR, "Needs attention")
                                .on_hover_text(error);
                        }
                    }
                });
            }
            #[cfg(target_os = "linux")]
            ui.label(
                RichText::new("On GNOME, enable AppIndicator support to see tray icons.")
                    .size(11.0)
                    .color(DIM),
            );
        });
        if self.background_running
            && let Some(e) = crate::monitor::tray_error()
        {
            ui.colored_label(design::ERROR, format!("Tray unavailable: {e}"));
        }
        if !self.settings_message.is_empty() {
            ui.add_space(6.0);
            ui.add(egui::Label::new(&self.settings_message).wrap());
        }
    }
    fn live_preferences(&mut self, ui: &mut egui::Ui) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());

            preference_toggle(
                ui,
                &mut self.preferences.live_updates,
                "Update automatically",
            );
            design::muted(
                ui,
                "Refresh device readings without changing your unsaved edits.",
            );
            ui.add_space(8.0);
            ui.add_enabled_ui(self.preferences.live_updates, |ui| {
                ui.horizontal(|ui| {
                    design::muted(ui, "Refresh interval");
                    ui.spacing_mut().slider_width = 155.0;
                    ui.add(
                        egui::Slider::new(&mut self.preferences.refresh_seconds, 1..=10)
                            .suffix(" sec"),
                    );
                });
            });
        });
    }
    fn alert_preferences(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());

            preference_toggle(
                ui,
                &mut self.preferences.battery_alerts,
                "Low-battery notification",
            );
            design::muted(
                ui,
                "Notify once when the battery reaches the threshold. Disabled while charging.",
            );
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(self.preferences.battery_alerts, |ui| {
                    design::muted(ui, "Notify at");
                    ui.spacing_mut().slider_width = 90.0;
                    ui.add(
                        egui::Slider::new(&mut self.preferences.low_battery_percent, 5..=50)
                            .suffix("%"),
                    );
                });
                if ui
                    .add_enabled(self.notification.is_none(), design::secondary("Send test"))
                    .on_hover_text("Send a test desktop notification")
                    .clicked()
                {
                    let (tx, rx) = mpsc::channel();
                    self.notification = Some(rx);
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let result = crate::alerts::show(
                            "G502 X Control",
                            "Battery alerts are ready. This is a test notification.",
                        )
                        .map_err(|e| format!("{e:#}"));
                        let _ = tx.send(result);
                        ctx.request_repaint();
                    });
                }
            });
        });
    }
    fn save_preferences(&mut self) {
        match crate::preferences::apply(&self.preferences) {
            Ok(()) => {
                self.saved_preferences = self.preferences.clone();
                self.next_poll = Instant::now();
                self.settings_message = "Preferences saved.".into();
            }
            Err(e) => self.settings_message = format!("Could not save preferences: {e:#}"),
        }
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(188.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(design::SIDEBAR).inner_margin(14))
            .show(ctx, |ui| {
                ui.add_space(15.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(27.0, 32.0), Sense::hover());
                    design::icon(ui.painter(), r.shrink(4.0), Icon::Mouse, TEXT);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(RichText::new("G502 X").size(18.0).strong());
                        ui.label(RichText::new("Control").size(12.0).color(MUTED));
                    });
                });
                ui.add_space(30.0);
                ui.spacing_mut().item_spacing.y = 4.0;
                for (i, (label, icon)) in [
                    ("Overview", Icon::Grid),
                    ("Profiles", Icon::Sliders),
                    ("Lighting", Icon::Sun),
                ]
                .iter()
                .enumerate()
                {
                    if design::nav(ui, label, *icon, self.tab == i) {
                        self.tab = i;
                    }
                }
                ui.add_space(16.0);
                design::separator(ui);
                ui.add_space(16.0);
                for (i, (label, icon)) in [
                    ("Backups & device", Icon::Shield),
                    ("Settings", Icon::Settings),
                ]
                .iter()
                .enumerate()
                {
                    if design::nav(ui, label, *icon, self.tab == i + 3) {
                        self.tab = i + 3;
                    }
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .size(10.0)
                            .color(DIM),
                    );
                    ui.add_space(12.0);
                    if ui
                        .add_enabled(
                            !self.busy && !self.dirty(),
                            design::secondary("Refresh device"),
                        )
                        .on_disabled_hover_text("Save or discard edits before refreshing.")
                        .clicked()
                    {
                        self.send(Job::Refresh);
                    }
                    ui.add_space(14.0);
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 67.0),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            ui.spacing_mut().item_spacing.y = 7.0;
                            ui.spacing_mut().interact_size.y = 16.0;
                            if self.connection_error.is_some() {
                                design::status(ui, "Reconnecting…", design::ERROR);
                            } else if self.saved_preferences.live_updates
                                && self.last_success.is_some()
                            {
                                design::status(ui, "Connected · Live", design::SUCCESS);
                            } else if self.snapshot.is_some() {
                                design::status(ui, "Live updates off", DIM);
                            } else {
                                design::status(ui, "Connecting…", DIM);
                            }
                            if let Some(s) = &self.snapshot {
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    design::icon(
                                        ui.painter(),
                                        r,
                                        if s.endpoint.product == 0xc095 {
                                            Icon::Link
                                        } else {
                                            Icon::Radio
                                        },
                                        DIM,
                                    );
                                    ui.label(RichText::new(connection(s)).size(11.0).color(MUTED));
                                });
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    design::icon(ui.painter(), r, Icon::Battery, DIM);
                                    ui.label(
                                        RichText::new(format!(
                                            "{}%{}",
                                            s.status.battery,
                                            if (1..=4).contains(&s.status.charge_status) {
                                                " · Charging"
                                            } else {
                                                " battery"
                                            }
                                        ))
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                });
                            }
                        },
                    );
                    ui.add_space(12.0);
                    design::separator(ui);
                });
            });
    }
    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("save-bar")
            .frame(
                egui::Frame::new()
                    .fill(design::SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(24, 8)),
            )
            .show(ctx, |ui| {
                let dirty = if self.tab == 4 {
                    self.preferences != self.saved_preferences
                } else {
                    self.dirty()
                };
                ui.horizontal(|ui| {
                    let available = ui.available_width();
                    ui.allocate_ui_with_layout(
                        vec2((available - 310.0).max(160.0), 36.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if self.busy {
                                ui.spinner();
                            } else {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(17.0, 17.0), Sense::hover());
                                if self.error || dirty {
                                    ui.painter().circle_filled(
                                        r.center(),
                                        3.0,
                                        if self.error {
                                            design::ERROR
                                        } else {
                                            design::WARNING
                                        },
                                    );
                                } else {
                                    design::icon(ui.painter(), r, Icon::Check, ACCENT);
                                }
                            }
                            let label = if self.tab == 4 {
                                if dirty {
                                    "Unsaved preferences".into()
                                } else {
                                    "Preferences saved on this computer".into()
                                }
                            } else if self.busy {
                                "Updating your mouse…".into()
                            } else if self.error {
                                "Couldn't complete that action".into()
                            } else if dirty {
                                format!("Unsaved edits · Profile {}", self.selected + 1)
                            } else {
                                self.message.clone()
                            };
                            let response = ui.add(
                                egui::Label::new(
                                    RichText::new(label).size(12.0).color(if self.error {
                                        design::ERROR
                                    } else {
                                        MUTED
                                    }),
                                )
                                .truncate()
                                .sense(Sense::click()),
                            );
                            if response.on_hover_text(&self.message).clicked() {
                                self.details_open = true;
                            }
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.tab == 4 {
                            if ui
                                .add_enabled(dirty, design::primary("Save preferences"))
                                .clicked()
                            {
                                self.save_preferences();
                            }
                            if dirty && ui.add(design::secondary("Reset")).clicked() {
                                self.preferences = self.saved_preferences.clone();
                            }
                        } else if dirty {
                            if ui
                                .add_enabled(!self.busy, design::primary("Save to mouse"))
                                .clicked()
                            {
                                self.save();
                            }
                            if ui
                                .add_enabled(!self.busy, design::secondary("Discard"))
                                .clicked()
                            {
                                self.select(self.selected);
                            }
                        } else if self.tab == 1
                            && let (Some(s), Some(p)) = (&self.snapshot, &self.draft)
                        {
                            if s.status.active_sector != p.sector || s.status.mode != 1 {
                                if ui
                                    .add_enabled(
                                        !self.busy && p.enabled,
                                        design::primary("Activate profile"),
                                    )
                                    .clicked()
                                {
                                    self.send(Job::Activate(p.sector));
                                }
                            } else {
                                design::pill(ui, "Active profile", design::SUCCESS);
                            }
                        } else if ui
                            .add(
                                egui::Button::new(RichText::new("Details").size(11.0).color(DIM))
                                    .frame(false),
                            )
                            .clicked()
                        {
                            self.details_open = true;
                        }
                    });
                });
            });
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(reply) = self.rx.try_recv() {
            if reply.automatic {
                self.poll_pending = false;
            } else {
                self.busy = false;
            }
            match reply.result {
                Ok(r) => {
                    self.last_success = Some(Instant::now());
                    self.connection_error = None;
                    self.poll_failures = 0;
                    if r.full {
                        self.last_full = Instant::now();
                    }
                    if !reply.automatic
                        || self.saved_preferences.live_updates
                        || self.snapshot.is_none()
                    {
                        self.accept(r, reply.automatic);
                    } else if let Some(e) = r.notification_error {
                        self.settings_message = e;
                    }
                    let seconds = if self.saved_preferences.live_updates {
                        self.saved_preferences.refresh_seconds
                    } else {
                        60
                    };
                    self.next_poll = Instant::now() + Duration::from_secs(seconds);
                }
                Err(e) => {
                    self.connection_error = Some(e.clone());
                    self.poll_failures = self.poll_failures.saturating_add(1);
                    self.next_poll = Instant::now()
                        + Duration::from_secs((1u64 << self.poll_failures.min(5)).min(30));
                    if !reply.automatic {
                        self.message = e;
                        self.error = true;
                    }
                }
            }
        }
        if !self.busy
            && !self.poll_pending
            && Instant::now() >= self.next_poll
            && (self.saved_preferences.live_updates || self.saved_preferences.battery_alerts)
        {
            let job = Job::Poll {
                cached: self.snapshot.clone().map(Box::new),
                full: self.saved_preferences.live_updates
                    && self.last_full.elapsed() >= Duration::from_secs(30),
                live: self.saved_preferences.live_updates,
            };
            self.poll_pending = self.tx.send(job).is_ok();
        }
        if let Some(result) = self.notification.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.notification = None;
            self.settings_message = match result {
                Ok(()) => {
                    "Test sent. Your desktop controls notification visibility and sound.".into()
                }
                Err(e) => e,
            };
        }
        if Instant::now() >= self.next_background_check {
            self.background_running = crate::monitor::running().unwrap_or(false);
            // Tray/CLI changes are reflected without overwriting unsaved preferences.
            if let Ok(p) = crate::preferences::Preferences::load() {
                if self.preferences == self.saved_preferences {
                    self.preferences = p.clone();
                }
                self.saved_preferences = p;
            }
            self.next_background_check = Instant::now() + Duration::from_secs(2);
        }
        if let Ok(dir) = device::state_dir() {
            let request = dir.join("open-window");
            if let Ok(value) = std::fs::read_to_string(&request) {
                let _ = std::fs::remove_file(request);
                if value == "settings" {
                    self.tab = 4;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        if let Some(result) = self.picker.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.picker = None;
            match result {
                Ok(Some((path, action))) => self.selected_file(path, action),
                Ok(None) => (),
                Err(e) => {
                    self.error = true;
                    self.message = e;
                    self.details_open = true;
                }
            }
        }
        for file in ctx.input(|i| i.raw.dropped_files.clone()) {
            if let Some(path) = file.path {
                self.file_path = path.display().to_string();
                self.tab = 3;
            }
        }
        if let Ok(path) = std::env::var("G502X_CAPTURE") {
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        &path,
                        &bytes,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Screenshot failed: {e}");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if (self.snapshot.is_some() || self.tab == 4) && !self.busy && !self.capture_requested {
                self.capture_settle = self.capture_settle.saturating_add(1);
                if self.capture_settle >= 3
                    && ctx.input(|i| i.time)
                        >= std::env::var("G502X_CAPTURE_DELAY")
                            .ok()
                            .and_then(|n| n.parse::<f64>().ok())
                            .unwrap_or(0.0)
                            .clamp(0.0, 30.0)
                {
                    self.capture_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
                ctx.request_repaint();
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && (self.busy || self.dirty()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_confirm = true;
        }
        self.sidebar(ctx);
        self.footer(ctx);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(design::BG).inner_margin(egui::Margin{left:30,right:30,top:30,bottom:24})).show(ctx,|ui|{
            egui::ScrollArea::vertical().id_salt(("page",self.tab,self.profile_tab)).auto_shrink([false,false]).show(ui,|ui|{ui.add_enabled_ui(!self.busy || self.tab == 4,|ui|{
                if self.tab == 4 { self.settings(ui, ctx); } else if self.snapshot.is_some(){
                    if let Some(error) = &self.connection_error {
                        ui.colored_label(design::ERROR, "Connection lost · Showing the last reading. Reconnecting automatically…").on_hover_text(error);
                        ui.add_space(12.0);
                    }
                    match self.tab{0=>self.overview(ui),1=>self.profiles(ui),2=>self.lighting(ui),_=>self.files(ui)}}else{
                    design::title(ui,if self.error{"Device disconnected"}else{"Connecting to your mouse"},"Connect your G502 X Plus with its USB cable or LIGHTSPEED receiver.");design::card().show(ui,|ui|{let (r,_)=ui.allocate_exact_size(vec2(ui.available_width(),340.0),Sense::hover());design::mouse(ui.painter(),r,&[design::DIM;8],false,None);ui.vertical_centered(|ui|{if self.error{ui.add(egui::Label::new(RichText::new(&self.message).color(design::ERROR)).wrap());if ui.add(design::primary("Try again")).clicked(){self.send(Job::Refresh);}}else{ui.spinner();design::muted(ui,"Reading your saved settings…");}});});
                }
            });});
        });
        if !self.busy {
            self.binding_window(ctx);
        }
        if self.details_open {
            egui::Window::new("Activity details")
                .open(&mut self.details_open)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new(&self.message).wrap());
                });
        }
        if self.restore_confirm {
            egui::Window::new("Restore this backup?").collapsible(false).resizable(false).default_width(460.0).show(ctx,|ui|{design::muted(ui,"This replaces the profiles, macros and animations stored in your mouse. We'll back up your current setup first.");ui.add_space(10.0);ui.horizontal(|ui|{if ui.add(design::primary("Restore backup")).clicked(){self.restore_confirm=false;self.send(Job::Restore(self.file_path.clone().into()));}
if ui.add(design::secondary("Cancel")).clicked(){self.restore_confirm=false;}});});
        }
        if self.close_confirm {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if self.busy {
                        design::muted(
                            ui,
                            "Your mouse is still updating. Keep it connected until this finishes.",
                        );
                    } else if self.dirty() {
                        design::muted(ui, "There are unsaved changes in this profile.");
                        ui.horizontal(|ui| {
                            if ui.add(design::primary("Save changes")).clicked() {
                                self.close_confirm = false;
                                self.save();
                            }
                            if ui.add(design::secondary("Discard and close")).clicked() {
                                self.draft = None;
                                self.close_confirm = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    } else {
                        self.close_confirm = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.add(design::secondary("Keep editing")).clicked() {
                        self.close_confirm = false;
                    }
                });
        }
        if self.busy {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }
}
fn preference_toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            design::toggle(ui, value, label).changed()
        })
        .inner
    })
    .inner
}
fn merge_live<T: Copy + PartialEq>(
    current: &mut T,
    previous: Option<T>,
    incoming: T,
    automatic: bool,
) {
    if !automatic || previous.is_none_or(|old| old == *current) {
        *current = incoming;
    }
}
fn edit_name(p: &Profile) -> String {
    let name = p.name();
    if name == "PROFILE_NAME_DEFAULT" {
        String::new()
    } else {
        name
    }
}
fn apply_name(p: &mut Profile, name: &str) -> Result<()> {
    if p.name() == "PROFILE_NAME_DEFAULT" && name.is_empty() {
        Ok(())
    } else {
        p.set_name(name)
    }
}
fn connection(s: &Snapshot) -> &'static str {
    if s.endpoint.product == 0xc095 {
        "USB cable"
    } else {
        "LIGHTSPEED"
    }
}
fn stage_combo(ui: &mut egui::Ui, id: &str, n: &mut u8) {
    egui::ComboBox::from_id_salt(id)
        .width(75.0)
        .selected_text(format!("Stage {}", *n + 1))
        .show_ui(ui, |ui| {
            for i in 0..5 {
                ui.selectable_value(n, i, format!("Stage {}", i + 1));
            }
        });
}
fn rate_combo(ui: &mut egui::Ui, id: &str, ms: &mut u8, choices: &[u8]) {
    egui::ComboBox::from_id_salt(id)
        .width(110.0)
        .selected_text(format!("{} Hz", 1000 / (*ms).max(1) as u16))
        .show_ui(ui, |ui| {
            for n in choices {
                ui.selectable_value(ms, *n, format!("{} Hz", 1000 / (*n) as u16));
            }
        });
}
/// Both lighting pages share the same preview and responsive control layout.
fn lighting_editor(ui: &mut egui::Ui, colors: [Color32; 8], content: impl FnOnce(&mut egui::Ui)) {
    let preview = |ui: &mut egui::Ui, width: f32, height: f32| {
        let (r, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        design::mouse(ui.painter(), r, &colors, false, None);
        ui.label(RichText::new("Preview only").size(11.0).color(DIM));
    };
    if ui.available_width() >= 680.0 {
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(240.0, 245.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    preview(ui, 240.0, 220.0);
                },
            );
            ui.vertical(|ui| {
                ui.set_width(width - 250.0);
                ui.add_space(12.0);
                content(ui);
            });
        });
    } else {
        ui.vertical_centered(|ui| {
            preview(ui, ui.available_width(), 190.0);
        });
        content(ui);
    }
}

fn lighting_controls(
    ui: &mut egui::Ui,
    effect: &mut u16,
    color: &mut [u8; 3],
    period: &mut u16,
    intensity: &mut u8,
    effects: &[device::Effect],
) {
    ui.horizontal_wrapped(|ui| {
        for e in effects {
            if [0, 1, 3, 10, 21, 22].contains(&e.id)
                && design::tab(ui, &effect_name(e.id), *effect == e.id).clicked()
            {
                *effect = e.id;
            }
        }
    });
    if [1, 10].contains(effect) {
        ui.horizontal_wrapped(|ui| {
            design::muted(ui, "Color");
            ui.color_edit_button_srgb(color);
            ui.label(
                RichText::new(format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]))
                    .monospace()
                    .size(12.0)
                    .color(MUTED),
            );
            for c in [
                [205, 244, 130],
                [139, 183, 255],
                [198, 159, 255],
                [255, 161, 174],
                [255, 207, 128],
                [242, 245, 249],
            ] {
                let b = egui::Button::new(" ")
                    .fill(Color32::from_rgb(c[0], c[1], c[2]))
                    .stroke(Stroke::NONE)
                    .corner_radius(6)
                    .min_size(vec2(24.0, 24.0));
                if ui
                    .add_sized([26.0, 26.0], b)
                    .on_hover_text(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
                    .clicked()
                {
                    *color = c;
                }
            }
        });
    }
    if [3, 10, 21, 22].contains(effect) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().slider_width = 110.0;
            design::muted(ui, "Cycle duration");
            ui.add(
                egui::DragValue::new(period)
                    .range(100..=60000)
                    .speed(100)
                    .suffix(" ms"),
            );
            design::muted(ui, "Brightness");
            ui.add(egui::Slider::new(intensity, 0..=100).suffix("%"));
        });
    }
}
fn preview_colors(effect: u16, color: [u8; 3]) -> [Color32; 8] {
    if effect == 0 {
        [Color32::from_rgb(57, 63, 72); 8]
    } else if [3, 21, 22].contains(&effect) {
        [
            [150, 174, 255],
            [163, 137, 255],
            [205, 133, 245],
            [248, 144, 184],
            [255, 174, 143],
            [242, 208, 132],
            [188, 233, 150],
            [131, 226, 208],
        ]
        .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
    } else {
        design::colors(color)
    }
}
fn effect_name(id: u16) -> String {
    match id {
        0 => "Off".into(),
        1 => "Static".into(),
        3 | 21 => "Spectrum".into(),
        10 => "Breathing".into(),
        22 => "Wave".into(),
        n => format!("Custom ({n})"),
    }
}
fn mouse_action(n: u8) -> String {
    match n {
        1 => "Left click".into(),
        2 => "Right click".into(),
        3 => "Middle click".into(),
        4 => "Back".into(),
        5 => "Forward".into(),
        n => format!("Mouse button {n}"),
    }
}
fn media_action(n: u16) -> String {
    match n {
        0xcd => "Play / pause".into(),
        0xb5 => "Next track".into(),
        0xb6 => "Previous track".into(),
        0xb7 => "Stop".into(),
        0xe2 => "Mute".into(),
        0xe9 => "Volume up".into(),
        0xea => "Volume down".into(),
        n => format!("Custom media key ({n})"),
    }
}
fn feature_name(id: u16) -> &'static str {
    match id {
        0 => "Root",
        1 => "Feature set",
        3 => "Device identity",
        5 => "Device name",
        0x1004 => "Battery",
        0x2201 => "Adjustable DPI",
        0x2121 => "Scroll wheel",
        0x8071 => "RGB effects",
        0x8081 => "LED segments",
        0x8100 => "Onboard profiles",
        0x8060 => "Report rate",
        0x8110 => "Button reporting",
        0x1500 => "Pairing",
        0xd0 => "Firmware update",
        _ => "Vendor feature",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Snapshot {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: vec![0; 255],
        };
        p.set_name("Original").unwrap();
        Snapshot {
            schema: 1,
            unit_id: "test-device".into(),
            protocol: "4.2".into(),
            endpoint: crate::transport::Endpoint {
                path: "test".into(),
                product: 0xc095,
                serial: String::new(),
                index: 255,
                name: "test".into(),
            },
            features: vec![],
            layout: device::Layout {
                memory: 1,
                format: 5,
                macro_format: 1,
                profiles: 1,
                buttons: 11,
                sectors: 16,
                size: 255,
                g_shift: true,
            },
            status: device::Status {
                battery: 80,
                charge_status: 0,
                dpi: 800,
                rate_ms: 1,
                mode: 1,
                active_sector: 1,
                dpi_index: 0,
                wheel: 0,
            },
            dpi_choices: vec![800, 1600],
            rate_choices: vec![1, 2],
            effects: vec![],
            led_ids: vec![],
            directory_sector: 0,
            directory: vec![],
            profiles: vec![p],
        }
    }
    fn app() -> App {
        let (tx, _) = mpsc::channel();
        let (_, rx) = mpsc::channel();
        App::new(
            tx,
            rx,
            crate::preferences::Preferences::default(),
            String::new(),
            false,
        )
    }
    fn incoming(snapshot: Snapshot) -> Reply {
        Reply {
            snapshot,
            message: "Read complete".into(),
            reset: false,
            imported: None,
            full: true,
            notification_error: None,
        }
    }
    #[test]
    fn automatic_refresh_preserves_draft_and_original_conflict_baseline() {
        let mut a = app();
        a.accept(incoming(sample()), false);
        a.name = "My unsaved profile".into();
        let original = a.save_base.as_ref().unwrap().profiles[0].clone();
        let mut changed = sample();
        changed.profiles[0].set_name("Changed on mouse").unwrap();
        changed.status.dpi = 1600;
        changed.status.battery = 75;
        a.accept(incoming(changed), true);
        assert_eq!(a.name, "My unsaved profile");
        assert_eq!(a.draft.as_ref().unwrap(), &original);
        assert_eq!(a.save_base.as_ref().unwrap().profiles[0], original);
        assert!(a.dirty());
        assert_eq!(
            a.snapshot.as_ref().unwrap().profiles[0].name(),
            "Changed on mouse"
        );
        assert_eq!(a.live_dpi, 1600);
        a.select(0); // Explicit discard rebases to the latest device state.
        assert_eq!(a.name, "Changed on mouse");
        assert!(!a.dirty());
    }
    #[test]
    fn automatic_refresh_keeps_open_binding_editor_and_pending_live_input() {
        let mut a = app();
        a.accept(incoming(sample()), false);
        a.edit_button = Some(2);
        a.live_dpi = 1200;
        let mut updated = sample();
        updated.status.dpi = 1600;
        a.accept(incoming(updated), true);
        assert_eq!(a.edit_button, Some(2));
        assert_eq!(a.live_dpi, 1200);
        a.accept(incoming(sample()), false);
        assert_eq!(a.live_dpi, 800);
    }
    #[test]
    fn clean_profile_tracks_external_profile_changes() {
        let mut a = app();
        a.accept(incoming(sample()), false);
        let mut changed = sample();
        changed.profiles[0].set_name("External").unwrap();
        a.accept(incoming(changed), true);
        assert_eq!(a.name, "External");
        assert!(!a.dirty());
    }
    #[test]
    fn rendering_dpi_sliders_preserves_disabled_stages_and_profile_bytes() {
        let mut snapshot = sample();
        snapshot.profiles[0].set_dpis([800, 0, 1600, 0, 25600]);
        snapshot.profiles[0].raw[0] = 1;
        for width in [540.0, 920.0] {
            let mut a = app();
            a.accept(incoming(snapshot.clone()), false);
            let original = a.draft.clone();
            let ctx = egui::Context::default();
            design::setup(&ctx);
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(width, 700.0),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| a.sensitivity(ui, &snapshot));
                });
            }
            assert_eq!(a.draft, original);
            assert!(!a.dirty());
        }
    }
    #[test]
    fn factory_name_placeholder_preserves_original_profile_bytes() {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: vec![0xff; 255],
        };
        p.set_name("PROFILE_NAME_DEFAULT").unwrap();
        let original = p.raw.clone();
        assert_eq!(edit_name(&p), "");
        apply_name(&mut p, "").unwrap();
        assert_eq!(p.raw, original);
    }
    #[test]
    fn ordinary_profile_name_can_be_renamed_and_cleared() {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: vec![0; 255],
        };
        apply_name(&mut p, "Work").unwrap();
        assert_eq!(edit_name(&p), "Work");
        apply_name(&mut p, "").unwrap();
        assert_eq!(p.name(), "");
    }
}
