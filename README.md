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

## Build for the Pi (ARMv6) — trusted path: build *on* the Pi

The Pi Zero W is **ARMv6** (ARM1176). **Cross-compiling from a Debian-based host
does not work**: Debian's armhf port is ARMv7-baseline (libc/crt/libgcc), so such
binaries `SIGILL` on the Pi. Raspberry Pi OS / DietPi userland *is* ARMv6, so the
reliable path is to **build on the Pi itself** (the `deploy/` scripts do this over
SSH — no `scp` needed, since minimal images often lack it).

> First build is slow — ~2 h cold on a single-core 512 MB Pi Zero. Incremental
> rebuilds are minutes (LTO is disabled and the build tree is kept on the Pi).

### Experimental Docker ARMv6 build

There are also experimental build-here/deploy-there paths.

For fastest builds, use the true cross-compiler path. It compiles Rust/C code on
this machine's CPU, but links and runs bindgen against a sysroot copied from the
actual Pi:

```bash
./deploy/build-cross-armv6.sh --sync-sysroot
```

Later builds can omit `--sync-sysroot` unless the Pi OS packages change:

```bash
./deploy/build-cross-armv6.sh
```

The artifact is written to `dist/pi-armv6-cross/moodlightpi`.

To build locally and deploy that cross-built artifact to the Pi:

```bash
./deploy/deploy-cross-armv6.sh
```

Use `./deploy/deploy-cross-armv6.sh --sync-sysroot` after Pi OS package
upgrades.

There is also a Docker/QEMU path. It is more conservative than true
cross-compilation because it builds inside an emulated ARMv6 container, but it is
slower:

```bash
./deploy/build-docker-armv6.sh
```

It creates/reuses a local ARMv6 builder image, runs the build inside a
`linux/arm/v6` Raspberry Pi-compatible container, and writes
`dist/pi-armv6/moodlightpi`. This is deliberately separate from
`deploy/deploy.sh` until the produced binary has been verified on the real Pi.
It may be faster than the first native Pi build on a Mac, but it still needs
QEMU emulation, Docker Desktop, Rust, clang, and the `rs_ws281x` C build stack
inside the container.

The current Pi is DietPi / Debian Trixie. The default Docker builder uses the
available `balenalib/raspberry-pi-debian:bookworm-build` ARMv6 image because the
equivalent Balena Trixie ARMv6 tag is not currently published. That is usually a
conservative compatibility direction (older glibc-built binary on newer
Trixie), but keep the native Pi build as the trusted deploy path until the
Docker artifact has been tested on-device. If a Trixie ARMv6 base becomes
available, override it with:

```bash
MLP_ARMV6_DOCKER_IMAGE=<armv6-trixie-image> ./deploy/build-docker-armv6.sh
```

Do **not** replace this with a generic Debian `armhf` cross-build; that target is
ARMv7-baseline and is not safe for the original Pi Zero W.

## Deploy

```bash
# 1. One-time: provision the Pi (audio off + state dir + build toolchain + swap):
ssh root@<pi> 'bash -s' < deploy/provision.sh
#    Reboot if audio was just disabled (frees GPIO18):
ssh root@<pi> reboot

# 2. Build (on the Pi) + install & enable the service:
./deploy/deploy.sh
```

`deploy/build.sh` ships the source via `tar`-over-SSH and runs `cargo build
--release --features hardware` on the Pi; `deploy/deploy.sh` then installs the
binary + unit and enables `moodlightpi.service` (root, restart-on-failure).
Override the target with `MLP_PI=user@host`.

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
