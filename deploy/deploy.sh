#!/usr/bin/env bash
set -euo pipefail
PI=root@192.168.1.230
TARGET=arm-unknown-linux-gnueabihf
BIN=target/$TARGET/release/moodlightpi

cross build --release --features hardware --target "$TARGET"
file "$BIN" | grep -q 'ARM' || { echo "not an ARM binary"; exit 1; }

ssh "$PI" 'systemctl stop moodlightpi 2>/dev/null || true'
scp "$BIN" "$PI:/usr/local/bin/moodlightpi"
scp deploy/moodlightpi.service "$PI:/etc/systemd/system/moodlightpi.service"
ssh "$PI" 'systemctl daemon-reload && systemctl enable --now moodlightpi && systemctl status --no-pager moodlightpi'
