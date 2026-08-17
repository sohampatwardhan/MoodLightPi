use crate::color::Rgb;
use crate::geometry::{Frame, BLACK_FRAME, HEIGHT, WIDTH};
use crate::state::{Mode, State};
use palette::{Hsv, IntoColor, Srgb};

/// Effects the registry knows about. Single source of truth for both
/// `GET /api/effects` and API validation.
pub const EFFECTS: &[&str] = &["solid", "rainbow", "colorcycle", "breathe"];

pub fn effect_names() -> Vec<&'static str> {
    EFFECTS.to_vec()
}

pub fn is_valid_effect(name: &str) -> bool {
    EFFECTS.contains(&name)
}

/// Map speed (0..=255) to a phase increment per tick. speed=0 -> slowest
/// non-zero (never frozen, never divide-by-zero).
fn phase_step(speed: u8) -> f32 {
    0.002 + (speed as f32 / 255.0) * 0.06
}

/// `hue_deg` in degrees (0..360). palette 0.7: convert via IntoColor, then
/// into_format() to get clamped/rounded Srgb<u8>.
fn hsv_to_rgb(hue_deg: f32, sat: f32, val: f32) -> Rgb {
    let rgb_f: Srgb = Hsv::new(hue_deg, sat, val).into_color();
    let rgb: Srgb<u8> = rgb_f.into_format();
    Rgb {
        r: rgb.red,
        g: rgb.green,
        b: rgb.blue,
    }
}

fn fill(color: Rgb) -> Frame {
    [color; crate::geometry::PIXEL_COUNT]
}

/// Pure: state + tick -> logical frame. Never applies brightness/power
/// (that is the Display's job via effective_brightness).
pub fn render_frame(state: &State, tick: u64) -> Frame {
    match state.mode {
        Mode::Solid => fill(state.rgb),
        Mode::Effect => match state.effect_name.as_str() {
            "solid" => fill(state.rgb),
            "rainbow" => rainbow(state, tick),
            "colorcycle" => colorcycle(state, tick),
            "breathe" => breathe(state, tick),
            _ => fill(state.rgb), // unknown -> safe fallback
        },
    }
}

fn rainbow(state: &State, tick: u64) -> Frame {
    let phase = tick as f32 * phase_step(state.speed);
    let mut frame = BLACK_FRAME;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let hue = ((x as f32 / WIDTH as f32) + phase).fract() * 360.0;
            frame[y * WIDTH + x] = hsv_to_rgb(hue, 1.0, 1.0);
        }
    }
    frame
}

fn colorcycle(state: &State, tick: u64) -> Frame {
    let hue = (tick as f32 * phase_step(state.speed)).fract() * 360.0;
    fill(hsv_to_rgb(hue, 1.0, 1.0))
}

fn breathe(state: &State, tick: u64) -> Frame {
    // triangle wave 0.15..1.0 on the state colour's value
    let t = (tick as f32 * phase_step(state.speed)).fract();
    let tri = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
    let v = 0.15 + tri * 0.85;
    // Read the state colour's hue/saturation (u8 -> f32 -> Hsv).
    let src: Srgb = Srgb::new(
        state.rgb.r as f32 / 255.0,
        state.rgb.g as f32 / 255.0,
        state.rgb.b as f32 / 255.0,
    );
    let base: Hsv = src.into_color();
    fill(hsv_to_rgb(
        base.hue.into_positive_degrees(),
        base.saturation,
        v,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::geometry::PIXEL_COUNT;
    use crate::state::{Mode, State};

    #[test]
    fn registry_lists_known_effects_and_validates() {
        let names = effect_names();
        assert!(names.contains(&"solid"));
        assert!(names.contains(&"rainbow"));
        assert!(is_valid_effect("breathe"));
        assert!(!is_valid_effect("nope"));
    }

    #[test]
    fn solid_mode_fills_with_state_color() {
        let s = State {
            mode: Mode::Solid,
            rgb: Rgb {
                r: 10,
                g: 20,
                b: 30,
            },
            ..State::default()
        };
        let frame = render_frame(&s, 0);
        assert!(frame.iter().all(|&p| p
            == Rgb {
                r: 10,
                g: 20,
                b: 30
            }));
    }

    #[test]
    fn rainbow_changes_over_time_and_fills_all_pixels() {
        let s = State {
            mode: Mode::Effect,
            effect_name: "rainbow".into(),
            speed: 128,
            ..State::default()
        };
        let f0 = render_frame(&s, 0);
        let f1 = render_frame(&s, 10);
        assert_eq!(f0.len(), PIXEL_COUNT);
        assert_ne!(f0, f1, "rainbow should animate across ticks");
    }

    #[test]
    fn breathe_returns_state_hue_but_varies_value() {
        let s = State {
            mode: Mode::Effect,
            effect_name: "breathe".into(),
            rgb: Rgb { r: 255, g: 0, b: 0 },
            speed: 128,
            ..State::default()
        };
        let dim = render_frame(&s, 0);
        let bright = render_frame(&s, 32);
        assert_ne!(
            dim[0], bright[0],
            "breathe should vary brightness of the pixel"
        );
    }

    #[test]
    fn unknown_effect_falls_back_to_solid() {
        let s = State {
            mode: Mode::Effect,
            effect_name: "bogus".into(),
            rgb: Rgb { r: 1, g: 2, b: 3 },
            ..State::default()
        };
        let frame = render_frame(&s, 5);
        assert!(frame.iter().all(|&p| p == Rgb { r: 1, g: 2, b: 3 }));
    }
}
