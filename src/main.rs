#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod device;
#[cfg(feature = "gui")]
mod gui;
mod profile;
mod transport;
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Native control panel for Logitech G502 X Plus. No command opens the GUI."
)]
struct Args {
    /// Select an endpoint path from `list`; defaults to USB cable, then receiver.
    #[arg(long, global = true)]
    device: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Read current settings and every onboard profile (JSON).
    Status,
    /// List matching HID endpoints without changing the mouse.
    List,
    /// Back up all writable memory, including existing macros and animations.
    Backup { file: PathBuf },
    /// Restore a full backup to the SAME mouse; makes another backup first.
    Restore {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    /// Export an editable profile as JSON, including all preserved raw bytes.
    Export { profile: u16, file: PathBuf },
    /// Import a profile file. Without --apply, validate and show the planned change.
    Import {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    /// Select an enabled onboard profile by sector (normally 1–5).
    Activate { profile: u16 },
    /// Choose onboard or software control. This does not rewrite flash.
    Mode {
        #[arg(value_parser=["onboard","software"])]
        mode: String,
    },
    /// Set live DPI in software mode.
    Dpi { value: u16 },
    /// Select DPI stage 1–5 in the active onboard profile.
    Stage { value: u8 },
    /// Set live polling rate in software mode (Hz).
    Rate {
        #[arg(value_parser=clap::value_parser!(u16).range(125..=1000))]
        hz: u16,
    },
    /// Set wheel direction and high-resolution mode.
    Wheel {
        #[arg(long)]
        reverse: bool,
        #[arg(long)]
        hires: bool,
    },
    /// Set software-mode lighting. For persistent lighting use the profile editor.
    Rgb {
        #[arg(value_parser=["off","static","breathe","cycle"])]
        effect: String,
        #[arg(default_value = "42D6B5")]
        color: String,
        #[arg(long, default_value_t = 5000)]
        period: u16,
        #[arg(long, default_value_t = 100)]
        brightness: u8,
    },
    /// Return lighting control to the firmware.
    ReleaseRgb,
    /// Set eight software-mode LED colors, in hardware segment order.
    Leds {
        #[arg(num_args = 8)]
        colors: Vec<String>,
    },
    /// Edit a profile field; validates by default and writes only with --apply.
    Profile {
        profile: u16,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_delimiter = ',')]
        dpi: Option<Vec<u16>>,
        #[arg(long)]
        default_stage: Option<u8>,
        #[arg(long)]
        shift_stage: Option<u8>,
        #[arg(long)]
        rate: Option<u16>,
        #[arg(long)]
        button: Option<u8>,
        #[arg(long)]
        binding: Option<String>,
        #[arg(long)]
        g_shift: bool,
        #[arg(long)]
        apply: bool,
    },
    /// Check hardware reads and no-op profile validation; no flash writes.
    Check,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ProfileFile {
    schema: u8,
    unit_id: String,
    layout: device::Layout,
    profile: profile::Profile,
}
fn main() {
    #[cfg(all(windows, not(debug_assertions)))]
    if std::env::args_os().len() > 1 {
        // GUI launches stay console-free; CLI invocations use the parent terminal.
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args = Args::parse();
    let Some(command) = args.command else {
        #[cfg(feature = "gui")]
        return gui::run(args.device);
        #[cfg(not(feature = "gui"))]
        anyhow::bail!("This build has no GUI. Run --help for commands.");
    };
    if matches!(command, Command::List) {
        println!("{}", serde_json::to_string_pretty(&transport::discover()?)?);
        return Ok(());
    }
    let mut d = device::Device::connect(args.device.as_deref())?;
    match command {
        Command::Status => println!("{}", serde_json::to_string_pretty(&d.snapshot()?)?),
        Command::Backup { file } => {
            device::write_json_new(&file, &d.backup()?)?;
            println!("Backup saved: {}", file.display());
        }
        Command::Restore { file, apply } => {
            let b: device::Backup = serde_json::from_slice(&std::fs::read(file)?)?;
            let layout = d.layout()?;
            device::validate_backup(&b, &d.unit_id, &layout)?;
            if apply {
                println!(
                    "Restored and verified. Pre-restore backup: {}",
                    d.restore(&b)?.display()
                );
            } else {
                println!(
                    "Backup valid for this mouse. Use --apply to restore its complete onboard memory."
                );
            }
        }
        Command::Export {
            profile: sector,
            file,
        } => {
            let s = d.snapshot()?;
            let p = s
                .profiles
                .iter()
                .find(|p| p.sector == sector)
                .context("Profile not found")?;
            device::write_json_new(
                &file,
                &ProfileFile {
                    schema: 1,
                    unit_id: s.unit_id,
                    layout: s.layout,
                    profile: p.clone(),
                },
            )?;
            println!("Exported to {}", file.display());
        }
        Command::Import { file, apply } => {
            let p: ProfileFile = serde_json::from_slice(&std::fs::read(file)?)?;
            let s = d.snapshot()?;
            ensure!(
                p.schema == 1 && p.unit_id == s.unit_id && p.layout == s.layout,
                "Profile file is for another mouse or layout"
            );
            p.profile
                .validate_settings(&s.dpi_choices, &s.rate_choices)?;
            save_or_preview(&mut d, &s, &p.profile, apply)?;
        }
        Command::Activate { profile } => {
            d.activate(profile)?;
            println!("Profile {profile} active");
        }
        Command::Mode { mode } => {
            if mode == "onboard" {
                d.release_rgb()?;
            }
            d.set_mode(mode == "onboard")?;
            println!("{mode} mode active");
        }
        Command::Dpi { value } => {
            d.set_dpi(value)?;
            println!("DPI {value} verified");
        }
        Command::Stage { value } => {
            ensure!((1..=5).contains(&value), "Stage must be 1–5");
            d.set_dpi_index(value - 1)?;
            println!("Stage {value} verified");
        }
        Command::Rate { hz } => {
            ensure!(1000 % hz == 0, "Rate must divide 1000 Hz exactly");
            d.set_rate((1000 / hz) as u8)?;
            println!("Polling rate {hz} Hz verified");
        }
        Command::Wheel { reverse, hires } => {
            d.set_wheel(reverse, hires)?;
            println!("Wheel settings verified");
        }
        Command::Rgb {
            effect,
            color,
            period,
            brightness,
        } => {
            let id = match effect.as_str() {
                "off" => 0,
                "static" => 1,
                "breathe" => 10,
                _ => 3,
            };
            d.set_rgb(profile::effect_bytes(
                id,
                parse_color(&color)?,
                period,
                brightness,
            )?)?;
            println!("Lighting command acknowledged by mouse (live RGB cannot be read back)");
        }
        Command::ReleaseRgb => {
            d.release_rgb()?;
            println!("Lighting returned to firmware");
        }
        Command::Leds { colors } => {
            let colors: Result<Vec<_>> = colors.iter().map(|c| parse_color(c)).collect();
            d.set_leds(&colors?)?;
            println!("LED colors acknowledged by mouse (no live readback)");
        }
        Command::Profile {
            profile: sector,
            name,
            dpi,
            default_stage,
            shift_stage,
            rate,
            button,
            binding,
            g_shift,
            apply,
        } => {
            let s = d.snapshot()?;
            let mut p = s
                .profiles
                .iter()
                .find(|p| p.sector == sector)
                .context("Profile not found")?
                .clone();
            if let Some(name) = name {
                p.set_name(&name)?;
            }
            if let Some(dpi) = dpi {
                p.set_dpis(
                    dpi.try_into()
                        .map_err(|_| anyhow::anyhow!("Exactly five DPI stages required"))?,
                );
            }
            for (value, offset) in [(default_stage, 1), (shift_stage, 2)] {
                if let Some(value) = value {
                    ensure!((1..=5).contains(&value), "Stage must be 1–5");
                    p.raw[offset] = value - 1;
                }
            }
            if let Some(rate) = rate {
                ensure!(
                    rate > 0 && 1000 % rate == 0 && 1000 / rate <= 8,
                    "Unsupported rate"
                );
                p.raw[0] = (1000 / rate) as u8;
            }
            match (button, binding) {
                (Some(button), Some(binding)) => {
                    ensure!((1..=11).contains(&button), "Button must be 1–11");
                    p.set_binding(
                        button as usize - 1,
                        g_shift,
                        profile::parse_binding(&binding)?,
                    )?;
                }
                (None, None) => (),
                _ => anyhow::bail!("--button and --binding must be supplied together"),
            }
            p.validate_settings(&s.dpi_choices, &s.rate_choices)?;
            save_or_preview(&mut d, &s, &p, apply)?;
        }
        Command::Check => {
            let s = d.snapshot()?;
            println!(
                "HID++ {} / {} / {}",
                s.protocol,
                s.unit_id,
                s.endpoint.path.display()
            );
            println!(
                "{} profiles, {} buttons, {} LEDs, {} DPI choices",
                s.profiles.len(),
                s.layout.buttons,
                s.led_ids.len(),
                s.dpi_choices.len()
            );
            for p in &s.profiles {
                ensure!(
                    d.save_profile(&s, p)?.is_none(),
                    "No-op unexpectedly wrote a profile"
                );
            }
            println!("All profile CRCs, field ranges and no-op saves passed. No flash writes.");
        }
        Command::List => unreachable!(),
    }
    Ok(())
}
fn save_or_preview(
    d: &mut device::Device,
    s: &device::Snapshot,
    p: &profile::Profile,
    apply: bool,
) -> Result<()> {
    println!(
        "Profile {}: name {:?}, DPI {:?}, {} ms",
        p.sector,
        p.name(),
        p.dpis(),
        p.raw[0]
    );
    if apply {
        match d.save_profile(s, p)? {
            Some(path) => println!("Saved and readback verified. Backup: {}", path.display()),
            None => println!("Already matches; no flash write needed."),
        }
    } else {
        println!("Preview only. Add --apply to write.");
    }
    Ok(())
}
pub fn parse_color(s: &str) -> Result<[u8; 3]> {
    let s = s.trim_start_matches('#');
    ensure!(s.len() == 6 && s.is_ascii(), "Color must be six hex digits");
    let n = u32::from_str_radix(s, 16)?;
    Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}
