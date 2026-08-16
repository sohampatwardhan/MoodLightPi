use crate::color::to_grb;
use crate::geometry::{xy_to_index, Frame, HEIGHT, PIXEL_COUNT, WIDTH};

/// Abstraction over the physical panel. Implementations apply the same
/// gamma -> brightness -> GRB pipeline so the mock is faithful.
pub trait Display: Send {
    fn show(&mut self, frame: &Frame, brightness: u8) -> anyhow::Result<()>;
    /// Turn all LEDs off (used on shutdown / power-off gate).
    fn clear(&mut self) -> anyhow::Result<()> {
        self.show(&crate::geometry::BLACK_FRAME, 0)
    }
}

/// In-memory Display for host tests. Records the last strip written,
/// as GRB byte triples at physical strip indices.
pub struct MockDisplay {
    strip: [[u8; 3]; PIXEL_COUNT],
}

impl MockDisplay {
    pub fn new() -> Self {
        Self {
            strip: [[0; 3]; PIXEL_COUNT],
        }
    }
    pub fn last_strip(&self) -> &[[u8; 3]; PIXEL_COUNT] {
        &self.strip
    }
}

impl Display for MockDisplay {
    fn show(&mut self, frame: &Frame, brightness: u8) -> anyhow::Result<()> {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = xy_to_index(x, y);
                self.strip[idx] = to_grb(frame[y * WIDTH + x], brightness);
            }
        }
        Ok(())
    }
}

impl Display for Box<dyn Display> {
    fn show(&mut self, frame: &crate::geometry::Frame, brightness: u8) -> anyhow::Result<()> {
        (**self).show(frame, brightness)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::geometry::{xy_to_index, BLACK_FRAME};

    #[test]
    fn mock_records_grb_bytes_at_mapped_indices() {
        let mut d = MockDisplay::new();
        let mut frame = BLACK_FRAME;
        frame[0] = Rgb { r: 255, g: 0, b: 0 }; // logical (0,0)
        d.show(&frame, 255).unwrap();
        let strip = d.last_strip();
        // logical (0,0) -> strip index 24; red at full brightness -> GRB [0,255,0]
        assert_eq!(strip[xy_to_index(0, 0)], [0, 255, 0]);
        assert_eq!(strip[xy_to_index(1, 0)], [0, 0, 0]);
    }

    #[test]
    fn mock_applies_brightness() {
        let mut d = MockDisplay::new();
        let mut frame = BLACK_FRAME;
        frame[0] = Rgb { r: 255, g: 0, b: 0 };
        d.show(&frame, 0).unwrap();
        assert_eq!(d.last_strip()[xy_to_index(0, 0)], [0, 0, 0]);
    }
}
