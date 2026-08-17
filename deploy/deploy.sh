#!/usr/bin/env bash
# Build (natively on the Pi) and install/enable the systemd service.
set -euo pipefail
cd "$(dirname "$0")/.."
PI=${MLP_PI:-root@192.168.1.230}
SRC=${MLP_SRC:-/root/moodlightpi}
MLP_SSH_OPTS=${MLP_SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=8}
read -r -a SSH_OPTS <<< "$MLP_SSH_OPTS"

usage() {
  cat <<USAGE
Usage: deploy/deploy.sh

Builds MoodLightPi natively on the connected Pi Zero W, installs the binary and
systemd unit, then verifies /healthz reports the hardware backend.

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

./deploy/build.sh

echo "installing binary + unit and (re)starting service ..."
REMOTE_SRC=${SRC//\'/\'\\\'\'}
ssh "${SSH_OPTS[@]}" "$PI" "MLP_REMOTE_SRC='$REMOTE_SRC' bash -s" <<'REMOTE'
  set -euo pipefail
  SRC=$MLP_REMOTE_SRC
  systemctl stop moodlightpi 2>/dev/null || true
  if [ -x /usr/local/bin/moodlightpi ]; then
    cp -f /usr/local/bin/moodlightpi /usr/local/bin/moodlightpi.bak
  fi
  install -m 755 "$SRC/target/release/moodlightpi" /usr/local/bin/moodlightpi
  install -m 644 "$SRC/deploy/moodlightpi.service" /etc/systemd/system/moodlightpi.service
  systemctl daemon-reload
  systemctl enable --now moodlightpi
  healthy=false
  for _ in $(seq 1 15); do
    health=$(curl -fsS -m 4 http://127.0.0.1/healthz 2>/dev/null || true)
    if printf '%s' "$health" | grep -q '"backend":"hardware"'; then
      healthy=true
      break
    fi
    sleep 2
  done
  if [ "$healthy" != true ]; then
    echo "MoodLightPi health check failed or did not report backend=hardware" >&2
    journalctl -u moodlightpi -n 40 --no-pager >&2 || true
    if [ -x /usr/local/bin/moodlightpi.bak ]; then
      cp -f /usr/local/bin/moodlightpi.bak /usr/local/bin/moodlightpi
      systemctl restart moodlightpi || true
    fi
    exit 1
  fi
  systemctl is-active moodlightpi
  systemctl status --no-pager moodlightpi | sed -n '1,6p'
REMOTE
