#!/usr/bin/env bash
# Experimental Docker/QEMU ARMv6 build for Raspberry Pi Zero W.
#
# This intentionally does NOT use Debian's generic armhf cross toolchain. The
# container is run as linux/arm/v6 so Rust, libc, libgcc, and rs_ws281x are built
# against an ARMv6-compatible Raspberry Pi userland.
set -euo pipefail

cd "$(dirname "$0")/.."

PROJECT_DIR=$(pwd)
DOCKER_IMAGE=${MLP_ARMV6_DOCKER_IMAGE:-balenalib/raspberry-pi-debian:bookworm-build}
BUILDER_IMAGE=${MLP_ARMV6_BUILDER_IMAGE:-moodlightpi-armv6-builder:bookworm}
DOCKER_PLATFORM=${MLP_ARMV6_DOCKER_PLATFORM:-linux/arm/v6}
DIST_DIR=${MLP_ARMV6_DIST_DIR:-$PROJECT_DIR/dist/pi-armv6}
ARTIFACT=$DIST_DIR/moodlightpi
CACHE_DIR=${MLP_ARMV6_CACHE_DIR:-$HOME/Library/Caches/moodlightpi-armv6}
TARGET_DIR=${MLP_ARMV6_TARGET_DIR:-$PROJECT_DIR/target/docker-armv6}

usage() {
  cat <<USAGE
Usage: deploy/build-docker-armv6.sh

Builds MoodLightPi in an ARMv6 Docker container and writes:

  $ARTIFACT

This is experimental. It runs the build under Docker/QEMU with an ARMv6
Raspberry Pi userland; it does not use Debian's generic ARMv7-baseline armhf
cross toolchain.

Environment overrides:
  MLP_ARMV6_DOCKER_IMAGE     Builder base image, default: $DOCKER_IMAGE
  MLP_ARMV6_BUILDER_IMAGE    Local builder tag, default: $BUILDER_IMAGE
  MLP_ARMV6_DOCKER_PLATFORM  Docker platform, default: $DOCKER_PLATFORM
  MLP_ARMV6_DIST_DIR         Artifact directory, default: $DIST_DIR
  MLP_ARMV6_CACHE_DIR        Cargo registry/git cache, default: $CACHE_DIR
  MLP_ARMV6_TARGET_DIR       Cargo target dir, default: $TARGET_DIR
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker is required for the ARMv6 Docker build. Install Docker Desktop and try again." >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "Docker is installed but the daemon is not running. Start Docker Desktop and try again." >&2
  exit 1
fi

mkdir -p "$DIST_DIR" "$CACHE_DIR/cargo" "$TARGET_DIR"

echo "ensuring ARMv6 builder image $BUILDER_IMAGE exists ..."
docker buildx build \
  --load \
  --platform "$DOCKER_PLATFORM" \
  --build-arg "BASE_IMAGE=$DOCKER_IMAGE" \
  -f deploy/Dockerfile.armv6-builder \
  -t "$BUILDER_IMAGE" \
  .

echo "building MoodLightPi for Raspberry Pi Zero W ($DOCKER_PLATFORM) with $BUILDER_IMAGE ..."
docker run --rm \
  --platform "$DOCKER_PLATFORM" \
  -e CARGO_HOME=/cache/cargo \
  -e RUSTUP_HOME=/opt/rustup \
  -e CARGO_TARGET_DIR=/work/target/docker-armv6 \
  -e HOST_GID="$(id -g)" \
  -e HOST_UID="$(id -u)" \
  -v "$PROJECT_DIR":/work \
  -v "$CACHE_DIR/cargo":/cache/cargo \
  -w /work \
  "$BUILDER_IMAGE" \
  bash -lc 'set -euo pipefail
    export PATH="/opt/cargo/bin:$PATH"

    echo "container arch: $(uname -m)"
    if [ "$(uname -m)" != armv6l ]; then
      echo "Expected an ARMv6 container (armv6l), got: $(uname -m)" >&2
      exit 1
    fi

    rustc -vV
    cargo build --release --locked --features hardware

    mkdir -p /work/dist/pi-armv6
    cp "$CARGO_TARGET_DIR/release/moodlightpi" /work/dist/pi-armv6/moodlightpi
    chmod 0755 /work/dist/pi-armv6/moodlightpi
    file /work/dist/pi-armv6/moodlightpi

    chown -R "$HOST_UID:$HOST_GID" \
      /work/dist/pi-armv6 /work/target/docker-armv6 /cache/cargo \
      2>/dev/null || true
  '

echo "built: $ARTIFACT"
