#!/usr/bin/env bash
# Build (natively on the Pi) and install/enable the systemd service.
set -euo pipefail
cd "$(dirname "$0")/.."
PI=${MLP_PI:-root@192.168.1.230}
SRC=${MLP_SRC:-/root/moodlightpi}

./deploy/build.sh

echo "installing binary + unit and (re)starting service ..."
ssh "$PI" "
  systemctl stop moodlightpi 2>/dev/null || true
  install -m 755 '$SRC/target/release/moodlightpi' /usr/local/bin/moodlightpi
  install -m 644 '$SRC/deploy/moodlightpi.service' /etc/systemd/system/moodlightpi.service
  systemctl daemon-reload
  systemctl enable --now moodlightpi
  sleep 3
  systemctl is-active moodlightpi
  systemctl status --no-pager moodlightpi | sed -n '1,6p'
"
