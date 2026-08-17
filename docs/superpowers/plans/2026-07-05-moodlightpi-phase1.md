# MoodLightPi Phase 1 (Core + REST + Web UI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust/Axum service that drives a Pimoroni Unicorn pHAT (8×4 WS2812 on GPIO18) on a Raspberry Pi Zero W as an IoT mood light, controllable via REST and an embedded web UI.

**Architecture:** A single-writer render engine owns the `Display` and the authoritative state; adapters (REST, WebSocket) talk to it only through a bounded command channel (in) and a `watch` state snapshot + `broadcast` frame stream (out). A `Display` trait has a real `rs_ws281x` impl (feature `hardware`) and a faithful in-memory mock so all logic is testable on the host.

**Tech Stack:** Rust 2021, Tokio (current-thread), Axum 0.7, `rs_ws281x` 0.5.1, `palette`, `rust-embed`, `serde`, `tracing`. Cross-compiled to `arm-unknown-linux-gnueabihf` via a custom `cross` image.

**Reference spec:** `docs/superpowers/specs/2026-07-05-moodlightpi-phase1-core-rest-webui-design.md`

---

## Conventions used throughout

- **Logical coordinates:** `WIDTH = 8` (x, 0..7), `HEIGHT = 4` (y, 0..3), origin top-left. A `Frame` is `[Rgb; 32]` indexed `frame[y * WIDTH + x]`.
- **Brightness:** `u8` 0–255 everywhere.
- **Commit after every task** (each task ends with a commit step).
- Run all host tests with the default (mock) feature set: `cargo test`. The `hardware` feature is **only** enabled for the Pi build.

## File structure (created across the plan)

```
Cargo.toml                     # crate manifest + release profile
Cross.toml                     # points cross at the custom image
docker/Dockerfile.armv6        # ARMv6 cross toolchain (gcc + libclang + git)
src/main.rs                    # startup ordering, wiring, graceful shutdown
src/color.rs                   # Rgb, gamma LUT, gamma→brightness→GRB pipeline
src/geometry.rs                # WIDTH/HEIGHT, Frame, pHAT pixel map
src/display.rs                 # Display trait + MockDisplay
src/hardware.rs                # Ws281xDisplay (cfg feature = "hardware")
src/state.rs                   # State, Mode, defaults, serde
src/effects.rs                 # effect fns, registry, render_frame, speed map
src/persist.rs                 # atomic save/load + rate-limited Persister
src/engine.rs                  # Command, StateSnapshot, channels, RenderEngine
src/api.rs                     # Axum router, REST handlers, security layer
src/ws.rs                      # /ws websocket handler
src/web.rs                     # rust-embed asset serving
web/{index.html,app.js,style.css}
deploy/{moodlightpi.service,provision.sh,deploy.sh}
```

---

# Milestone 0 — Toolchain & hardware spike (highest risk first)

The single biggest risk is the ARMv6 cross-build + WS2812/PWM/DMA path. Prove it end-to-end before writing the app.

### Task 1: Project scaffold + release profile

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Create `.gitignore`**

```
/target
*.tmp
```

- [ ] **Step 2: Create `Cargo.toml`**

```toml
[package]
name = "moodlightpi"
version = "0.1.0"
edition = "2021"

[features]
default = []
hardware = ["dep:rs_ws281x"]

[dependencies]
axum = { version = "0.7", features = ["ws"] }
tokio = { version = "1", features = ["rt", "macros", "sync", "time", "signal", "net"] }
tower = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
palette = "0.7"
rust-embed = { version = "8", features = ["axum"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "1"
rs_ws281x = { version = "0.5.1", optional = true }

[profile.release]
opt-level = "z"
lto = true
panic = "abort"
codegen-units = 1
strip = true
```

- [ ] **Step 3: Create a trivial `src/main.rs`**

```rust
fn main() {
    println!("moodlightpi v{}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 4: Verify it builds and runs on the host**

Run: `cargo run`
Expected: prints `moodlightpi v0.1.0`

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "chore: scaffold moodlightpi crate with release profile"
```

### Task 2: Custom ARMv6 cross image + build

Vanilla `cross` cannot target Pi Zero/ARMv6, and `rs_ws281x` compiles a vendored C lib via `cc` + `bindgen` (needs `libclang` + `git`). This task builds a custom image and proves an ARMv6 binary is produced.

**Files:**
- Create: `docker/Dockerfile.armv6`
- Create: `Cross.toml`

- [ ] **Step 1: Create `docker/Dockerfile.armv6`**

```dockerfile
FROM ghcr.io/cross-rs/arm-unknown-linux-gnueabihf:0.2.5

# rs_ws281x builds a vendored C library via cc + bindgen.
RUN apt-get update && apt-get install --assume-yes --no-install-recommends \
        libclang-dev clang git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Ensure the C toolchain emits ARMv6 (ARM1176), not ARMv7.
ENV CFLAGS_arm_unknown_linux_gnueabihf="-march=armv6 -mfpu=vfp -mfloat-abi=hard"
ENV BINDGEN_EXTRA_CLANG_ARGS_arm_unknown_linux_gnueabihf="--target=arm-linux-gnueabihf -march=armv6"
```

- [ ] **Step 2: Create `Cross.toml`**

```toml
[target.arm-unknown-linux-gnueabihf]
dockerfile = "docker/Dockerfile.armv6"

[target.arm-unknown-linux-gnueabihf.env]
passthrough = ["RUST_LOG"]
```

- [ ] **Step 3: Install the target + cross (one-time, host)**

Run:
```bash
rustup target add arm-unknown-linux-gnueabihf
cargo install cross --locked || true
```
Expected: target installed; `cross --version` prints a version.

- [ ] **Step 4: Cross-build the trivial binary**

Run: `cross build --release --target arm-unknown-linux-gnueabihf`
Expected: build succeeds; produces `target/arm-unknown-linux-gnueabihf/release/moodlightpi`.

- [ ] **Step 5: Verify the binary is genuinely ARMv6**

Run: `file target/arm-unknown-linux-gnueabihf/release/moodlightpi`
Expected: contains `ARM, EABI5` and `version 1 (SYSV)` — and crucially NOT `armv7`. If your `file` reports the arch tag, confirm it does not say `v7`.

- [ ] **Step 6: Verify it runs on the Pi**

Run:
```bash
scp target/arm-unknown-linux-gnueabihf/release/moodlightpi root@192.168.1.230:/tmp/mlp-smoke
ssh root@192.168.1.230 /tmp/mlp-smoke
```
Expected: prints `moodlightpi v0.1.0` (NOT `Illegal instruction`). An illegal-instruction crash means the build emitted ARMv7 — revisit the CFLAGS in Step 1.

- [ ] **Step 7: Commit**

```bash
git add docker/Dockerfile.armv6 Cross.toml
git commit -m "build: custom ARMv6 cross image for Pi Zero W"
```

### Task 3: One-time Pi provisioning (audio off) + LED blink spike

Proves PWM/DMA + the audio-conflict fix on the real board, behind the `hardware` feature.

**Files:**
- Create: `src/bin/blink.rs`
- Create: `deploy/provision.sh`

- [ ] **Step 1: Create `deploy/provision.sh` (idempotent)**

```bash
#!/usr/bin/env bash
# Run ON the Pi as root. Idempotent one-time provisioning.
set -euo pipefail
CONFIG=/boot/firmware/config.txt
BLACKLIST=/etc/modprobe.d/snd-blacklist.conf
STATE_DIR=/var/lib/moodlightpi

# 1. Free GPIO18/PWM from onboard audio (primary fix).
if ! grep -q '^dtparam=audio=off' "$CONFIG"; then
  sed -i 's/^dtparam=audio=on/dtparam=audio=off/' "$CONFIG" || true
  grep -q '^dtparam=audio=off' "$CONFIG" || echo 'dtparam=audio=off' >> "$CONFIG"
  echo "config.txt: audio disabled"
fi
# 2. Backstop: blacklist the module.
if [ ! -f "$BLACKLIST" ]; then
  echo 'blacklist snd_bcm2835' > "$BLACKLIST"
  echo "wrote $BLACKLIST"
fi
# 3. State directory.
mkdir -p "$STATE_DIR"
echo "provision complete — REBOOT REQUIRED for audio change to take effect"
```

- [ ] **Step 2: Run provisioning on the Pi and reboot**

Run:
```bash
scp deploy/provision.sh root@192.168.1.230:/tmp/provision.sh
ssh root@192.168.1.230 'bash /tmp/provision.sh && reboot'
```
Expected: "provision complete"; the Pi reboots. Wait ~30s for it to return.

- [ ] **Step 3: Create `src/bin/blink.rs` (only meaningful with `--features hardware`)**

```rust
// Minimal WS2812 spike: fill the 32-LED pHAT green for 2 seconds, then clear.
#[cfg(feature = "hardware")]
fn main() -> anyhow::Result<()> {
    use rs_ws281x::{ChannelBuilder, ControllerBuilder, StripType};
    let mut controller = ControllerBuilder::new()
        .freq(800_000)
        .dma(10) // default; NEVER 5 (filesystem corruption)
        .channel(0, ChannelBuilder::new()
            .pin(18)
            .count(32)
            .strip_type(StripType::Ws2812)
            .brightness(40) // low: brown-out safety during the spike
            .build())
        .build()?;
    for led in controller.leds_mut(0) {
        *led = [0, 255, 0, 0]; // rs_ws281x is [B,G,R,W]? verify order on-device
    }
    controller.render()?;
    std::thread::sleep(std::time::Duration::from_secs(2));
    for led in controller.leds_mut(0) { *led = [0, 0, 0, 0]; }
    controller.render()?;
    Ok(())
}

#[cfg(not(feature = "hardware"))]
fn main() {
    eprintln!("blink requires --features hardware (run on the Pi)");
}
```

- [ ] **Step 4: Cross-build the blink spike with hardware feature**

Run: `cross build --release --features hardware --bin blink --target arm-unknown-linux-gnueabihf`
Expected: compiles (this exercises the vendored C build + bindgen in the cross image).

- [ ] **Step 5: Deploy and run on the Pi (as root)**

Run:
```bash
scp target/arm-unknown-linux-gnueabihf/release/blink root@192.168.1.230:/tmp/blink
ssh root@192.168.1.230 /tmp/blink
```
Expected: the pHAT lights for 2 seconds then clears. **Note the observed color** — if it shows red instead of green, the byte order in `led = [..]` is confirmed as GRB-ish and we lock the mapping in Task 6/Task 9. No `WS2811_ERROR_MMAP` (that means not root) and no segfault (that means audio not disabled / reboot skipped).

- [ ] **Step 6: Commit**

```bash
git add src/bin/blink.rs deploy/provision.sh
git commit -m "spike: LED blink proves ARMv6 + PWM/DMA + audio-off path"
```

---

# Milestone 1 — Color, geometry, Display (pure + testable)

### Task 4: `Rgb` + gamma LUT + output pipeline

**Files:**
- Create: `src/color.rs`
- Modify: `src/main.rs` (add `mod color;`)
- Test: inline `#[cfg(test)]` in `src/color.rs`

- [ ] **Step 1: Write the failing test**

In `src/color.rs`:
```rust
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
        for v in 0u8..255 { assert!(gamma_correct(v) <= gamma_correct(v + 1)); }
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
        assert_eq!(to_grb(Rgb { r: 255, g: 255, b: 255 }, 0), [0, 0, 0]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test color::`
Expected: FAIL — `gamma_correct`, `Rgb`, `to_grb` not found.

- [ ] **Step 3: Implement `src/color.rs`**

```rust
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
    [scale(rgb.g, brightness), scale(rgb.r, brightness), scale(rgb.b, brightness)]
}
```

Add to `src/main.rs`: `mod color;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test color::`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/color.rs src/main.rs
git commit -m "feat: Rgb type + gamma LUT + gamma->brightness->GRB pipeline"
```

### Task 5: Geometry + authoritative pHAT pixel map

**Files:**
- Create: `src/geometry.rs`
- Modify: `src/main.rs` (add `mod geometry;`)

- [ ] **Step 1: Write the failing test**

In `src/geometry.rs`:
```rust
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
        // PHAT[x][y]: column-major table from unicornhat.py.
        assert_eq!(xy_to_index(0, 0), 24);
        assert_eq!(xy_to_index(0, 3), 0);
        assert_eq!(xy_to_index(7, 0), 31);
        assert_eq!(xy_to_index(7, 3), 7);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test geometry::`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement `src/geometry.rs`**

```rust
use crate::color::Rgb;

pub const WIDTH: usize = 8;
pub const HEIGHT: usize = 4;
pub const PIXEL_COUNT: usize = WIDTH * HEIGHT;

/// A logical frame indexed `frame[y * WIDTH + x]`.
pub type Frame = [Rgb; PIXEL_COUNT];

pub const BLACK_FRAME: Frame = [Rgb::BLACK; PIXEL_COUNT];

/// Authoritative Unicorn pHAT map (PHAT[x][y]) from
/// pimoroni/unicorn-hat library/UnicornHat/unicornhat.py (v2.2.3).
/// Column-major. Orientation confirmed on-device (checklist #6); if the
/// physical origin differs, flip y here and update the corner test.
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
```

Add to `src/main.rs`: `mod geometry;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test geometry::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/geometry.rs src/main.rs
git commit -m "feat: pHAT geometry + authoritative pixel map with golden tests"
```

### Task 6: `Display` trait + `MockDisplay`

**Files:**
- Create: `src/display.rs`
- Modify: `src/main.rs` (add `mod display;`)

- [ ] **Step 1: Write the failing test**

In `src/display.rs`:
```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test display::`
Expected: FAIL — `MockDisplay` not found.

- [ ] **Step 3: Implement `src/display.rs`**

```rust
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
        Self { strip: [[0; 3]; PIXEL_COUNT] }
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
```

Add to `src/main.rs`: `mod display;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test display::`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/display.rs src/main.rs
git commit -m "feat: Display trait + faithful MockDisplay (GRB+gamma+brightness)"
```

### Task 7: `Ws281xDisplay` (hardware impl)

Compiles only under `--features hardware`; no host test (verified on-device via Task 3 + checklist).

**Files:**
- Create: `src/hardware.rs`
- Modify: `src/main.rs` (add `#[cfg(feature = "hardware")] mod hardware;`)

- [ ] **Step 1: Implement `src/hardware.rs`**

```rust
use crate::color::to_grb;
use crate::display::Display;
use crate::geometry::{xy_to_index, Frame, HEIGHT, WIDTH};
use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};

pub struct Ws281xDisplay {
    controller: Controller,
}

impl Ws281xDisplay {
    /// `dma_channel` defaults to 10; NEVER 5 (filesystem corruption).
    pub fn new(dma_channel: i32) -> anyhow::Result<Self> {
        let controller = ControllerBuilder::new()
            .freq(800_000)
            .dma(dma_channel)
            .channel(0, ChannelBuilder::new()
                .pin(18)
                .count(crate::geometry::PIXEL_COUNT as i32)
                .strip_type(StripType::Ws2812)
                .brightness(255) // brightness applied by our pipeline, not here
                .build())
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
                // rs_ws281x expects [B, G, R, W] per LED for a GRB strip written
                // as raw bytes; grb = [G, R, B]. Map explicitly and confirm on-device.
                leds[idx] = [grb[2], grb[0], grb[1], 0];
            }
        }
        self.controller.render()?;
        Ok(())
    }
}
```

> **On-device note:** the exact byte permutation into `rs_ws281x` is confirmed by checklist #7 (`{255,0,0}` must show red). If it shows green/blue, adjust the `leds[idx] = [...]` line only.

- [ ] **Step 2: Verify the hardware feature compiles (host, no link run)**

Run: `cargo build --features hardware` (on the host this needs libclang locally; if unavailable, defer to the cross build)
Alternate: `cross build --features hardware --target arm-unknown-linux-gnueabihf`
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add src/hardware.rs src/main.rs
git commit -m "feat: Ws281xDisplay hardware backend behind 'hardware' feature"
```

---

# Milestone 2 — State & effects (pure + testable)

### Task 8: `State`, `Mode`, defaults, serde

**Files:**
- Create: `src/state.rs`
- Modify: `src/main.rs` (add `mod state;`)

- [ ] **Step 1: Write the failing test**

In `src/state.rs`:
```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test state::`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement `src/state.rs`**

```rust
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
```

Add to `src/main.rs`: `mod state;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test state::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/state.rs src/main.rs
git commit -m "feat: State model with power gate + brown-out brightness cap"
```

### Task 9: Effects + registry + `render_frame`

**Files:**
- Create: `src/effects.rs`
- Modify: `src/main.rs` (add `mod effects;`)

- [ ] **Step 1: Write the failing test**

In `src/effects.rs`:
```rust
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
        let s = State { mode: Mode::Solid, rgb: Rgb { r: 10, g: 20, b: 30 }, ..State::default() };
        let frame = render_frame(&s, 0);
        assert!(frame.iter().all(|&p| p == Rgb { r: 10, g: 20, b: 30 }));
    }

    #[test]
    fn rainbow_changes_over_time_and_fills_all_pixels() {
        let s = State { mode: Mode::Effect, effect_name: "rainbow".into(), speed: 128, ..State::default() };
        let f0 = render_frame(&s, 0);
        let f1 = render_frame(&s, 10);
        assert_eq!(f0.len(), PIXEL_COUNT);
        assert_ne!(f0, f1, "rainbow should animate across ticks");
    }

    #[test]
    fn breathe_returns_state_hue_but_varies_value() {
        let s = State { mode: Mode::Effect, effect_name: "breathe".into(),
                        rgb: Rgb { r: 255, g: 0, b: 0 }, speed: 128, ..State::default() };
        let dim = render_frame(&s, 0);
        let bright = render_frame(&s, 32);
        assert_ne!(dim[0], bright[0], "breathe should vary brightness of the pixel");
    }

    #[test]
    fn unknown_effect_falls_back_to_solid() {
        let s = State { mode: Mode::Effect, effect_name: "bogus".into(),
                        rgb: Rgb { r: 1, g: 2, b: 3 }, ..State::default() };
        let frame = render_frame(&s, 5);
        assert!(frame.iter().all(|&p| p == Rgb { r: 1, g: 2, b: 3 }));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test effects::`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement `src/effects.rs`**

```rust
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
    Rgb { r: rgb.red, g: rgb.green, b: rgb.blue }
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
    fill(hsv_to_rgb(base.hue.into_positive_degrees(), base.saturation, v))
}
```

Add to `src/main.rs`: `mod effects;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test effects::`
Expected: PASS (5 tests). If `breathe`'s hue extraction needs a different palette call, adjust to compile; the behavioural asserts must still hold.

- [ ] **Step 5: Commit**

```bash
git add src/effects.rs src/main.rs
git commit -m "feat: effects (solid/rainbow/colorcycle/breathe) + registry + render_frame"
```

---

# Milestone 3 — Persistence

### Task 10: Atomic, rate-limited persistence

**Files:**
- Create: `src/persist.rs`
- Modify: `src/main.rs` (add `mod persist;`)
- Test: inline + a tempdir-based test

- [ ] **Step 1: Write the failing test**

In `src/persist.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("mlp-test-{}-{}.json", name, std::process::id()));
        p
    }

    #[test]
    fn load_missing_returns_default() {
        let p = tmp_path("missing");
        let _ = std::fs::remove_file(&p);
        assert_eq!(load(&p), State::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let p = tmp_path("roundtrip");
        let s = State { brightness: 123, ..State::default() };
        save_atomic(&p, &s).unwrap();
        assert_eq!(load(&p), s);
        assert!(!p.with_extension("json.tmp").exists(), "temp file must be renamed away");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_corrupt_returns_default() {
        let p = tmp_path("corrupt");
        std::fs::write(&p, b"{ not json").unwrap();
        assert_eq!(load(&p), State::default());
        let _ = std::fs::remove_file(&p);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test persist::`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement `src/persist.rs`**

```rust
use crate::state::State;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_PATH: &str = "/var/lib/moodlightpi/state.json";

/// Load state; any error (missing/corrupt) yields the safe default.
pub fn load(path: &Path) -> State {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!("corrupt state file, using default: {e}");
            State::default()
        }),
        Err(_) => State::default(),
    }
}

/// Atomic write: temp file in the same dir -> fsync -> rename over target.
pub fn save_atomic(path: &Path, state: &State) -> anyhow::Result<()> {
    let tmp: PathBuf = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(state)?;
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Rate-limited persister: skips writes when unchanged and enforces a
/// minimum interval between disk writes. Call `flush` on shutdown.
pub struct Persister {
    path: PathBuf,
    min_interval: std::time::Duration,
    last_written: Option<std::time::Instant>,
    last_state: Option<State>,
    pending: Option<State>,
}

impl Persister {
    pub fn new(path: PathBuf, min_interval: std::time::Duration) -> Self {
        let existing = load(&path);
        Self {
            path,
            min_interval,
            last_written: None,
            last_state: Some(existing),
            pending: None,
        }
    }

    /// Record a new state; writes to disk only if changed and enough time
    /// has elapsed, otherwise stashes it as pending for the next `maybe_flush`.
    pub fn record(&mut self, state: &State) {
        if self.last_state.as_ref() == Some(state) {
            return;
        }
        let due = self
            .last_written
            .map(|t| t.elapsed() >= self.min_interval)
            .unwrap_or(true);
        if due {
            self.write(state);
        } else {
            self.pending = Some(state.clone());
        }
    }

    /// Flush any pending state ignoring the rate limit (call on shutdown).
    pub fn flush(&mut self) {
        if let Some(state) = self.pending.take() {
            self.write(&state);
        }
    }

    fn write(&mut self, state: &State) {
        if let Err(e) = save_atomic(&self.path, state) {
            tracing::error!("failed to persist state: {e}");
            return;
        }
        self.last_written = Some(std::time::Instant::now());
        self.last_state = Some(state.clone());
        self.pending = None;
    }
}
```

Add to `src/main.rs`: `mod persist;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test persist::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/persist.rs src/main.rs
git commit -m "feat: atomic + rate-limited state persistence with default fallback"
```

---

# Milestone 4 — Engine & channels

### Task 11: Commands, snapshot, and the render engine

**Files:**
- Create: `src/engine.rs`
- Modify: `src/main.rs` (add `mod engine;`)

- [ ] **Step 1: Write the failing test**

In `src/engine.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::display::MockDisplay;
    use crate::state::Mode;

    fn apply(state: &mut crate::state::State, cmd: Command) {
        super::apply_command(state, cmd);
    }

    #[test]
    fn set_color_forces_solid_mode() {
        let mut s = crate::state::State { mode: Mode::Effect, ..Default::default() };
        apply(&mut s, Command::SetColor(Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(s.mode, Mode::Solid);
        assert_eq!(s.rgb, Rgb { r: 1, g: 2, b: 3 });
    }

    #[test]
    fn set_effect_switches_mode_and_keeps_last_speed_when_none() {
        let mut s = crate::state::State { speed: 77, ..Default::default() };
        apply(&mut s, Command::SetEffect { name: "rainbow".into(), speed: None });
        assert_eq!(s.mode, Mode::Effect);
        assert_eq!(s.effect_name, "rainbow");
        assert_eq!(s.speed, 77, "omitted speed keeps previous value");
    }

    #[test]
    fn power_gate_blanks_output_but_keeps_state() {
        let mut s = crate::state::State { power: true, brightness: 100, ..Default::default() };
        apply(&mut s, Command::SetPower(false));
        assert!(!s.power);
        assert_eq!(s.brightness, 100);
        assert_eq!(s.effective_brightness(), 0);
    }

    #[tokio::test]
    async fn engine_renders_a_frame_to_the_display() {
        let (handle, mut engine) = Engine::new(MockDisplay::new(), crate::state::State::default());
        // Drive one render tick manually.
        engine.render_once();
        // Solid default color present at logical (0,0) mapped index.
        let strip = engine.display_ref().last_strip();
        let idx = crate::geometry::xy_to_index(0, 0);
        assert_ne!(strip[idx], [0, 0, 0]);
        drop(handle);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test engine::`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement `src/engine.rs`**

```rust
use crate::color::Rgb;
use crate::display::Display;
use crate::effects::render_frame;
use crate::state::{Mode, State};
use tokio::sync::{broadcast, mpsc, watch};

/// Who originated a command/snapshot (for future adapter echo-suppression).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Rest,
    WebSocket,
    Internal,
}

#[derive(Clone, Debug)]
pub enum Command {
    SetPower(bool),
    SetColor(Rgb),
    SetBrightness(u8),
    SetEffect { name: String, speed: Option<u8> },
}

#[derive(Clone, Debug)]
pub struct StateSnapshot {
    pub state: State,
    pub seq: u64,
    pub source: Source,
}

/// Frame pushed to WS clients (logical order).
pub type FrameMsg = crate::geometry::Frame;

pub const COMMAND_CAPACITY: usize = 32;
const FRAME_CAPACITY: usize = 8;

/// Cloneable handle adapters use to reach the engine.
#[derive(Clone)]
pub struct EngineHandle {
    pub commands: mpsc::Sender<(Command, Source)>,
    pub snapshots: watch::Receiver<StateSnapshot>,
    pub frames: broadcast::Sender<FrameMsg>,
}

impl EngineHandle {
    pub fn current(&self) -> State {
        self.snapshots.borrow().state.clone()
    }
    pub fn subscribe_frames(&self) -> broadcast::Receiver<FrameMsg> {
        self.frames.subscribe()
    }
}

pub struct Engine<D: Display> {
    display: D,
    state: State,
    seq: u64,
    tick: u64,
    cmd_rx: mpsc::Receiver<(Command, Source)>,
    snap_tx: watch::Sender<StateSnapshot>,
    frame_tx: broadcast::Sender<FrameMsg>,
}

/// Pure state transition — unit tested directly.
pub fn apply_command(state: &mut State, cmd: Command) {
    match cmd {
        Command::SetPower(on) => state.power = on,
        Command::SetColor(rgb) => {
            state.rgb = rgb;
            state.mode = Mode::Solid;
        }
        Command::SetBrightness(v) => state.brightness = v,
        Command::SetEffect { name, speed } => {
            state.effect_name = name;
            state.mode = Mode::Effect;
            if let Some(s) = speed {
                state.speed = s;
            }
        }
    }
}

impl<D: Display> Engine<D> {
    pub fn new(display: D, state: State) -> (EngineHandle, Engine<D>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(COMMAND_CAPACITY);
        let snapshot = StateSnapshot { state: state.clone(), seq: 0, source: Source::Internal };
        let (snap_tx, snap_rx) = watch::channel(snapshot);
        let (frame_tx, _) = broadcast::channel(FRAME_CAPACITY);
        let handle = EngineHandle {
            commands: cmd_tx,
            snapshots: snap_rx,
            frames: frame_tx.clone(),
        };
        let engine = Engine {
            display, state, seq: 0, tick: 0, cmd_rx, snap_tx, frame_tx,
        };
        (handle, engine)
    }

    #[cfg(test)]
    pub fn display_ref(&self) -> &D { &self.display }

    /// Render the current state once (used by tests and the solid path).
    pub fn render_once(&mut self) {
        let frame = render_frame(&self.state, self.tick);
        let _ = self.display.show(&frame, self.state.effective_brightness());
        let _ = self.frame_tx.send(frame);
    }

    fn drain_commands(&mut self) -> bool {
        let mut changed = false;
        while let Ok((cmd, source)) = self.cmd_rx.try_recv() {
            apply_command(&mut self.state, cmd);
            self.seq += 1;
            let _ = self.snap_tx.send(StateSnapshot {
                state: self.state.clone(), seq: self.seq, source,
            });
            changed = true;
        }
        changed
    }

    /// The single-writer run loop. Owns the Display for its whole life.
    pub async fn run(mut self) {
        use tokio::time::{interval, Duration, MissedTickBehavior};
        let mut ticker = interval(Duration::from_millis(33)); // ~30 fps
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            let animating = matches!(self.state.mode, Mode::Effect) && self.state.power;
            if animating {
                tokio::select! {
                    _ = ticker.tick() => { self.tick = self.tick.wrapping_add(1); }
                    n = self.cmd_rx.recv() => {
                        match n {
                            Some((cmd, source)) => {
                                apply_command(&mut self.state, cmd);
                                self.seq += 1;
                                let _ = self.snap_tx.send(StateSnapshot {
                                    state: self.state.clone(), seq: self.seq, source });
                            }
                            None => break, // all senders dropped -> shutdown
                        }
                        self.drain_commands();
                    }
                }
            } else {
                // Idle: wake only on a command.
                match self.cmd_rx.recv().await {
                    Some((cmd, source)) => {
                        apply_command(&mut self.state, cmd);
                        self.seq += 1;
                        let _ = self.snap_tx.send(StateSnapshot {
                            state: self.state.clone(), seq: self.seq, source });
                        self.drain_commands();
                    }
                    None => break,
                }
            }
            self.render_once();
        }
        let _ = self.display.clear();
    }
}
```

Add to `src/main.rs`: `mod engine;`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test engine::`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/engine.rs src/main.rs
git commit -m "feat: render engine — single-writer loop, bounded cmd channel, snapshots+frames"
```

---

# Milestone 5 — REST API + security

### Task 12: Security layer (Origin/Host/Content-Type)

**Files:**
- Create: `src/api.rs`
- Modify: `src/main.rs` (add `mod api;`)

- [ ] **Step 1: Write the failing test**

In `src/api.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_lan_host() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(cfg.host_ok(Some("192.168.1.230")));
        assert!(cfg.host_ok(Some("192.168.1.230:80")));
    }

    #[test]
    fn rejects_foreign_host_and_missing() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        assert!(!cfg.host_ok(Some("evil.example.com")));
        assert!(!cfg.host_ok(None));
    }

    #[test]
    fn origin_ok_only_for_expected_or_absent_nonbrowser() {
        let cfg = SecurityConfig { allowed_host: "192.168.1.230".into() };
        // Browser-set Origin must match; curl (no Origin) is allowed.
        assert!(cfg.origin_ok(Some("http://192.168.1.230")));
        assert!(cfg.origin_ok(None));
        assert!(!cfg.origin_ok(Some("http://evil.example.com")));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test api::`
Expected: FAIL — `SecurityConfig` not found.

- [ ] **Step 3: Implement the security config in `src/api.rs`**

```rust
#[derive(Clone)]
pub struct SecurityConfig {
    /// Host (no scheme/port) the service is reached at on the LAN.
    pub allowed_host: String,
}

impl SecurityConfig {
    pub fn host_ok(&self, host_header: Option<&str>) -> bool {
        match host_header {
            Some(h) => h.split(':').next() == Some(self.allowed_host.as_str()),
            None => false,
        }
    }
    /// Origin is only sent by browsers. Absent = non-browser client (curl) = allow.
    /// Present = must match the expected host (defeats DNS-rebinding/CSRF).
    pub fn origin_ok(&self, origin_header: Option<&str>) -> bool {
        match origin_header {
            None => true,
            Some(o) => o
                .strip_prefix("http://")
                .or_else(|| o.strip_prefix("https://"))
                .map(|rest| rest.split(':').next() == Some(self.allowed_host.as_str()))
                .unwrap_or(false),
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test api::`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/api.rs src/main.rs
git commit -m "feat: security config — Origin/Host validation (DNS-rebind/CSRF guard)"
```

### Task 13: REST handlers + router

**Files:**
- Modify: `src/api.rs`

- [ ] **Step 1: Write the failing test (handler behavior via the app)**

Append to `src/api.rs` tests module:
```rust
    use crate::display::MockDisplay;
    use crate::engine::Engine;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // oneshot

    async fn test_app() -> axum::Router {
        let (handle, engine) = Engine::new(MockDisplay::new(), crate::state::State::default());
        tokio::spawn(engine.run());
        router(AppState {
            engine: handle,
            security: SecurityConfig { allowed_host: "testhost".into() },
        })
    }

    #[tokio::test]
    async fn healthz_ok() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/healthz").header("host", "testhost")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn effects_lists_registry() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/api/effects").header("host", "testhost")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_effect_is_400() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().method("POST").uri("/api/effect")
                .header("host", "testhost").header("content-type", "application/json")
                .body(Body::from(r#"{"name":"bogus"}"#)).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn foreign_host_is_403() {
        let app = test_app().await;
        let res = app.oneshot(
            Request::builder().uri("/api/state").header("host", "evil.com")
                .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test api::`
Expected: FAIL — `router`, `AppState` not found.

- [ ] **Step 3: Implement handlers + router in `src/api.rs`**

```rust
use crate::effects::{effect_names, is_valid_effect};
use crate::engine::{Command, EngineHandle, Source};
use crate::color::Rgb;
use axum::extract::State as AxState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
pub struct AppState {
    pub engine: EngineHandle,
    pub security: SecurityConfig,
}

fn check(headers: &HeaderMap, sec: &SecurityConfig) -> Result<(), StatusCode> {
    let host = headers.get("host").and_then(|v| v.to_str().ok());
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    if !sec.host_ok(host) || !sec.origin_ok(origin) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

async fn send(engine: &EngineHandle, cmd: Command) -> Result<(), StatusCode> {
    engine.commands.try_send((cmd, Source::Rest))
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE) // bounded channel full -> load-shed
}

pub async fn healthz(AxState(st): AxState<AppState>) -> impl IntoResponse {
    // Hardware liveness is reported by main via a shared flag in a later task;
    // here the HTTP layer is alive if it responds.
    Json(json!({ "alive": true, "hardware": "ok" }))
}

async fn get_state(headers: HeaderMap, AxState(st): AxState<AppState>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    let snap = st.engine.snapshots.borrow().clone();
    Ok(Json(json!({ "state": snap.state, "seq": snap.seq })))
}

async fn get_effects(headers: HeaderMap, AxState(st): AxState<AppState>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    Ok(Json(json!({ "effects": effect_names() })))
}

#[derive(Deserialize)] struct PowerBody { on: bool }
#[derive(Deserialize)] struct ColorBody { r: u8, g: u8, b: u8 }
#[derive(Deserialize)] struct BrightnessBody { value: u8 }
#[derive(Deserialize)] struct EffectBody { name: String, speed: Option<u8> }

async fn post_power(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<PowerBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetPower(b.on)).await?;
    Ok(StatusCode::OK)
}

async fn post_color(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<ColorBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetColor(Rgb { r: b.r, g: b.g, b: b.b })).await?;
    Ok(StatusCode::OK)
}

async fn post_brightness(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<BrightnessBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    send(&st.engine, Command::SetBrightness(b.value)).await?;
    Ok(StatusCode::OK)
}

async fn post_effect(headers: HeaderMap, AxState(st): AxState<AppState>, Json(b): Json<EffectBody>) -> Result<impl IntoResponse, StatusCode> {
    check(&headers, &st.security)?;
    if !is_valid_effect(&b.name) {
        return Err(StatusCode::BAD_REQUEST);
    }
    send(&st.engine, Command::SetEffect { name: b.name, speed: b.speed }).await?;
    Ok(StatusCode::OK)
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/state", get(get_state))
        .route("/api/effects", get(get_effects))
        .route("/api/power", post(post_power))
        .route("/api/color", post(post_color))
        .route("/api/brightness", post(post_brightness))
        .route("/api/effect", post(post_effect))
        .with_state(state)
}
```

Add to `src/main.rs`: ensure `mod api;` exists.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test api::`
Expected: PASS (7 tests total in api).

- [ ] **Step 5: Commit**

```bash
git add src/api.rs src/main.rs
git commit -m "feat: REST API — state/effects/power/color/brightness/effect + host checks"
```

---

# Milestone 6 — WebSocket + Web UI

### Task 14: `/ws` frame stream

**Files:**
- Create: `src/ws.rs`
- Modify: `src/api.rs` (add the `/ws` route), `src/main.rs` (add `mod ws;`)

- [ ] **Step 1: Write the failing test (message shape is pure/testable)**

In `src/ws.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::geometry::BLACK_FRAME;

    #[test]
    fn frame_serializes_to_flat_rgb_triples() {
        let mut frame = BLACK_FRAME;
        frame[0] = Rgb { r: 1, g: 2, b: 3 };
        let json = frame_to_json(&frame);
        // 32 pixels, each [r,g,b]
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["pixels"].as_array().unwrap().len(), 32);
        assert_eq!(v["pixels"][0][0], 1);
        assert_eq!(v["pixels"][0][2], 3);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test ws::`
Expected: FAIL — `frame_to_json` not found.

- [ ] **Step 3: Implement `src/ws.rs`**

```rust
use crate::api::{AppState, SecurityConfig};
use crate::geometry::Frame;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State as AxState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

pub fn frame_to_json(frame: &Frame) -> String {
    let pixels: Vec<[u8; 3]> = frame.iter().map(|p| [p.r, p.g, p.b]).collect();
    json!({ "pixels": pixels }).to_string()
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    AxState(st): AxState<AppState>,
) -> impl IntoResponse {
    // Validate Origin on the upgrade (browser-set, unspoofable by page JS).
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    let host = headers.get("host").and_then(|v| v.to_str().ok());
    if !st.security.origin_ok(origin) || !st.security.host_ok(host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| client_loop(socket, st))
}

async fn client_loop(mut socket: WebSocket, st: AppState) {
    // 1. Send the current frame immediately (solid mode is otherwise idle).
    let current = crate::effects::render_frame(&st.engine.current(), 0);
    if socket.send(Message::Text(frame_to_json(&current))).await.is_err() {
        return;
    }
    // 2. Stream frames; drop this client on lag (never back-pressure the engine).
    let mut rx = st.engine.subscribe_frames();
    loop {
        match rx.recv().await {
            Ok(frame) => {
                if socket.send(Message::Text(frame_to_json(&frame))).await.is_err() {
                    break; // client gone
                }
            }
            Err(RecvError::Lagged(_)) => continue, // slow client: skip missed frames
            Err(RecvError::Closed) => break,
        }
    }
}
```

- [ ] **Step 4: Add the route in `src/api.rs`**

In `router()`, add before `.with_state(state)`:
```rust
        .route("/ws", get(crate::ws::ws_handler))
```
Add to `src/main.rs`: `mod ws;`

- [ ] **Step 5: Run the test + full build**

Run: `cargo test ws::` then `cargo build`
Expected: PASS; builds clean.

- [ ] **Step 6: Commit**

```bash
git add src/ws.rs src/api.rs src/main.rs
git commit -m "feat: /ws frame stream — on-connect snapshot, origin-checked, drop-slow-clients"
```

### Task 15: Embedded web UI

**Files:**
- Create: `web/index.html`, `web/style.css`, `web/app.js`
- Create: `src/web.rs`
- Modify: `src/api.rs` (serve embedded assets), `src/main.rs` (add `mod web;`)

- [ ] **Step 1: Create `web/index.html`**

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>MoodLightPi</title>
<link rel="stylesheet" href="/style.css">
</head>
<body>
<main>
  <h1>MoodLightPi</h1>
  <canvas id="preview" width="320" height="160" aria-label="LED preview"></canvas>
  <div class="row">
    <button id="power">Toggle Power</button>
  </div>
  <label>Color <input type="color" id="color" value="#ff9329"></label>
  <label>Brightness <input type="range" id="brightness" min="0" max="255" value="80"></label>
  <label>Effect
    <select id="effect"></select>
  </label>
  <label>Speed <input type="range" id="speed" min="0" max="255" value="128"></label>
</main>
<script src="/app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Create `web/style.css`**

```css
:root { color-scheme: dark; }
body { margin: 0; background: #111; color: #eee; font-family: system-ui, sans-serif; }
main { max-width: 420px; margin: 0 auto; padding: 1.25rem; display: flex; flex-direction: column; gap: 1rem; }
h1 { font-size: 1.25rem; text-align: center; margin: .5rem 0; }
canvas { width: 100%; height: auto; background: #000; border-radius: 8px; image-rendering: pixelated; }
label { display: flex; flex-direction: column; gap: .35rem; font-size: .9rem; }
input[type=range], select { width: 100%; }
button { padding: .75rem; font-size: 1rem; border: 0; border-radius: 8px; background: #2b6; color: #071; font-weight: 600; }
.row { display: flex; gap: .5rem; }
```

- [ ] **Step 3: Create `web/app.js`**

```javascript
const WIDTH = 8, HEIGHT = 4;
const canvas = document.getElementById('preview');
const ctx = canvas.getContext('2d');
const cell = canvas.width / WIDTH;

function draw(pixels) {
  for (let y = 0; y < HEIGHT; y++) {
    for (let x = 0; x < WIDTH; x++) {
      const [r, g, b] = pixels[y * WIDTH + x];
      ctx.fillStyle = `rgb(${r},${g},${b})`;
      ctx.fillRect(x * cell, y * cell, cell, cell);
    }
  }
}

function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (e) => { draw(JSON.parse(e.data).pixels); };
  ws.onclose = () => setTimeout(connect, 1000); // auto-reconnect
}
connect();

async function post(path, body) {
  await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
}

document.getElementById('power').onclick = async () => {
  const s = await (await fetch('/api/state')).json();
  await post('/api/power', { on: !s.state.power });
};
document.getElementById('color').oninput = (e) => {
  const v = e.target.value; // #rrggbb
  post('/api/color', {
    r: parseInt(v.slice(1, 3), 16),
    g: parseInt(v.slice(3, 5), 16),
    b: parseInt(v.slice(5, 7), 16),
  });
};
document.getElementById('brightness').oninput = (e) =>
  post('/api/brightness', { value: parseInt(e.target.value, 10) });
document.getElementById('speed').oninput = (e) => {
  const name = document.getElementById('effect').value;
  post('/api/effect', { name, speed: parseInt(e.target.value, 10) });
};
document.getElementById('effect').onchange = (e) =>
  post('/api/effect', { name: e.target.value });

(async () => {
  const { effects } = await (await fetch('/api/effects')).json();
  const sel = document.getElementById('effect');
  for (const name of effects) {
    const opt = document.createElement('option');
    opt.value = name; opt.textContent = name; sel.appendChild(opt);
  }
})();
```

- [ ] **Step 4: Implement `src/web.rs`**

```rust
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

pub async fn serve_index() -> impl IntoResponse {
    serve_path("index.html")
}

pub async fn serve_asset(uri: Uri) -> impl IntoResponse {
    serve_path(uri.path().trim_start_matches('/'))
}

fn serve_path(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data.into_owned()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
```

Add `mime_guess = "2"` to `Cargo.toml` dependencies (rust-embed re-exports it, but declare for clarity).

- [ ] **Step 5: Wire routes in `src/api.rs` `router()`**

Add:
```rust
        .route("/", get(crate::web::serve_index))
        .route("/style.css", get(crate::web::serve_asset))
        .route("/app.js", get(crate::web::serve_asset))
```
Add to `src/main.rs`: `mod web;`

- [ ] **Step 6: Build to verify embedding compiles**

Run: `cargo build`
Expected: builds; assets embedded.

- [ ] **Step 7: Commit**

```bash
git add web src/web.rs src/api.rs src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: embedded OctoCam-style web UI with live canvas preview"
```

---

# Milestone 7 — Wiring, deployment, acceptance

### Task 16: `main.rs` — startup ordering, hardware selection, graceful shutdown

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace `src/main.rs` with the full wiring**

```rust
mod api;
mod color;
mod display;
mod effects;
mod engine;
mod geometry;
mod persist;
mod state;
mod web;
mod ws;
#[cfg(feature = "hardware")]
mod hardware;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

fn select_display() -> Box<dyn display::Display> {
    #[cfg(feature = "hardware")]
    {
        match hardware::Ws281xDisplay::new(10) {
            Ok(d) => {
                tracing::info!("hardware display initialised");
                return Box::new(d);
            }
            Err(e) => {
                // Keep the API up for diagnosis rather than crash-looping.
                tracing::error!("hardware init failed ({e}); running with mock display");
            }
        }
    }
    Box::new(display::MockDisplay::new())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // 1. Load state (or safe default).
    let state_path = PathBuf::from(persist::DEFAULT_PATH);
    let initial = persist::load(&state_path);

    // 2. Init hardware (falls back to mock on failure) and 3. start engine.
    let display = select_display();
    let (handle, engine) = engine::Engine::new_boxed(display, initial);
    tokio::spawn(engine.run());

    // Persist snapshots as they change (rate-limited, atomic).
    {
        let mut rx = handle.snapshots.clone();
        let path = state_path.clone();
        tokio::spawn(async move {
            let mut persister = persist::Persister::new(path, Duration::from_millis(1000));
            while rx.changed().await.is_ok() {
                let snap = rx.borrow().clone();
                persister.record(&snap.state);
            }
            persister.flush();
        });
    }

    // 4. Bind HTTP only after the engine is live.
    let allowed_host = std::env::var("MLP_HOST").unwrap_or_else(|_| "192.168.1.230".into());
    let app = api::router(api::AppState {
        engine: handle,
        security: api::SecurityConfig { allowed_host },
    });
    let bind = std::env::var("MLP_BIND").unwrap_or_else(|_| "192.168.1.230:80".into());
    let addr: SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutdown signal received");
        })
        .await?;
    Ok(())
}
```

- [ ] **Step 2: Add `Engine::new_boxed` in `src/engine.rs`**

Add an impl for the boxed trait object so `main` can hold a `Box<dyn Display>`:
```rust
impl Engine<Box<dyn crate::display::Display>> {
    pub fn new_boxed(
        display: Box<dyn crate::display::Display>,
        state: State,
    ) -> (EngineHandle, Engine<Box<dyn crate::display::Display>>) {
        Engine::new(display, state)
    }
}
```
And ensure `impl Display for Box<dyn Display>` exists in `src/display.rs`:
```rust
impl Display for Box<dyn Display> {
    fn show(&mut self, frame: &crate::geometry::Frame, brightness: u8) -> anyhow::Result<()> {
        (**self).show(frame, brightness)
    }
}
```

- [ ] **Step 3: Build for host and run the test suite**

Run: `cargo build && cargo test`
Expected: builds; all tests pass. (`main` binds to the LAN IP; running locally will fail to bind — that is expected off-device. Override with `MLP_BIND=127.0.0.1:8080 MLP_HOST=127.0.0.1 cargo run` to smoke-test locally.)

- [ ] **Step 4: Local smoke test**

Run: `MLP_BIND=127.0.0.1:8080 MLP_HOST=127.0.0.1 cargo run`
Then in another shell: `curl -s -H 'Host: 127.0.0.1' http://127.0.0.1:8080/api/state`
Expected: JSON state. Open `http://127.0.0.1:8080/` in a browser: the canvas shows the warm-white solid; switching the effect animates the preview.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/engine.rs src/display.rs
git commit -m "feat: wire startup ordering, hardware fallback, graceful shutdown"
```

### Task 17: systemd unit + deploy script

**Files:**
- Create: `deploy/moodlightpi.service`
- Create: `deploy/deploy.sh`

- [ ] **Step 1: Create `deploy/moodlightpi.service`**

```ini
[Unit]
Description=MoodLightPi
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=root
Environment=RUST_LOG=info
Environment=MLP_HOST=192.168.1.230
Environment=MLP_BIND=192.168.1.230:80
ExecStart=/usr/local/bin/moodlightpi
Restart=on-failure
RestartSec=2
TimeoutStopSec=5

[Install]
WantedBy=multi-user.target
```

- [ ] **Step 2: Create `deploy/deploy.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
PI=root@192.168.1.230
TARGET=arm-unknown-linux-gnueabihf
BIN=target/$TARGET/release/moodlightpi

cross build --release --features hardware --target "$TARGET"
file "$BIN" | grep -q 'ARM' || { echo "not an ARM binary"; exit 1; }

ssh "$PI" 'systemctl stop moodlightpi 2>/dev/null || true'
scp "$BIN" "$PI:/usr/local/bin/moodlightpi"
scp deploy/moodlightpi.service "$PI:/etc/systemd/system/moodlightpi.service"
ssh "$PI" 'systemctl daemon-reload && systemctl enable --now moodlightpi && systemctl status --no-pager moodlightpi'
```

- [ ] **Step 3: Make scripts executable + deploy**

Run:
```bash
chmod +x deploy/deploy.sh deploy/provision.sh
./deploy/deploy.sh
```
Expected: build succeeds, binary is ARM, service reports `active (running)`.

- [ ] **Step 4: Commit**

```bash
git add deploy/moodlightpi.service deploy/deploy.sh
git commit -m "build: systemd unit + one-command deploy script"
```

### Task 18: On-device acceptance (spec §12)

**Files:** none (verification only).

- [ ] **Step 1: Verify service + API on the Pi**

Run: `curl -s -H 'Host: 192.168.1.230' http://192.168.1.230/api/state`
Expected: JSON state; the pHAT shows the restored/default warm-white solid.

- [ ] **Step 2: Color + GRB order (checklist #7)**

Run: `curl -s -X POST -H 'Host: 192.168.1.230' -H 'content-type: application/json' -d '{"r":255,"g":0,"b":0}' http://192.168.1.230/api/color`
Expected: panel shows **red**. If green/blue, fix the byte permutation in `src/hardware.rs` `show()` only, redeploy.

- [ ] **Step 3: Pixel-map orientation (checklist #6)**

Temporarily set a per-corner test via the web UI effect or a one-off; confirm the physical corners match the browser canvas preview. If mirrored/rotated, flip `y` in `PHAT_MAP` (`src/geometry.rs`) and update the corner golden test, then redeploy.

- [ ] **Step 4: Effect + FPS + brown-out (checklist #4, #5, #8)**

Run: `curl -s -X POST -H 'Host: 192.168.1.230' -H 'content-type: application/json' -d '{"name":"rainbow","speed":200}' http://192.168.1.230/api/effect`
Then set brightness high: `-d '{"value":255}'` to `/api/brightness`.
Expected: smooth rainbow (not visibly banded — confirms gamma), no under-voltage reset (confirms the `MAX_BRIGHTNESS` cap). Watch `ssh root@192.168.1.230 dmesg | grep -i voltage` for throttling.

- [ ] **Step 5: Persistence across reboot**

Run: set a distinct color, then `ssh root@192.168.1.230 reboot`. After it returns, confirm the panel restores that color.
Expected: state restored from `/var/lib/moodlightpi/state.json`.

- [ ] **Step 6: SD-card safety under load (checklist #3)**

Run: leave an effect running ~10 minutes, then `ssh root@192.168.1.230 'dmesg | grep -iE "mmc|ext4|corrupt"'`
Expected: no filesystem errors (confirms DMA channel 10 is safe).

- [ ] **Step 7: Multi-client + security (checklist #10, #11)**

Open the web UI in two browser tabs — both preview live with no stutter. Confirm a cross-origin request is rejected:
Run: `curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.com' http://192.168.1.230/api/state`
Expected: `403`.

- [ ] **Step 8: Mark Phase 1 complete**

```bash
git commit --allow-empty -m "test: Phase 1 on-device acceptance passed (spec §12)"
```

---

## Self-review notes (author)

- **Spec coverage:** driver+map (T5–T7), Display/mock fidelity GRB+gamma (T4,T6), state+cap+power gate (T8), effects+registry+speed (T9), atomic/rate-limited persistence (T10), single-writer engine + bounded channel + snapshot attribution (T11), REST + Origin/Host/Content-Type (T12–T13), WS lifecycle on-connect/drop-slow/origin (T14), embedded web UI (T15), startup ordering + hardware fallback + graceful shutdown (T16), systemd + provisioning (audio-off + blacklist + state dir) + deploy (T3,T17), on-device checklist §12 (T18), ARMv6 cross toolchain (T2). Logging level/journald caps handled via `RUST_LOG=info` in the unit (T17); tighten journald `SystemMaxUse` during T17 if desired.
- **Deferred to on-device (by design):** exact GRB byte permutation (T7/T18-2), pixel-map orientation (T5/T18-3), sustained FPS (T18-4) — all have explicit adjustment points.
- **Watchdog/sd_notify** (spec §11) is intentionally omitted from Phase 1 tasks to keep scope tight; `Restart=on-failure` + `panic=abort` covers process-death restart. Add `WatchdogSec` + the `sd-notify` crate as a fast follow if desired.
```
