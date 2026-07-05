#!/usr/bin/env bash
# Build the ARMv6 (Pi Zero W) release binary using the arm64-native builder
# image — no `cross`, no QEMU emulation. Works on Apple Silicon and amd64.
set -euo pipefail
cd "$(dirname "$0")/.."
IMAGE=moodlightpi-builder
TARGET=arm-unknown-linux-gnueabihf

docker build -f docker/Dockerfile.build -t "$IMAGE" docker/
docker run --rm -v "$PWD":/work "$IMAGE" \
  cargo build --release --features hardware --target "$TARGET"
file "target/$TARGET/release/moodlightpi"
