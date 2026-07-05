use crate::color::Rgb;
use serde::{Deserialize, Serialize};

/// Brown-out ceiling: caps worst-case current from 32 WS2812 (see spec §7).
pub const MAX_BRIGHTNESS: u8 = 160;
/// Conservative default so a fresh device never boots into full-white draw.
pub const SAFE_DEFAULT_BRIGHTNESS: u8 = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Solid,
    Effect,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub power: bool,
    pub mode: Mode,
    pub rgb: Rgb,
    pub brightness: u8,
    pub effect_name: String,
    pub speed: u8,
}

impl Default for State {
    fn default() -> Self {
        Self {
            power: true,
            mode: Mode::Solid,
            rgb: Rgb { r: 255, g: 147, b: 41 }, // warm white
            brightness: SAFE_DEFAULT_BRIGHTNESS,
            effect_name: "rainbow".to_string(),
            speed: 128,
        }
    }
}

impl State {
    /// Brightness actually sent to the panel: 0 when powered off, else capped.
    pub fn effective_brightness(&self) -> u8 {
        if !self.power {
            0
        } else {
            self.brightness.min(MAX_BRIGHTNESS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_safe_low_brightness_solid() {
        let s = State::default();
        assert!(s.power);
        assert_eq!(s.mode, Mode::Solid);
        assert!(s.brightness <= SAFE_DEFAULT_BRIGHTNESS);
    }

    #[test]
    fn roundtrips_through_json() {
        let s = State { brightness: 200, ..State::default() };
        let json = serde_json::to_string(&s).unwrap();
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn effective_brightness_is_capped() {
        let s = State { brightness: 255, ..State::default() };
        assert_eq!(s.effective_brightness(), MAX_BRIGHTNESS);
        let off = State { power: false, brightness: 255, ..State::default() };
        assert_eq!(off.effective_brightness(), 0);
    }
}
