#!/usr/bin/env bash
set -euo pipefail
PI=root@192.168.1.230
TARGET=arm-unknown-linux-gnueabihf
BIN=target/$TARGET/release/moodlightpi

# cross-rs images are amd64-only; on Apple Silicon hosts this runs them under
# emulation (no-op on amd64 hosts). Output binary is still ARMv6.
export DOCKER_DEFAULT_PLATFORM=linux/amd64

cross build --release --features hardware --target "$TARGET"
file "$BIN" | grep -q 'ARM' || { echo "not an ARM binary"; exit 1; }

ssh "$PI" 'systemctl stop moodlightpi 2>/dev/null || true'
scp "$BIN" "$PI:/usr/local/bin/moodlightpi"
scp deploy/moodlightpi.service "$PI:/etc/systemd/system/moodlightpi.service"
ssh "$PI" 'systemctl daemon-reload && systemctl enable --now moodlightpi && systemctl status --no-pager moodlightpi'
