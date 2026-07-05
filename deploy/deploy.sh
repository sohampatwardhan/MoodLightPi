#!/usr/bin/env bash
# Build the ARMv6 binary and deploy it to the Pi as a systemd service.
set -euo pipefail
cd "$(dirname "$0")/.."
PI=root@192.168.1.230
TARGET=arm-unknown-linux-gnueabihf
BIN=target/$TARGET/release/moodlightpi

./deploy/build.sh
file "$BIN" | grep -q 'ARM' || { echo "not an ARM binary"; exit 1; }

ssh "$PI" 'systemctl stop moodlightpi 2>/dev/null || true'
scp "$BIN" "$PI:/usr/local/bin/moodlightpi"
scp deploy/moodlightpi.service "$PI:/etc/systemd/system/moodlightpi.service"
ssh "$PI" 'systemctl daemon-reload && systemctl enable --now moodlightpi && systemctl status --no-pager moodlightpi'
