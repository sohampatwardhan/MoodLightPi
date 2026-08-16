# MoodLightPi Agent Notes

MoodLightPi targets a Raspberry Pi Zero W running 32-bit Raspberry Pi OS or
DietPi. That board is ARMv6 (`armv6l`), not the 64-bit ARM target used by
OctoCam's Pi Zero 2 W workflow.

## Build Rules

- `cargo test` and `cargo build --release` on the development Mac are valid
  host/mock builds only. They do not include the LED hardware backend and must
  not be deployed to the Pi.
- Do not try to deploy a generic Debian `armhf` cross-build. Debian `armhf`
  assumes an ARMv7 baseline and can produce binaries that fail with `SIGILL` on
  the Pi Zero W.
- Do not use OctoCam's Docker/aarch64 build pattern here. OctoCam targets a Pi
  Zero 2 W on 64-bit OS; MoodLightPi targets the original Pi Zero W on ARMv6.
- The deployable hardware binary is built natively on the Pi by
  `deploy/build.sh`, then installed by `deploy/deploy.sh`.
- `deploy/build-docker-armv6.sh` is an experimental Docker/QEMU path. It must
  run as `linux/arm/v6` and use an ARMv6 Raspberry Pi-compatible userland. Do
  not substitute a generic Debian `armhf` cross-build.
- `deploy/build-cross-armv6.sh` is the fast true cross-compile path. It uses a
  sysroot copied from the actual Pi via `deploy/sync-pi-sysroot.sh`, the Rust
  `arm-unknown-linux-gnueabihf` target, and wrapper linkers that force
  `-march=armv6 -mfpu=vfp -mfloat-abi=hard`.
- `deploy/deploy-cross-armv6.sh` is the build-here/deploy-there workflow. It
  installs the cross-built artifact only after `ldd` succeeds on the Pi and
  rolls back if `/healthz` does not report `backend=hardware`.
- The live Pi is DietPi / Debian Trixie. The Docker ARMv6 script currently
  defaults to Balena's Bookworm ARMv6 builder because a Balena Trixie ARMv6 tag
  was not available when checked. This is a conservative older-glibc-on-newer-OS
  direction, but validate Docker artifacts on the real Pi before trusting them.

## Offline Pi Behavior

The Mood Light Pi is sometimes disconnected. If SSH to `MLP_PI` fails, stop
after verifying the host/mock build and explain that hardware build/deploy needs
the Pi online.

Useful host-only checks:

```bash
cargo test
cargo build --release
bash -n deploy/*.sh
```

Avoid `cargo build --features hardware` on macOS as a deployment strategy. It
requires the `rs_ws281x` C/bindgen stack and still would not produce a Pi Zero W
artifact.

## Provisioning Reminder

Run provisioning on the Pi before the first hardware build:

```bash
ssh root@<pi> 'bash -s' < deploy/provision.sh
```

Provisioning disables onboard audio for GPIO18/PWM, installs the Rust/C build
toolchain, creates persistent swap for native builds, and may require a reboot.
