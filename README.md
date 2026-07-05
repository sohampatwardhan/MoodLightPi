# MoodLightPi

IoT-enabled mood light built on the Pimoroni Raspberry Pi Zero W Mood Light Kit.
A lean **Rust + Axum** service drives a **Unicorn pHAT** (8×4 = 32 WS2812 LEDs on
GPIO18) and exposes it over a REST API and an embedded web UI.

## Hardware

- **Board:** Pimoroni Unicorn pHAT — 32 WS2812/NeoPixel RGB LEDs on GPIO18 (PWM0).
- **Host:** Raspberry Pi Zero W (single-core ARMv6, 512 MB), Raspberry Pi OS.
- **Driver:** [`rs_ws281x`](https://crates.io/crates/rs_ws281x) (PWM/DMA). Requires
  running as **root**, and onboard PWM audio must be disabled (see Deploy).

## Features

- Solid colour + brightness, on/off, and animated effects: **rainbow**,
  **colorcycle**, **breathe** (ported from the Pimoroni examples).
- **REST API** (JSON) and an **OctoCam-style web UI** with a live 8×4 preview
  streamed over WebSocket.
- **Persistent state** — restores the last colour/effect/brightness across reboots
  (atomic, SD-card-friendly writes).
- **Brown-out safety** — a firmware brightness cap keeps worst-case LED current
  within a Pi Zero W's budget.
- **Graceful shutdown** — clears the panel and flushes state on SIGINT/SIGTERM.

## Architecture

One **core** owns the hardware and state; every protocol is a thin **adapter**
talking to it through a single internal interface:

```
   REST ─┐  cmd   ┌───────────── CORE ─────────────┐
   Web/WS┤ ─────▶ │ State mgr ──▶ Render engine ────┼─▶ Display ─▶ pHAT
         │        │  (+persist)   (effects, ~30fps) │   (rs_ws281x / mock)
         └────────┴──── state broadcast ◀───────────┘
```

The render engine is the **single writer** of the display; adapters reach it only
via a bounded command channel (in) and `watch` state + `broadcast` frame channels
(out). A `Display` trait has a real `rs_ws281x` backend (`hardware` feature) and a
faithful in-memory mock, so all logic is testable on a host with no Pi.

Design docs and the implementation plan live under
[`docs/superpowers/`](docs/superpowers/).

## Build & test (host)

```bash
cargo test          # 34 tests, no hardware needed (uses the mock display)
cargo run           # runs with the mock display; override the bind for local use:
MLP_BIND=127.0.0.1:8080 MLP_HOST=127.0.0.1 cargo run
```

## Build for the Pi (ARMv6)

Cross-compiling for the Pi Zero's ARM1176 is done with a self-contained
arm64/amd64-native Docker builder (no `cross`, no emulation quirks):

```bash
./deploy/build.sh   # -> target/arm-unknown-linux-gnueabihf/release/moodlightpi
```

Requires Docker. The builder installs rustup + the `arm-unknown-linux-gnueabihf`
target + the armhf gcc cross-toolchain and forces `-march=armv6` for the vendored
`rs_ws281x` C library.

## Deploy

```bash
# 1. Provision the Pi ONCE (disables PWM audio so GPIO18 is free) then reboot:
scp deploy/provision.sh root@<pi>:/tmp/ && ssh root@<pi> 'bash /tmp/provision.sh && reboot'

# 2. Build + install the systemd service:
./deploy/deploy.sh
```

`deploy/deploy.sh` builds the ARMv6 binary, copies it + the unit to the Pi, and
enables `moodlightpi.service` (runs as root, restarts on failure).

## Configuration (environment)

| Var | Default | Meaning |
|---|---|---|
| `MLP_BIND` | `192.168.1.230:80` | Socket address to bind |
| `MLP_HOST` | `192.168.1.230` | Expected `Host`/`Origin` for request validation |
| `RUST_LOG` | `info` | Log filter |

Set these in `deploy/moodlightpi.service` for your Pi's address.

## REST API

| Method | Path | Body | Effect |
|---|---|---|---|
| GET | `/api/state` | — | Current state + `seq` |
| GET | `/api/effects` | — | Available effect names |
| POST | `/api/power` | `{"on":bool}` | On/off (state preserved) |
| POST | `/api/color` | `{"r":0-255,"g":0-255,"b":0-255}` | Solid colour |
| POST | `/api/brightness` | `{"value":0-255}` | Global brightness (capped) |
| POST | `/api/effect` | `{"name":str,"speed"?:0-255}` | Switch to an effect |
| GET | `/healthz` | — | `{alive, backend}` (`backend`: `hardware`\|`mock`) |
| GET | `/ws` | — | WebSocket: live 8×4 frame stream |

Example:

```bash
curl -X POST -H 'content-type: application/json' \
  -d '{"r":255,"g":40,"b":0}' http://<pi>/api/color
```

## Security

The service runs as **root** and is **LAN-trusted** — there is no login. It does
validate the `Host`/`Origin` headers on every route (and on the WebSocket
upgrade) to block DNS-rebinding / cross-site drive-by requests. Only expose it on
a trusted network.

## Roadmap

Phase 1 (this release) is the core service. Planned phases, each an adapter on the
same core interface:

- **Phase 2** — MQTT + Home Assistant (MQTT Discovery).
- **Phase 3** — HomeKit (`hap-rs`).
- **Phase 4** — Matter (via `matter.js` or `connectedhomeip`; decided at that
  phase — see the spec's Phase 4 note).

## License

Personal project. The Pimoroni `unicorn-hat` effect logic is referenced from its
upstream examples.
