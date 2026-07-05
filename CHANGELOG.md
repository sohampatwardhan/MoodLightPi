# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-05

Phase 1 — core mood-light service (REST API + web UI).

### Added
- Rust/Axum service driving the Unicorn pHAT (8×4 WS2812 on GPIO18) via
  `rs_ws281x`, behind a `Display` trait with a real backend (`hardware` feature)
  and a faithful in-memory mock.
- Single-writer **render engine**: owns the display, drains a bounded command
  channel, broadcasts state snapshots (`watch`) and frames (`broadcast`), renders
  effects at ~30 fps.
- Effects: **solid**, **rainbow**, **colorcycle**, **breathe** (pure functions),
  behind a single effect registry that backs both `/api/effects` and validation.
- **REST API**: `/api/state`, `/api/effects`, `/api/power`, `/api/color`,
  `/api/brightness`, `/api/effect`, `/healthz`.
- **Embedded web UI** (rust-embed): dark, mobile-friendly control page with a live
  8×4 canvas preview over a `/ws` WebSocket (on-connect frame, slow clients
  dropped, never back-pressures the engine).
- **Persistence**: atomic (temp + fsync + rename), rate-limited writes to
  `/var/lib/moodlightpi/state.json`; restores last state on boot, safe default on
  missing/corrupt.
- **Safety**: brown-out brightness cap + safe default; power gate; graceful
  shutdown (SIGINT/SIGTERM) that clears the panel and flushes state.
- **Security**: `Host`/`Origin` validation on every route and the WS upgrade
  (DNS-rebinding / CSRF guard) for the LAN-trusted, root-run service.
- **Build/deploy**: native on-Pi build over SSH (`deploy/build.sh`,
  ARMv6-correct), idempotent Pi provisioning + build-toolchain setup
  (`deploy/provision.sh`), and one-command deploy with a systemd unit
  (`deploy/deploy.sh`).

### Notes
- On-device acceptance (GRB byte order, pHAT pixel-map orientation, sustained FPS,
  brown-out headroom) is finalized against the physical panel; the byte
  permutation in `src/hardware.rs` and `PHAT_MAP` in `src/geometry.rs` are the
  adjustment points if the panel disagrees.
- Cross-compiling for the Pi Zero's ARMv6 from a Debian-based host does not work
  (Debian armhf is ARMv7-baseline → `SIGILL` on the ARM1176); MoodLightPi builds
  **natively on the Pi** (Raspberry Pi OS / DietPi userland is ARMv6). Verified
  on-device: `backend: hardware`, red-shows-red (GRB), persistence across reboot,
  clean graceful shutdown.

### Fixed (during development review)
- Engine now renders restored state at startup (not only after the first command).
- Graceful shutdown no longer hangs when a browser/WebSocket is connected (an open
  upgraded connection previously kept shutdown pending, leaving the panel lit).
- `rs_ws281x::Controller` is `!Send`; resolved with a justified `unsafe impl Send`
  for the single-writer-owned display.
