// Minimal WS2812 spike: fill the 32-LED pHAT green for 2 seconds, then clear.
#[cfg(feature = "hardware")]
fn main() -> anyhow::Result<()> {
    use rs_ws281x::{ChannelBuilder, ControllerBuilder, StripType};
    let mut controller = ControllerBuilder::new()
        .freq(800_000)
        .dma(10) // default; NEVER 5 (filesystem corruption)
        .channel(
            0,
            ChannelBuilder::new()
                .pin(18)
                .count(32)
                .strip_type(StripType::Ws2812)
                .brightness(40) // low: brown-out safety during the spike
                .build(),
        )
        .build()?;
    for led in controller.leds_mut(0) {
        *led = [0, 255, 0, 0]; // [B, G, R, W]; confirm observed colour on-device
    }
    controller.render()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    for led in controller.leds_mut(0) {
        *led = [0, 0, 0, 0];
    }
    controller.render()?;
    Ok(())
}

#[cfg(not(feature = "hardware"))]
fn main() {
    eprintln!("blink requires --features hardware (run on the Pi)");
}
