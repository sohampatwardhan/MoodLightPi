use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
}

/// Standard WS2812 gamma (~2.8) lookup, computed once.
fn gamma_table() -> &'static [u8; 256] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u8; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let normalized = i as f32 / 255.0;
            *slot = (normalized.powf(2.8) * 255.0 + 0.5) as u8;
        }
        t
    })
}

pub fn gamma_correct(value: u8) -> u8 {
    gamma_table()[value as usize]
}

fn scale(channel: u8, brightness: u8) -> u8 {
    ((gamma_correct(channel) as u16 * brightness as u16) / 255) as u8
}

/// Pipeline order (fixed): gamma -> brightness -> GRB byte order.
/// Returns the three colour bytes in the WS2812 wire order [G, R, B].
pub fn to_grb(rgb: Rgb, brightness: u8) -> [u8; 3] {
    [
        scale(rgb.g, brightness),
        scale(rgb.r, brightness),
        scale(rgb.b, brightness),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_endpoints_preserved() {
        assert_eq!(gamma_correct(0), 0);
        assert_eq!(gamma_correct(255), 255);
    }

    #[test]
    fn gamma_is_monotonic_and_dims_midtones() {
        // gamma pulls mid values down (perceptual correction)
        assert!(gamma_correct(128) < 128);
        for v in 0u8..255 {
            assert!(gamma_correct(v) <= gamma_correct(v + 1));
        }
    }

    #[test]
    fn pipeline_scales_brightness_and_emits_grb() {
        let rgb = Rgb { r: 255, g: 0, b: 0 };
        // full brightness, red -> gamma(255)=255 red; GRB order => [G,R,B] = [0,255,0]
        assert_eq!(to_grb(rgb, 255), [0, 255, 0]);
        // half brightness scales the (gamma-corrected) channel
        let half = to_grb(rgb, 128);
        assert_eq!(half[0], 0);
        assert!(half[1] > 0 && half[1] < 255);
        assert_eq!(half[2], 0);
    }

    #[test]
    fn brightness_zero_is_black() {
        assert_eq!(
            to_grb(
                Rgb {
                    r: 255,
                    g: 255,
                    b: 255
                },
                0
            ),
            [0, 0, 0]
        );
    }
}
