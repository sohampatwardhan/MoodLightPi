#!/usr/bin/env bash
# True cross-compile for Raspberry Pi Zero W: build on the host CPU, link
# against a sysroot copied from the ARMv6/Trixie Pi.
set -euo pipefail

cd "$(dirname "$0")/.."

PROJECT_DIR=$(pwd)
SYSROOT=${MLP_SYSROOT_DIR:-$PROJECT_DIR/target/pi-sysroot}
BUILDER_IMAGE=${MLP_CROSS_ARMV6_BUILDER_IMAGE:-moodlightpi-cross-armv6:trixie}
DIST_DIR=${MLP_CROSS_ARMV6_DIST_DIR:-$PROJECT_DIR/dist/pi-armv6-cross}
ARTIFACT=$DIST_DIR/moodlightpi
CACHE_DIR=${MLP_CROSS_ARMV6_CACHE_DIR:-$HOME/Library/Caches/moodlightpi-cross-armv6}
TARGET_DIR=${MLP_CROSS_ARMV6_TARGET_DIR:-$PROJECT_DIR/target/cross-armv6}
SYNC_SYSROOT=0

usage() {
  cat <<USAGE
Usage: deploy/build-cross-armv6.sh [--sync-sysroot]

Cross-compiles MoodLightPi on this machine for Raspberry Pi Zero W ARMv6/Trixie
and writes:

  $ARTIFACT

This is the fast path: Rust and C code compile on the Docker host CPU, while the
linker and bindgen use the Pi-derived sysroot at:

  $SYSROOT

Options:
  --sync-sysroot  Refresh the local sysroot from MLP_PI before building

Environment overrides:
  MLP_PI                         SSH target used with --sync-sysroot
  MLP_SYSROOT_DIR                Local Pi sysroot, default: $SYSROOT
  MLP_CROSS_ARMV6_BUILDER_IMAGE  Local builder tag, default: $BUILDER_IMAGE
  MLP_CROSS_ARMV6_DIST_DIR       Artifact directory, default: $DIST_DIR
  MLP_CROSS_ARMV6_CACHE_DIR      Cargo registry/git cache, default: $CACHE_DIR
  MLP_CROSS_ARMV6_TARGET_DIR     Cargo target dir, default: $TARGET_DIR
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sync-sysroot)
      SYNC_SYSROOT=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker is required for the ARMv6 cross build. Install Docker Desktop and try again." >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "Docker is installed but the daemon is not running. Start Docker Desktop and try again." >&2
  exit 1
fi

if [[ "$SYNC_SYSROOT" -eq 1 ]]; then
  ./deploy/sync-pi-sysroot.sh
fi

if [[ ! -d "$SYSROOT/usr/include" || ! -d "$SYSROOT/usr/lib/arm-linux-gnueabihf" ]]; then
  cat >&2 <<EOF
Missing Pi sysroot at $SYSROOT.

Run:
  ./deploy/build-cross-armv6.sh --sync-sysroot

or:
  ./deploy/sync-pi-sysroot.sh
EOF
  exit 1
fi

mkdir -p "$DIST_DIR" "$CACHE_DIR/cargo" "$TARGET_DIR"

echo "ensuring cross builder image $BUILDER_IMAGE exists ..."
docker buildx build \
  --load \
  -f deploy/Dockerfile.cross-armv6 \
  -t "$BUILDER_IMAGE" \
  .

echo "cross-compiling MoodLightPi for arm-unknown-linux-gnueabihf with Pi sysroot ..."
docker run --rm \
  -e CARGO_HOME=/cache/cargo \
  -e RUSTUP_HOME=/opt/rustup \
  -e CARGO_TARGET_DIR=/work/target/cross-armv6 \
  -e CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/local/bin/armv6-rpi-linux-gnueabihf-gcc \
  -e CC_arm_unknown_linux_gnueabihf=/usr/local/bin/armv6-rpi-linux-gnueabihf-gcc \
  -e CXX_arm_unknown_linux_gnueabihf=/usr/local/bin/armv6-rpi-linux-gnueabihf-g++ \
  -e AR_arm_unknown_linux_gnueabihf=arm-linux-gnueabihf-ar \
  -e CFLAGS_arm_unknown_linux_gnueabihf="--sysroot=/sysroot -march=armv6 -mfpu=vfp -mfloat-abi=hard" \
  -e CXXFLAGS_arm_unknown_linux_gnueabihf="--sysroot=/sysroot -march=armv6 -mfpu=vfp -mfloat-abi=hard" \
  -e BINDGEN_EXTRA_CLANG_ARGS="--target=arm-linux-gnueabihf --sysroot=/sysroot -I/sysroot/usr/include -I/sysroot/usr/include/arm-linux-gnueabihf -march=armv6 -mfpu=vfp -mfloat-abi=hard" \
  -e PKG_CONFIG_ALLOW_CROSS=1 \
  -e PKG_CONFIG_SYSROOT_DIR=/sysroot \
  -e PKG_CONFIG_LIBDIR=/sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig:/sysroot/usr/share/pkgconfig \
  -e RUSTFLAGS="-C target-cpu=arm1176jzf-s -C link-arg=-Wl,-rpath-link,/sysroot/lib/arm-linux-gnueabihf -C link-arg=-Wl,-rpath-link,/sysroot/usr/lib/arm-linux-gnueabihf" \
  -e HOST_GID="$(id -g)" \
  -e HOST_UID="$(id -u)" \
  -v "$PROJECT_DIR":/work \
  -v "$SYSROOT":/sysroot:ro \
  -v "$CACHE_DIR/cargo":/cache/cargo \
  -w /work \
  "$BUILDER_IMAGE" \
  bash -lc 'set -euo pipefail
    export PATH="/opt/cargo/bin:$PATH"
    rustc -vV
    arm-linux-gnueabihf-gcc -v 2>&1 | tail -n 1
    cargo build --release --locked --target arm-unknown-linux-gnueabihf --features hardware
    mkdir -p /work/dist/pi-armv6-cross
    cp "$CARGO_TARGET_DIR/arm-unknown-linux-gnueabihf/release/moodlightpi" /work/dist/pi-armv6-cross/moodlightpi
    chmod 0755 /work/dist/pi-armv6-cross/moodlightpi
    file /work/dist/pi-armv6-cross/moodlightpi
    chown -R "$HOST_UID:$HOST_GID" /work/dist/pi-armv6-cross /work/target/cross-armv6 /cache/cargo 2>/dev/null || true
  '

echo "built: $ARTIFACT"
