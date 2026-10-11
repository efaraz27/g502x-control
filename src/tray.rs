//! Native tray; Linux uses StatusNotifierItem without a GTK/GPU runtime.
use anyhow::Result;
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

struct Tray {
    icon: TrayIcon,
    battery: MenuItem,
    open: MenuItem,
    settings: MenuItem,
    quit: MenuItem,
    last_label: String,
}
impl Tray {
    fn new() -> Result<Self> {
        let image = image::load_from_memory(include_bytes!("../packaging/g502x-control.png"))?
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        let icon = tray_icon::Icon::from_rgba(image.into_raw(), 32, 32)?;
        let menu = Menu::new();
        let battery = MenuItem::new("G502 X Plus · Connecting…", false, None);
        let open = MenuItem::new("Open G502 X Control", true, None);
        let settings = MenuItem::new("Settings…", true, None);
        let quit = MenuItem::new("Quit background monitor", true, None);
        menu.append_items(&[
            &battery,
            &PredefinedMenuItem::separator(),
            &open,
            &settings,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;
        let icon = TrayIconBuilder::new()
            .with_id(crate::preferences::APP_ID)
            .with_tooltip("G502 X Control")
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_menu_on_left_click(true)
            .build()?;
        Ok(Self {
            icon,
            battery,
            open,
            settings,
            quit,
            last_label: String::new(),
        })
    }
    fn tick(&mut self, stop: &AtomicBool) {
        let label = match crate::monitor::health() {
            Some(h) if h.connected => format!(
                "G502 X Plus · {}% battery{}",
                h.battery.unwrap_or(0),
                if h.charging { " · Charging" } else { "" }
            ),
            Some(_) => "G502 X Plus · Disconnected / asleep".into(),
            None => "G502 X Plus · Connecting…".into(),
        };
        if self.last_label != label {
            self.battery.set_text(&label);
            let _ = self.icon.set_tooltip(Some(&label));
            self.last_label = label;
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open.id() {
                open_window(false);
            }
            if event.id == self.settings.id() {
                open_window(true);
            }
            if event.id == self.quit.id() {
                stop.store(true, Ordering::Relaxed);
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                open_window(false);
            }
        }
    }
}
fn open_window(settings: bool) {
    if let Ok(exe) = std::env::current_exe() {
        let mut command = Command::new(exe);
        if settings {
            command.arg("--settings");
        }
        // Reap the child when it exits, without tying the monitor to its lifetime.
        if let Ok(mut child) = command.spawn() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
pub fn run(stop: Arc<AtomicBool>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let mut tray = Tray::new()?;
        while !stop.load(Ordering::Relaxed) {
            tray.tick(&stop);
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        use winit::{
            application::ApplicationHandler,
            event::WindowEvent,
            event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
            window::WindowId,
        };
        struct Resident {
            tray: Option<Tray>,
            stop: Arc<AtomicBool>,
            error: Option<anyhow::Error>,
        }
        impl ApplicationHandler for Resident {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                if self.tray.is_none() {
                    match Tray::new() {
                        Ok(tray) => self.tray = Some(tray),
                        Err(e) => {
                            self.error = Some(e);
                            event_loop.exit();
                        }
                    }
                }
            }
            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
            fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
                if let Some(tray) = &mut self.tray {
                    tray.tick(&self.stop);
                }
                if self.stop.load(Ordering::Relaxed) {
                    event_loop.exit();
                } else {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + Duration::from_millis(250),
                    ));
                }
            }
        }
        let builder = EventLoop::builder();
        #[cfg(target_os = "macos")]
        let builder = {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            let mut builder = builder;
            builder.with_activation_policy(ActivationPolicy::Accessory);
            builder
        };
        let event_loop = {
            let mut builder = builder;
            builder.build()?
        };
        let mut app = Resident {
            tray: None,
            stop,
            error: None,
        };
        event_loop.run_app(&mut app)?;
        match app.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
