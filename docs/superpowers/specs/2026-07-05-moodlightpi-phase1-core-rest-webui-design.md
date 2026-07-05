# MoodLightPi — Phase 1: Core + REST + Web UI

**Status:** Approved design, hardened v2 (2026-07-05, after `plan-harden` thorough review)
**Scope:** Phase 1 of 4. Later phases (own specs): Phase 2 MQTT + Home Assistant, Phase 3 HomeKit, Phase 4 Matter.

> **Phase 4 note (decided 2026-07-05):** Matter will be implemented in
> JavaScript via [`matter.js`](https://github.com/project-chip/matter.js), not
> Rust `rs-matter` — matter.js is significantly more mature. It runs as a
> **separate out-of-process adapter** that talks to the Rust core over its REST
> API (or MQTT from Phase 2), preserving the "one engine, many adapters" model.
> Caveat: modern Node.js has no official ARMv6 support, so on the Pi Zero W this
> depends on unofficial Node builds; alternatively the bridge can run on any
> other always-on host pointing at the mood light. Final placement decided in
> the Phase 4 spec.

## 1. Goal

A Rust/Axum service running on a Raspberry Pi Zero W that drives a Pimoroni
Unicorn pHAT as an IoT mood light. Phase 1 delivers the native LED driver, a
render/effects engine, persistent state, a REST API, and an OctoCam-style web
UI. Bare-metal performance and a lean footprint on the ARMv6 core are primary
constraints.

## 2. Hardware & environment (verified)

- **Board:** Pimoroni **Unicorn pHAT** — 8×4 = **32 WS2812/NeoPixel RGB LEDs**
  on **GPIO18 (PWM0)**. GRB color order. No buttons on this board.
- **Host:** Raspberry Pi Zero W Rev 1.1 — single-core **ARMv6 (`armv6l`,
  ARM1176)**, 512 MB RAM.
- **OS:** Raspbian GNU/Linux 13 (trixie), kernel 6.18. Reachable at
  `192.168.1.230`, login `root`.
- **No existing Python:** `python3` is not installed and there are no `unicorn*`
  directories on the device. Effects are ported from the upstream
  [`pimoroni/unicorn-hat`](https://github.com/pimoroni/unicorn-hat) examples.
- **Rust target:** `arm-unknown-linux-gnueabihf` (ARMv6 hard-float). **Not** the
  armv7 target — an armv7 build emits instructions the ARM1176 lacks and crashes
  with SIGILL at runtime.
- **Driver:** WS2812 on GPIO18 is timing-critical and driven via PWM+DMA. Use
  [`rs_ws281x`](https://crates.io/crates/rs_ws281x) `0.5.1` (bindings to
  jgarff/`rpi_ws281x`). It **requires root** (`/dev/mem` + PWM/DMA); `/dev/gpiomem`
  is insufficient and only the SPI path avoids root — which the pHAT's fixed
  GPIO18 wiring does not allow. So the service **runs as root**.

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

### 3.1 Channels & concurrency contract

- **Command channel** — **bounded** `tokio::sync::mpsc` (capacity 32). Adapters →
  engine. Carries `Command { intent, source, }`. On full, the REST handler
  **load-sheds** with `503`/`429` and never blocks indefinitely. Rapid
  color/brightness updates coalesce **latest-wins**.
- **State broadcast** — `tokio::sync::watch<StateSnapshot>` where the snapshot
  carries a **monotonic `seq`** and the **`source`** that produced it. `watch`
  (latest-wins) is correct for UI/state; adapters ignore snapshots whose
  `source` is themselves (echo suppression) and use `seq` to detect updates.
  Rationale: this attribution is cheap now and prevents MQTT/HomeKit/Matter
  feedback loops later.
- **Frame stream** — separate `tokio::sync::broadcast<Frame>` for the WS preview
  (see §6). The render loop **never `await`s** an adapter/WS client.
- **Ownership contract (single-writer):** the **render engine task is the sole
  owner and mutator** of the `Display` and the authoritative state. Each tick it
  drains *all* pending commands, applies them, then renders one frame. In Solid
  mode it is event-driven (wakes on a command). There is **no shared,
  lock-guarded `Display`** — this is what prevents REST writes from racing the
  render loop.

### 3.2 Stack & build profile

- **Runtime:** Rust 2021 + Tokio **current-thread** runtime (single ARMv6 core),
  Axum for HTTP + WebSocket.
- **Key crates (all verified to cross-compile for ARMv6):** `axum`, `tokio`,
  `tower-http`, `serde` / `serde_json`, `rs_ws281x` `0.5.1` (feature-gated),
  `palette` (HSV↔RGB), `rust-embed` (web assets baked in), `tracing` /
  `tracing-subscriber`, `anyhow` / `thiserror`.
- **Atomics are a non-issue:** `arm-unknown-linux-gnueabihf` has
  `max_atomic_width = 64`, so `AtomicU64` (and thus tokio/axum) build without
  special `RUSTFLAGS`. The "missing AtomicU64" failures are on ARMv5 / armv7-musl,
  not this target.
- **Release profile:** `opt-level = "z"`, `lto = true`, `panic = "abort"`,
  `codegen-units = 1`. Target footprint ≈ 3 MB binary, ≈ 15 MB RAM.
  `panic = "abort"` is deliberate: a panicking render task aborts the process, so
  systemd restarts it into a clean, safe state (see §11).

### 3.3 Hardware abstraction (`Display` trait)

A `Display` trait abstracts the panel:

- Real impl backed by `rs_ws281x`, compiled only when the `hardware` cargo
  feature is on (the Pi build).
- Mock/simulator impl (default) that records frames in memory. **The mock must
  model the real pipeline** — GRB byte order, the gamma LUT, and the same global
  brightness path — so host tests exercise the real transform, not just linear
  RGB. This closes the "tested on Mac but wrong on hardware" gap.

CI runs host-only with the mock; the `hardware` feature keeps the C lib and
libclang/bindgen out of host builds.

## 4. Core components

1. **Driver (`hardware`)** — wraps `rs_ws281x`: 1 channel, GPIO18, 32 LEDs,
   WS2812 **GRB**. **DMA channel is configurable, default 10** (never 5 — causes
   filesystem corruption). Applies a **gamma LUT** then global brightness. Uses
   the authoritative pHAT pixel map (Appendix A). Applies the on/off gate (off =
   all-black while preserving state). On init failure, surfaces a typed error
   (see §11), pinning exact `rs_ws281x` + vendored C-lib versions.
2. **State manager** — single source of truth:
   `{ power: bool, mode: Solid|Effect, rgb: [u8;3], brightness: u8 (0–255), effect_name: String, speed: u8 (0–255) }`.
   - **Brightness is 0–255 end-to-end** (state, API, driver, UI slider maps
     linearly). `brightness = 0` is legal and means "fully dim" — distinct from
     `power`. **`power` is the master gate:** off ⇒ all-black regardless of
     brightness. Power-on with brightness 0 yields a dark panel by design.
   - **Persistence:** debounced *and* rate-limited writes (skip if serialized
     state unchanged; enforce a minimum write interval) to
     `/var/lib/moodlightpi/state.json`. **Atomic writes** — write `state.json.tmp`,
     `fsync`, `rename()` over the target (crash-safe against brown-outs).
     Flushed on graceful shutdown. On read, corrupt/missing ⇒ safe default.
3. **Render engine** — async task owning the `Display` (see §3.1). Solid mode →
   render once and idle. Effect mode → tick via `tokio::time::interval` with
   `MissedTickBehavior::Skip` at a target ~30 fps (adaptive/measured; treat as a
   target, not a guarantee). Effect frame-computation is **pure functions**
   `fn(state, tick) -> [Rgb; 32]` — unit-testable. Emits a heartbeat each tick
   for liveness (§11) and pushes frames to the broadcast for the WS preview.
   - **`speed` semantics (defined once, shared by all effects):** `speed` maps to
     a phase-increment per tick; `speed = 0` = slowest non-zero (never frozen, no
     divide-by-zero). Omitted `speed` on the API defaults to the last-used value.

### 4.1 Startup ordering

`load state (or default)` → `init hardware` (handle failure per §11) → `start
render loop seeded with restored state` → **then** `bind HTTP/WS`. The API is not
served until state is restored, so an early `GET /api/state` never returns
defaults that later "jump." **Brown-out guard on boot:** do not blindly restore a
high-draw state (see §7) — clamp to the safe brightness ceiling and/or ramp in.

## 5. REST API

All mutations funnel through the bounded command channel. JSON in/out.

| Method | Path | Body | Effect |
|---|---|---|---|
| GET | `/api/state` | — | Current state (incl. `seq`) |
| GET | `/api/effects` | — | Available effects (from the shared registry) |
| POST | `/api/power` | `{ "on": bool }` | On/off (state preserved) |
| POST | `/api/color` | `{ "r":0-255, "g":0-255, "b":0-255 }` | Set solid color (forces Solid mode; `rgb` retained even if a later effect ignores it) |
| POST | `/api/brightness` | `{ "value": 0-255 }` | Global brightness |
| POST | `/api/effect` | `{ "name": str, "speed"?: 0-255 }` | Switch to Effect mode |
| GET | `/healthz` | — | Liveness + hardware status (see §11) |

- Invalid input → `400` JSON error. Unknown effect name → `400`. **Both
  `GET /api/effects` and effect-name validation derive from one effect
  registry** so they can never diverge.
- **Security (the service runs as root; "LAN-trusted" is not sufficient alone):**
  - Reject requests whose `Origin`/`Host` header isn't the expected LAN host
    (defeats **DNS-rebinding** and cross-origin drive-by).
  - Require `Content-Type: application/json` (defeats simple-request CSRF).
  - **Validate the `Origin` header on the WebSocket upgrade.**
  - **Bind to the LAN interface**, not `0.0.0.0`.
  - Light rate-limiting / load-shedding on write endpoints (ties to the bounded
    channel) so a script can't flood the command path.
  - No user auth in Phase 1 — the above closes the browser-driven attack surface
    without a login flow.

## 6. Web UI

- Dark, single-column, mobile-friendly (OctoCam vibe), embedded via `rust-embed`.
- **Live 8×4 matrix preview** over `/ws`:
  - On connect, **send the current frame immediately** (Solid mode is idle, so
    the client would otherwise see nothing until the next change).
  - Effect mode streams frames at ~10–15 fps; Solid mode is event-driven
    (send-on-change), not a needless static stream.
  - Each connection is its own task consuming the frame `broadcast`; **slow
    clients get frames dropped (or are disconnected) — never apply backpressure
    to the render loop.** Cap concurrent WS clients (small N, given RAM).
- Controls: on/off toggle, color picker, brightness slider (0–255), effect
  selector, speed slider. Controls call REST; state arrives via `/ws`.

## 7. Effects, color & power safety

- **Effects (v1):** solid + brightness, plus ported animated effects —
  **rainbow**, **color-cycle**, **breathe/pulse** (exact set finalized against
  the upstream examples in the plan). Each is a pure `fn(state, tick) -> [Rgb;32]`.
- **Color pipeline order (fixed once):** effect/HSV compute (`palette`) → gamma
  LUT → global brightness scale → GRB output. Getting this order wrong yields
  banded/crushed output; nail it in the driver and mirror it in the mock.
- **Power/brown-out safety:** 32 WS2812 at full white draw ≈ 1.9 A at 5 V, which
  a marginal Pi Zero W PSU cannot source → under-voltage → SD corruption / reset
  → (naive) restore full-white → brown-out loop. Mitigations: a **firmware
  brightness/estimated-current ceiling**, a **safe default brightness** (not
  255), documented **2.5 A PSU** requirement, and the boot clamp/ramp in §4.1.

## 8. Build & deployment

- **Cross-compile from the Mac requires a custom toolchain — vanilla `cross`
  does not target Pi Zero/ARMv6 out of the box.** Provide a **custom `cross`
  `Dockerfile`** (or equivalent) containing an **ARMv6** C cross-toolchain
  (`gcc` emitting `-march=armv6 -mfpu=vfp -mfloat-abi=hard`), **`libclang`**
  (bindgen), and **`git`** (rs_ws281x vendors its C lib as a submodule and builds
  it via the `cc` crate at build time). Verify the output is genuinely ARMv6
  (`file` reports ARM EABI5 v6) and runs on the device — an armv7 build SIGILLs.
- **"One binary + one-time provisioning"** (not "one file"): a working system
  also needs the systemd unit, the state directory, and a firmware edit. Provide
  an **idempotent provisioning script** that:
  - creates `/var/lib/moodlightpi/` with correct permissions (app also creates it
    on startup and degrades gracefully — runs without persistence — if absent);
  - installs `moodlightpi.service` (root, `Restart=on-failure`, `WatchdogSec`,
    `TimeoutStopSec`, `WantedBy=multi-user.target`);
  - edits **`/boot/firmware/config.txt`** (correct path on trixie) to
    `dtparam=audio=off`, **and** writes `/etc/modprobe.d/snd-blacklist.conf`
    (`blacklist snd_bcm2835`) as a backstop, then prompts for the **required
    reboot**. Rationale: WS2812/PWM on GPIO18 conflicts with onboard PWM audio;
    `dtparam=audio=off` is the primary fix, the blacklist is the reliable
    backstop.
- A deploy script `scp`s the single binary and restarts the service.

## 9. Observability & SD-card protection

- Default a **conservative log level**; log `400`s at debug. Rely on **journald
  with size caps** (`SystemMaxUse=`) rather than unbounded log files — unbounded
  logging on flash is a known SD-card killer, compounded by `Restart=on-failure`
  loops.
- Combined with §4.2 atomic + rate-limited state writes, this bounds all
  routine flash writes.

## 10. Testing

- Effects, state manager, and REST handlers unit-tested on the host via the
  faithful mock `Display` (GRB + gamma + brightness modeled).
- **Golden-frame tests** for the pHAT pixel map (Appendix A) so a wrong map fails
  in CI, not just on the bench.
- The `hardware` feature keeps `rs_ws281x`/the C lib out of host builds and CI.
- On-device verification checklist (§12) is mandatory acceptance, scripted where
  possible.

## 11. Failure handling

- **Hardware init failure** (audio not disabled, DMA busy, permissions): keep the
  API + web UI **up** so the user can diagnose over the LAN; `/healthz` returns
  `{ alive: true, hardware: "error", detail: "…" }`. Only exit on truly
  unrecoverable errors, to avoid a systemd crash-loop that also kills the
  diagnostic API. DMA init has a **self-check + backoff** so a bad init doesn't
  tight-loop poking `/dev/mem` as root.
- **Render-loop liveness:** the engine bumps a heartbeat each tick; `/healthz`
  reports unhealthy if it goes stale. With `panic = "abort"`, a panicking render
  task aborts the process → systemd restarts into a safe state. Add
  `WatchdogSec` + `sd_notify` pings from the render loop.
- **Graceful shutdown (SIGTERM):** flush pending state to disk, then clear the
  panel to all-black (chosen behavior — avoids leaving the light stuck on if the
  service doesn't restart). `TimeoutStopSec` bounds the stop.

## 12. On-device verification checklist (mandatory before "done")

1. Custom cross image produces a runnable **ARMv6** binary (`file` = ARM EABI5
   v6; executes on the Pi).
2. `rs_ws281x` PWM/DMA works on this exact trixie / kernel 6.18 image.
3. DMA channel 10 is free; **no SD errors under sustained LED load**.
4. Sustained FPS the single core can actually hold (30 vs fall back to 20).
5. Brown-out: behavior at full white on the intended PSU; the brightness cap
   prevents reset.
6. Pixel-map orientation: per-corner pattern matches the WS preview.
7. GRB order: `{255,0,0}` shows **red**, not green.
8. Gamma: breathe/rainbow look smooth, not banded/crushed.
9. `dtparam=audio=off` alone sufficient, or is the `snd_bcm2835` blacklist
   needed?
10. Multi-client WS: several tabs → no render stutter.
11. `Origin`/`Host` check doesn't break the legit embedded web UI.

## 13. Non-goals (Phase 1)

MQTT, Home Assistant discovery, HomeKit, Matter (Phases 2–4); user
authentication (LAN + Origin/Host validation only); per-pixel / full-frame push
API; scrolling text; button input (no buttons on the pHAT).

## Appendix A — Unicorn pHAT pixel map (authoritative)

Source of truth: `library/UnicornHat/unicornhat.py` in `pimoroni/unicorn-hat`
(v2.2.3). The pHAT map is **column-major** — distinct from both the 8×8
serpentine `HAT` map and the row-major `PHAT_VERTICAL`. The C library
(`library_c/unicorn/unicorn.c`) contains **only** an 8×8 HAT map — no pHAT map —
so Python is authoritative.

Raw `PHAT` array, indexed `PHAT[x][y]` (x = 0..7 columns, y = 0..3 rows):

```
PHAT = [
  [24, 16,  8, 0],   # x=0
  [25, 17,  9, 1],   # x=1
  [26, 18, 10, 2],
  [27, 19, 11, 3],
  [28, 20, 12, 4],
  [29, 21, 13, 5],
  [30, 22, 14, 6],
  [31, 23, 15, 7],   # x=7
]
```

Caveat: the library's `get_index_from_xy(x, y)` applies a **vertical flip**
(`y = 3 - y`) and a rotation (switching to `PHAT_VERTICAL` at 90°/270°) *on top*
of this array. Our Rust driver will bake a single direct `(x,y) -> strip index`
table for our chosen logical origin, and **confirm orientation on-device**
(checklist #6) before locking the golden-frame tests (§10).
