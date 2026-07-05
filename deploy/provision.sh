#!/usr/bin/env bash
# Run ON the Pi as root (e.g. `ssh root@<pi> 'bash -s' < deploy/provision.sh`).
# Idempotent one-time setup for BOTH running and building MoodLightPi natively.
set -euo pipefail

CONFIG=/boot/firmware/config.txt
[ -f "$CONFIG" ] || CONFIG=/boot/config.txt   # older/DietPi path fallback
BLACKLIST=/etc/modprobe.d/snd-blacklist.conf
STATE_DIR=/var/lib/moodlightpi

echo "== runtime: free GPIO18/PWM from onboard audio =="
if ! grep -q '^dtparam=audio=off' "$CONFIG" 2>/dev/null; then
  sed -i 's/^dtparam=audio=on/dtparam=audio=off/' "$CONFIG" 2>/dev/null || true
  grep -q '^dtparam=audio=off' "$CONFIG" 2>/dev/null || echo 'dtparam=audio=off' >> "$CONFIG"
fi
[ -f "$BLACKLIST" ] || echo 'blacklist snd_bcm2835' > "$BLACKLIST"
mkdir -p "$STATE_DIR"

echo "== build toolchain (native on-Pi builds; ARMv6 correctness) =="
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential libclang-dev clang git curl pkg-config ca-certificates
if ! [ -x "$HOME/.cargo/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
fi

echo "== ensure >= 1 GB swap (async stack needs it on 512 MB) =="
if [ "$(free -m | awk '/Swap:/{print $2}')" -lt 1024 ]; then
  fallocate -l 2G /var/mlp-swap 2>/dev/null || dd if=/dev/zero of=/var/mlp-swap bs=1M count=2048
  chmod 600 /var/mlp-swap && mkswap /var/mlp-swap >/dev/null && swapon /var/mlp-swap
fi

echo "provision complete. If audio was just disabled, REBOOT before running the LED service."
