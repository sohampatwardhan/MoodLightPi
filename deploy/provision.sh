#!/usr/bin/env bash
# Run ON the Pi as root (e.g. `ssh root@<pi> 'bash -s' < deploy/provision.sh`).
# Idempotent one-time setup for BOTH running and building MoodLightPi natively.
set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  echo "provision.sh must run as root" >&2
  exit 1
fi

CONFIG=/boot/firmware/config.txt
[ -f "$CONFIG" ] || CONFIG=/boot/config.txt   # older/DietPi path fallback
BLACKLIST=/etc/modprobe.d/snd-blacklist.conf
STATE_DIR=/var/lib/moodlightpi
SWAPFILE=/var/mlp-swap

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

echo "== ensure >= 1 GB persistent swap (async stack needs it on 512 MB) =="
if [ "$(free -m | awk '/Swap:/{print $2}')" -lt 1024 ]; then
  current_bytes=0
  [ -f "$SWAPFILE" ] && current_bytes=$(stat -c%s "$SWAPFILE" 2>/dev/null || echo 0)
  if [ "$current_bytes" -lt 1073741824 ]; then
    swapoff "$SWAPFILE" 2>/dev/null || true
    rm -f "$SWAPFILE"
    fallocate -l 2G "$SWAPFILE" 2>/dev/null || dd if=/dev/zero of="$SWAPFILE" bs=1M count=2048
    chmod 600 "$SWAPFILE"
    mkswap "$SWAPFILE" >/dev/null
  fi
  grep -qs "^$SWAPFILE[[:space:]]" /etc/fstab || echo "$SWAPFILE none swap sw 0 0" >> /etc/fstab
  swapon --show=NAME --noheadings | grep -qx "$SWAPFILE" || swapon "$SWAPFILE"
fi

echo "provision complete. If audio was just disabled, REBOOT before running the LED service."
