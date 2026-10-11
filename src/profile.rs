use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

pub const BUTTON_NAMES: [&str; 11] = [
    "Left click",
    "Right click",
    "Wheel click",
    "Back",
    "DPI shift",
    "Forward",
    "Wheel tilt left",
    "Wheel tilt right",
    "Profile cycle",
    "DPI up",
    "DPI down",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub sector: u16,
    pub enabled: bool,
    /// The original bytes are authoritative. Only explicitly changed fields are patched.
    pub raw: Vec<u8>,
}

pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for &b in bytes {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
pub fn valid_crc(bytes: &[u8]) -> bool {
    bytes.len() >= 2
        && crc16(&bytes[..bytes.len() - 2])
            == u16::from_be_bytes([bytes[bytes.len() - 2], bytes[bytes.len() - 1]])
}
pub fn seal(bytes: &mut [u8]) {
    let n = bytes.len();
    let crc = crc16(&bytes[..n - 2]);
    bytes[n - 2..].copy_from_slice(&crc.to_be_bytes());
}

impl Profile {
    pub fn validate_shape(&self) -> Result<()> {
        ensure!(
            (255..=4096).contains(&self.raw.len()),
            "Unsupported profile length"
        );
        Ok(())
    }
    pub fn name(&self) -> String {
        let units: Vec<u16> = self.raw[160..208]
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .take_while(|&c| c != 0 && c != 0xffff)
            .collect();
        String::from_utf16_lossy(&units)
    }
    pub fn set_name(&mut self, name: &str) -> Result<()> {
        if self.name() == name {
            return Ok(());
        }
        let units: Vec<_> = name.encode_utf16().collect();
        ensure!(
            units.len() <= 24 && !units.contains(&0) && !units.contains(&0xffff),
            "Name must fit in 24 UTF-16 units and contain no NUL characters"
        );
        self.raw[160..208].fill(0);
        for (i, c) in units.into_iter().enumerate() {
            self.raw[160 + 2 * i..162 + 2 * i].copy_from_slice(&c.to_le_bytes());
        }
        Ok(())
    }
    pub fn dpis(&self) -> [u16; 5] {
        std::array::from_fn(|i| u16::from_le_bytes([self.raw[3 + 2 * i], self.raw[4 + 2 * i]]))
    }
    pub fn set_dpis(&mut self, dpis: [u16; 5]) {
        for (i, dpi) in dpis.iter().enumerate() {
            self.raw[3 + 2 * i..5 + 2 * i].copy_from_slice(&dpi.to_le_bytes());
        }
    }
    pub fn binding(&self, index: usize, shifted: bool) -> [u8; 4] {
        let start = if shifted { 96 } else { 32 };
        self.raw[start + 4 * index..start + 4 * index + 4]
            .try_into()
            .unwrap()
    }
    pub fn set_binding(&mut self, index: usize, shifted: bool, binding: [u8; 4]) -> Result<()> {
        ensure!(index < 11, "Button index must be 0–10");
        let start = if shifted { 96 } else { 32 };
        self.raw[start + 4 * index..start + 4 * index + 4].copy_from_slice(&binding);
        Ok(())
    }
    pub fn timeout(&self, off: bool) -> u16 {
        let i = if off { 30 } else { 28 };
        u16::from_le_bytes([self.raw[i], self.raw[i + 1]])
    }
    pub fn set_timeout(&mut self, off: bool, seconds: u16) {
        let i = if off { 30 } else { 28 };
        self.raw[i..i + 2].copy_from_slice(&seconds.to_le_bytes());
    }
    /// G502 X Plus uses the second lighting slot, including its alternate bank.
    pub fn lighting(&self) -> [u8; 11] {
        self.raw[219..230].try_into().unwrap()
    }
    pub fn set_lighting(&mut self, effect: [u8; 11]) {
        self.raw[219..230].copy_from_slice(&effect);
        self.raw[241..252].copy_from_slice(&effect);
        self.raw[252] = 0;
    }
    pub fn validate_settings(&self, allowed_dpi: &[u16], rates: &[u8]) -> Result<()> {
        self.validate_shape()?;
        ensure!(rates.contains(&self.raw[0]), "Unsupported polling interval");
        let dpis = self.dpis();
        for dpi in dpis {
            ensure!(
                dpi == 0 || allowed_dpi.contains(&dpi),
                "Unsupported DPI {dpi}"
            );
        }
        for index in [self.raw[1], self.raw[2]] {
            ensure!(
                (index as usize) < 5 && dpis[index as usize] != 0,
                "Default and shift DPI must refer to enabled stages"
            );
        }
        Ok(())
    }
}

pub fn binding_label(b: [u8; 4]) -> String {
    match (b[0], b[1]) {
        (0x80, 0) | (0x90, 0) => "Disabled".into(),
        (0x80, 1) => {
            let mask = u16::from_be_bytes([b[2], b[3]]);
            match mask {
                1 => "Left click".into(),
                2 => "Right click".into(),
                4 => "Middle click".into(),
                8 => "Back".into(),
                16 => "Forward".into(),
                _ => format!("Mouse mask 0x{mask:04x}"),
            }
        }
        (0x80, 2) => {
            let mut parts = Vec::new();
            for (bit, label) in [
                (1, "Ctrl"),
                (2, "Shift"),
                (4, "Alt"),
                (8, "Super"),
                (16, "Right Ctrl"),
                (32, "Right Shift"),
                (64, "Right Alt"),
                (128, "Right Super"),
            ] {
                if b[2] & bit != 0 {
                    parts.push(label.to_owned());
                }
            }
            parts.push(key_name(b[3]));
            parts.join(" + ")
        }
        (0x80, 3) => format!("Media key 0x{:04x}", u16::from_be_bytes([b[2], b[3]])),
        (0x90, f) => format!(
            "{}{}",
            function_name(f),
            if f == 13 {
                format!(" {}", b[3])
            } else {
                String::new()
            }
        ),
        _ if b[0] >> 4 <= 2 => "Existing macro (preserved)".into(),
        _ => format!("Preserved {:02x?}", b),
    }
}
pub fn function_name(f: u8) -> &'static str {
    match f {
        0 => "Disabled",
        1 => "Tilt left",
        2 => "Tilt right",
        3 => "Next DPI",
        4 => "Previous DPI",
        5 => "Cycle DPI",
        6 => "Default DPI",
        7 => "DPI shift",
        8 => "Next profile",
        9 => "Previous profile",
        10 => "Cycle profile",
        11 => "G-Shift",
        12 => "Battery level",
        13 => "Select profile",
        14 => "Mode switch",
        15 => "Host button",
        16 => "Scroll down",
        17 => "Scroll up",
        _ => "Unknown function",
    }
}

pub fn parse_binding(value: &str) -> Result<[u8; 4]> {
    let parts: Vec<_> = value.split(':').collect();
    let num = |s: &str| -> Result<u16> {
        Ok(if let Some(x) = s.strip_prefix("0x") {
            u16::from_str_radix(x, 16)?
        } else {
            s.parse()?
        })
    };
    match parts.as_slice() {
        ["disabled"] => Ok([0x80, 0, 0xff, 0xff]),
        ["mouse", n] => {
            let n = num(n)?;
            ensure!((1..=16).contains(&n), "Mouse button must be 1–16");
            let b = (1u16 << (n - 1)).to_be_bytes();
            Ok([0x80, 1, b[0], b[1]])
        }
        ["key", key, modifiers] => {
            let (key, mods) = (num(key)?, num(modifiers)?);
            ensure!(
                key <= 255 && mods <= 255,
                "Key and modifier must fit in one byte"
            );
            Ok([0x80, 2, mods as u8, key as u8])
        }
        ["media", code] => {
            let code = num(code)?.to_be_bytes();
            Ok([0x80, 3, code[0], code[1]])
        }
        ["function", code] => {
            let code = num(code)?;
            ensure!(
                code <= 17 && code != 13,
                "Use profile:N for profile selection"
            );
            Ok([0x90, code as u8, 0xff, 0])
        }
        ["profile", n] => {
            let n = num(n)?;
            ensure!((1..=5).contains(&n), "Profile must be 1–5");
            Ok([0x90, 13, 0xff, n as u8])
        }
        _ => bail!(
            "Binding: disabled | mouse:1–16 | key:HID_USAGE:MODIFIER_MASK | media:HID_USAGE | function:0–17 | profile:1–5"
        ),
    }
}

pub fn effect_bytes(id: u16, color: [u8; 3], period: u16, intensity: u8) -> Result<[u8; 11]> {
    ensure!(intensity <= 100, "Brightness must be 0–100");
    let mut b = [0; 11];
    b[0] = id.try_into()?;
    match id {
        0 => (),
        1 => b[1..4].copy_from_slice(&color),
        3 => {
            b[6..8].copy_from_slice(&period.to_be_bytes());
            b[8] = intensity;
        }
        10 => {
            b[1..4].copy_from_slice(&color);
            b[4..6].copy_from_slice(&period.to_be_bytes());
            b[7] = intensity;
        }
        21 | 22 => {
            b[2] = 255;
            b[7..9].copy_from_slice(&period.to_be_bytes());
            b[9] = intensity;
        }
        _ => bail!("This effect's parameter layout is not supported"),
    }
    Ok(b)
}

pub fn key_name(k: u8) -> String {
    match k {
        4..=29 => ((b'A' + k - 4) as char).to_string(),
        30..=38 => format!("{}", k - 29),
        39 => "0".into(),
        40 => "Enter".into(),
        41 => "Escape".into(),
        42 => "Backspace".into(),
        43 => "Tab".into(),
        44 => "Space".into(),
        58..=69 => format!("F{}", k - 57),
        79 => "Right arrow".into(),
        80 => "Left arrow".into(),
        81 => "Down arrow".into(),
        82 => "Up arrow".into(),
        _ => format!("HID 0x{k:02X}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_known_vector() {
        assert_eq!(crc16(b"123456789"), 0x29b1);
    }
    #[test]
    fn preserves_reserved_bytes_and_macros() {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: (0..255).map(|i| i as u8).collect(),
        };
        let original = p.raw.clone();
        p.set_name("Linux 🐧").unwrap();
        p.set_dpis([400, 800, 1600, 3200, 0]);
        assert_eq!(p.name(), "Linux 🐧");
        assert_eq!(&p.raw[13..160], &original[13..160]);
        assert_eq!(&p.raw[208..], &original[208..]);
        seal(&mut p.raw);
        assert!(valid_crc(&p.raw));
        p.raw[24] ^= 1;
        assert!(!valid_crc(&p.raw));
    }
    #[test]
    fn unicode_limit_never_splits_surrogates() {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: vec![0; 255],
        };
        assert!(p.set_name(&"🐧".repeat(13)).is_err());
        assert!(p.set_name(&"🐧".repeat(12)).is_ok());
    }
    #[test]
    fn remap_wire_order() {
        assert_eq!(parse_binding("key:0x06:1").unwrap(), [0x80, 2, 1, 6]);
        assert_eq!(parse_binding("mouse:5").unwrap(), [0x80, 1, 0, 16]);
        assert!(parse_binding("mouse:17").is_err());
    }
    #[test]
    fn lighting_uses_second_slot_and_retains_other_fields() {
        let mut p = Profile {
            sector: 1,
            enabled: true,
            raw: vec![0xff; 255],
        };
        p.set_lighting(effect_bytes(1, [1, 2, 3], 0, 100).unwrap());
        assert_eq!(&p.raw[219..223], &[1, 1, 2, 3]);
        assert_eq!(p.raw[208], 255);
        assert_eq!(p.raw[252], 0);
    }
}
