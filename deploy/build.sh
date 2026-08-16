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
MLP_SSH_OPTS=${MLP_SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=8}
read -r -a SSH_OPTS <<< "$MLP_SSH_OPTS"

usage() {
  cat <<USAGE
Usage: deploy/build.sh

Builds MoodLightPi on the Raspberry Pi Zero W itself, then leaves the binary at:

  $SRC/target/release/moodlightpi

This project targets a 32-bit Pi Zero W (ARMv6). Do not deploy a macOS build or
a generic Debian armhf cross-build; those are not valid Pi Zero W artifacts.

Environment overrides:
  MLP_PI        SSH target, default: $PI
  MLP_SRC       Remote source/build directory, default: $SRC
  MLP_SSH_OPTS  SSH options, default: $MLP_SSH_OPTS
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

echo "checking SSH access to $PI ..."
if ! ssh "${SSH_OPTS[@]}" "$PI" "true"; then
  cat >&2 <<EOF
Cannot reach $PI over SSH.

The Mood Light Pi is expected to be online for hardware builds because the
release binary is built natively on the Pi Zero W (ARMv6). Connect the Pi or set
MLP_PI=user@host before running this script. While it is offline, use
\`cargo test\` or \`cargo build --release\` only for the host/mock build.
EOF
  exit 1
fi

echo "checking Pi build prerequisites ..."
ssh "${SSH_OPTS[@]}" "$PI" '
  set -euo pipefail
  arch=$(uname -m)
  if [ "$arch" != armv6l ]; then
    echo "Expected Raspberry Pi Zero W armv6l, got: $arch" >&2
    exit 1
  fi
  export PATH="$HOME/.cargo/bin:$PATH"
  command -v cargo >/dev/null || {
    echo "cargo is missing; run deploy/provision.sh on the Pi first" >&2
    exit 1
  }
  command -v clang >/dev/null || {
    echo "clang is missing; run deploy/provision.sh on the Pi first" >&2
    exit 1
  }
  swap_mb=$(free -m | awk "/^Swap:/{print \$2}")
  if [ "${swap_mb:-0}" -lt 1024 ]; then
    echo "Swap is ${swap_mb:-0} MB; run deploy/provision.sh on the Pi to create persistent build swap" >&2
    exit 1
  fi
'

echo "shipping source to $PI:$SRC (preserving target/ for incremental builds) ..."
# Refresh source but keep the build cache (target/), so rebuilds are incremental.
ssh "${SSH_OPTS[@]}" "$PI" "mkdir -p '$SRC' && find '$SRC' -maxdepth 1 -mindepth 1 ! -name target -exec rm -rf {} +"
COPYFILE_DISABLE=1 tar --no-xattrs --no-mac-metadata -czf - --exclude=./target --exclude=./.git . | ssh "${SSH_OPTS[@]}" "$PI" "tar xzf - -C '$SRC'"

echo "building on the Pi (LTO off, codegen-units=16 to fit 512 MB) — this is slow (~2 h cold, minutes incremental) ..."
ssh "${SSH_OPTS[@]}" "$PI" "cd '$SRC' && CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 PATH=\$HOME/.cargo/bin:\$PATH cargo build --release --features hardware"
echo "built: $SRC/target/release/moodlightpi (on the Pi)"
