#!/usr/bin/env bash
# Build the ARMv6 release binary NATIVELY ON THE PI.
#
# Why not cross-compile? The Pi Zero W is ARMv6 (ARM1176). Debian's armhf port
# is ARMv7-baseline (its libc/crt/libgcc), so binaries cross-built in a Debian
# container SIGILL on the Pi. Raspberry Pi OS / DietPi userland is ARMv6, so
# building on the Pi itself is the reliable path.
#
# One-time prereqs on the Pi (see README "Deploy"): rustup, build-essential,
# libclang-dev, clang, git, and >=1 GB swap.
set -euo pipefail
cd "$(dirname "$0")/.."
PI=${MLP_PI:-root@192.168.1.230}
SRC=${MLP_SRC:-/root/moodlightpi}

echo "shipping source to $PI:$SRC ..."
tar czf - --exclude=./target --exclude=./.git . \
  | ssh "$PI" "rm -rf '$SRC' && mkdir -p '$SRC' && tar xzf - -C '$SRC'"

echo "building on the Pi (LTO off, codegen-units=16 to fit 512 MB) — this is slow (~2 h cold, minutes incremental) ..."
ssh "$PI" "cd '$SRC' && CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 PATH=\$HOME/.cargo/bin:\$PATH cargo build --release --features hardware"
echo "built: $SRC/target/release/moodlightpi (on the Pi)"
