#!/usr/bin/env bash
# Run ON the Pi as root. Idempotent one-time provisioning.
set -euo pipefail
CONFIG=/boot/firmware/config.txt
BLACKLIST=/etc/modprobe.d/snd-blacklist.conf
STATE_DIR=/var/lib/moodlightpi

# 1. Free GPIO18/PWM from onboard audio (primary fix).
if ! grep -q '^dtparam=audio=off' "$CONFIG"; then
  sed -i 's/^dtparam=audio=on/dtparam=audio=off/' "$CONFIG" || true
  grep -q '^dtparam=audio=off' "$CONFIG" || echo 'dtparam=audio=off' >> "$CONFIG"
  echo "config.txt: audio disabled"
fi
# 2. Backstop: blacklist the module.
if [ ! -f "$BLACKLIST" ]; then
  echo 'blacklist snd_bcm2835' > "$BLACKLIST"
  echo "wrote $BLACKLIST"
fi
# 3. State directory.
mkdir -p "$STATE_DIR"
echo "provision complete — REBOOT REQUIRED for audio change to take effect"
