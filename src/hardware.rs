use crate::color::to_grb;
use crate::display::Display;
use crate::geometry::{xy_to_index, Frame, HEIGHT, WIDTH};
use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};

pub struct Ws281xDisplay {
    controller: Controller,
}

// SAFETY: `Controller` is `!Send` because it holds raw pointers into DMA/mmap
// regions. `Ws281xDisplay` is owned exclusively by the single-writer render
// engine task (see engine.rs) and is never shared or accessed concurrently
// from more than one thread, so transferring ownership to the async runtime's
// thread is sound.
unsafe impl Send for Ws281xDisplay {}

impl Ws281xDisplay {
    /// `dma_channel` defaults to 10; NEVER 5 (filesystem corruption).
    pub fn new(dma_channel: i32) -> anyhow::Result<Self> {
        let controller = ControllerBuilder::new()
            .freq(800_000)
            .dma(dma_channel)
            .channel(
                0,
                ChannelBuilder::new()
                    .pin(18)
                    .count(crate::geometry::PIXEL_COUNT as i32)
                    .strip_type(StripType::Ws2812)
                    .brightness(255) // brightness applied by our pipeline, not here
                    .build(),
            )
            .build()?;
        Ok(Self { controller })
    }
}

impl Display for Ws281xDisplay {
    fn show(&mut self, frame: &Frame, brightness: u8) -> anyhow::Result<()> {
        let leds = self.controller.leds_mut(0);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = xy_to_index(x, y);
                let grb = to_grb(frame[y * WIDTH + x], brightness);
                // rs_ws281x expects [B, G, R, W] per LED; grb = [G, R, B].
                // Confirm the permutation on-device (checklist #7: red shows red).
                leds[idx] = [grb[2], grb[0], grb[1], 0];
            }
        }
        self.controller.render()?;
        Ok(())
    }
}
