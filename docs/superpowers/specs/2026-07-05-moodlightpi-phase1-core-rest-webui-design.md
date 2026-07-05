# MoodLightPi — Phase 1: Core + REST + Web UI

**Status:** Approved design (2026-07-05)
**Scope:** Phase 1 of 4. Later phases (own specs): Phase 2 MQTT + Home Assistant, Phase 3 HomeKit, Phase 4 Matter.

## 1. Goal

A Rust/Axum service running on a Raspberry Pi Zero W that drives a Pimoroni
Unicorn pHAT as an IoT mood light. Phase 1 delivers the native LED driver, a
render/effects engine, persistent state, a REST API, and an OctoCam-style web
UI. Bare-metal performance and a lean footprint on the ARMv6 core are primary
constraints.

## 2. Hardware & environment (verified)

- **Board:** Pimoroni **Unicorn pHAT** — 8×4 = **32 WS2812/NeoPixel RGB LEDs**
  on **GPIO18 (PWM0)**. GRB color order. No buttons on this board.
- **Host:** Raspberry Pi Zero W Rev 1.1 — single-core **ARMv6 (`armv6l`)**,
  512 MB RAM.
- **OS:** Raspbian GNU/Linux 13 (trixie), kernel 6.18. Reachable at
  `192.168.1.230`, login `root`.
- **No existing Python:** `python3` is not installed and there are no `unicorn*`
  directories on the device. "Leave the Python scripts untouched" is therefore a
  non-issue in practice — effects are ported from the upstream
  [`pimoroni/unicorn-hat`](https://github.com/pimoroni/unicorn-hat) examples, not
  from anything on the Pi.
- **Rust target:** `arm-unknown-linux-gnueabihf` (ARMv6 hard-float). **Not**
  the armv7 target.
- **Driver approach:** WS2812 on GPIO18 is timing-critical and driven via
  PWM+DMA. Use the [`rs_ws281x`](https://crates.io/crates/rs_ws281x) crate
  (Rust bindings to jgarff/`rpi_ws281x`). Requires `/dev/mem` + PWM/DMA access,
  so the service **runs as root**.

## 3. Architecture

One **core** owns the hardware and the state. Every protocol is a thin
**adapter** that talks to the core through a single internal interface. Adapters
never touch the LEDs directly. This isolation is what makes the later phases
pluggable.

```
                 ┌─────────────────────────────────────┐
   REST ───┐     │              CORE                     │
   Web/WS ─┤ cmd │  ┌────────────┐   ┌───────────────┐  │
  (MQTT)───┼───▶ │  │ State mgr  │──▶│ Render engine │──┼──▶ Display ─▶ pHAT
 (HomeKit)─┤     │  │ (+persist) │   │ (effects,FPS) │  │   (rs_ws281x, GPIO18)
 (Matter)──┘     │  └────────────┘   └───────────────┘  │
       ▲         └───────────────┬─────────────────────┘
       └──────── state broadcast ┘
```

- **Command channel** (`tokio::sync::mpsc`): adapters → engine. Carries
  intents (set power, color, brightness, effect).
- **State broadcast** (`tokio::sync::watch`): engine → adapters. Adapters
  observe the authoritative current state.
- Parenthesized adapters (MQTT/HomeKit/Matter) are future phases; they attach at
  the same two channels without modifying the core.

### 3.1 Stack

- **Runtime:** Rust 2021 + Tokio (small/lean runtime — single ARMv6 core),
  Axum for HTTP + WebSocket.
- **Key crates:** `axum`, `tokio`, `tower-http` (static/compression),
  `serde` / `serde_json`, `rs_ws281x` (LED driver, feature-gated), `palette`
  (HSV↔RGB for effects), `rust-embed` (bake web assets into the binary),
  `tracing` / `tracing-subscriber`, `anyhow` / `thiserror`.

### 3.2 Hardware abstraction (`Display` trait)

A `Display` trait abstracts the panel:

- Real impl backed by `rs_ws281x`, compiled only when the `hardware` cargo
  feature is on (the Pi build).
- Mock/simulator impl (default) that records frames in memory.

This lets effects, state, and REST handlers **compile, run, and be unit-tested
on the Mac** with no Pi and no C library. CI runs host-only with the mock.

## 4. Core components

1. **Driver (`hardware`)** — wraps `rs_ws281x`: 1 channel, GPIO18, 32 LEDs,
   WS2812 GRB. Owns the exact **8×4 pixel map** ported from the Pimoroni library
   (the real mapping is lifted from the lib source during planning — do not
   assume naive row-major or serpentine). Applies global brightness and the
   on/off state (off = render all-black while preserving state).
2. **State manager** — single source of truth:
   `{ power: bool, mode: Solid|Effect, rgb: [u8;3], brightness: u8, effect_name: String, speed: u8 }`.
   Debounced persistence (≈500 ms after last change) to
   `/var/lib/moodlightpi/state.json` to limit SD-card wear. Restores on boot;
   falls back to a sane default if the file is missing/corrupt.
3. **Render engine** — async task owning the `Display`. Solid mode → render once
   and idle. Effect mode → tick at a modest fixed FPS (~30) computing frames.
   Effect frame-computation is **pure functions** (state + tick → 32-pixel
   frame), so it is trivially unit-testable. The engine also emits rendered
   frames to the WebSocket preview.

## 5. REST API

All mutations funnel through the command channel. JSON in/out.

| Method | Path | Body | Effect |
|---|---|---|---|
| GET | `/api/state` | — | Current state |
| GET | `/api/effects` | — | List of available effects |
| POST | `/api/power` | `{ "on": bool }` | Turn LEDs on/off (state preserved) |
| POST | `/api/color` | `{ "r":0-255, "g":0-255, "b":0-255 }` | Set solid color (switches to Solid mode) |
| POST | `/api/brightness` | `{ "value": 0-255 }` | Set global brightness |
| POST | `/api/effect` | `{ "name": str, "speed"?: 0-255 }` | Switch to Effect mode |
| GET | `/healthz` | — | Liveness |

Invalid input → `400` with a JSON error body. Unknown effect name → `400`.

## 6. Web UI

- Dark, single-column, mobile-friendly (OctoCam vibe). Embedded in the binary
  via `rust-embed` — deploy is a single file.
- **Live 8×4 matrix preview** fed by a `/ws` WebSocket that streams the actual
  rendered frames at ~10–15 fps (32 pixels ⇒ trivial bandwidth), so the preview
  is truthful even during effects.
- Controls: on/off toggle, color picker, brightness slider, effect selector,
  speed slider. All controls call the REST API; state updates arrive via `/ws`.

## 7. Effects (v1)

Ported from the upstream `pimoroni/unicorn-hat` examples (exact set finalized
against the repo during planning):

- **Solid** (color + brightness) — the baseline.
- **Rainbow** — moving hue across the panel.
- **Color-cycle** — whole-panel hue sweep.
- **Breathe / pulse** — brightness oscillation on the current color.

Each effect is a pure `fn(state, tick) -> [Rgb; 32]`.

## 8. Deployment

- **Cross-compile** from the Mac with `cross`:
  `cross build --release --features hardware --target arm-unknown-linux-gnueabihf`.
  (`cross` provides the ARMv6 toolchain + C compiler for `rs_ws281x`.)
- A deploy script `scp`s the single binary to the Pi and installs/restarts a
  `systemd` unit `moodlightpi.service` (runs as root, `Restart=on-failure`,
  `WantedBy=multi-user.target`).
- ⚠️ **Critical gotcha:** WS2812 on GPIO18 uses PWM0, which conflicts with the
  Pi's onboard analog audio. Setup must add `dtparam=audio=off` to
  `/boot/firmware/config.txt` and reboot, or the LEDs will not drive reliably.

## 9. Testing

- Effects, state manager, and REST handlers unit-tested on the host via the mock
  `Display` (no Pi, no C library).
- The `hardware` feature keeps `rs_ws281x`/the C lib out of host builds and CI.
- Manual on-device smoke test against `192.168.1.230` after each deploy.

## 10. Non-goals (Phase 1)

MQTT, Home Assistant discovery, HomeKit, Matter (Phases 2–4); authentication
(LAN-trusted); per-pixel / full-frame push API; scrolling text; button input
(no buttons on the pHAT).
