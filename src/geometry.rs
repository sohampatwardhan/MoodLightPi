use crate::color::Rgb;

pub const WIDTH: usize = 8;
pub const HEIGHT: usize = 4;
pub const PIXEL_COUNT: usize = WIDTH * HEIGHT;

/// A logical frame indexed `frame[y * WIDTH + x]`.
pub type Frame = [Rgb; PIXEL_COUNT];

pub const BLACK_FRAME: Frame = [Rgb::BLACK; PIXEL_COUNT];

/// Authoritative Unicorn pHAT map (PHAT[x][y]) from
/// pimoroni/unicorn-hat library/UnicornHat/unicornhat.py (v2.2.3).
/// Column-major. Orientation confirmed on-device later; if the physical
/// origin differs, flip y here and update the corner test.
const PHAT_MAP: [[usize; HEIGHT]; WIDTH] = [
    [24, 16, 8, 0],
    [25, 17, 9, 1],
    [26, 18, 10, 2],
    [27, 19, 11, 3],
    [28, 20, 12, 4],
    [29, 21, 13, 5],
    [30, 22, 14, 6],
    [31, 23, 15, 7],
];

pub fn xy_to_index(x: usize, y: usize) -> usize {
    PHAT_MAP[x][y]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions() {
        assert_eq!(WIDTH, 8);
        assert_eq!(HEIGHT, 4);
        assert_eq!(PIXEL_COUNT, 32);
    }

    #[test]
    fn map_is_a_bijection_over_0_31() {
        let mut seen = [false; PIXEL_COUNT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = xy_to_index(x, y);
                assert!(idx < PIXEL_COUNT, "index {idx} out of range at ({x},{y})");
                assert!(!seen[idx], "duplicate strip index {idx}");
                seen[idx] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "map does not cover all 32 LEDs");
    }

    #[test]
    fn known_corners_from_pimoroni_phat_array() {
        assert_eq!(xy_to_index(0, 0), 24);
        assert_eq!(xy_to_index(0, 3), 0);
        assert_eq!(xy_to_index(7, 0), 31);
        assert_eq!(xy_to_index(7, 3), 7);
    }
}
