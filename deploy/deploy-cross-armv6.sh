#!/usr/bin/env bash
# Build on this machine with the true ARMv6 cross toolchain, then install on Pi.
set -euo pipefail

cd "$(dirname "$0")/.."

PI=${MLP_PI:-root@192.168.1.230}
ARTIFACT=${MLP_CROSS_ARMV6_ARTIFACT:-$(pwd)/dist/pi-armv6-cross/moodlightpi}
MLP_SSH_OPTS=${MLP_SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=8}
read -r -a SSH_OPTS <<< "$MLP_SSH_OPTS"
REMOTE_TMP=/tmp/moodlightpi-cross-deploy-$$
SKIP_BUILD=0
BUILD_ARGS=()

usage() {
  cat <<USAGE
Usage: deploy/deploy-cross-armv6.sh [--skip-build] [--sync-sysroot]

Builds MoodLightPi on this machine with the true ARMv6 cross toolchain, streams
the artifact to the Pi, installs it, restarts systemd, and verifies /healthz.

Options:
  --skip-build     Deploy existing artifact at $ARTIFACT
  --sync-sysroot   Refresh target/pi-sysroot from MLP_PI before building

Environment overrides:
  MLP_PI                    SSH target, default: $PI
  MLP_CROSS_ARMV6_ARTIFACT  Artifact path, default: $ARTIFACT
  MLP_SSH_OPTS              SSH options, default: $MLP_SSH_OPTS
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build)
      SKIP_BUILD=1
      shift
      ;;
    --sync-sysroot)
      BUILD_ARGS+=(--sync-sysroot)
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

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  if [[ "${#BUILD_ARGS[@]}" -gt 0 ]]; then
    ./deploy/build-cross-armv6.sh "${BUILD_ARGS[@]}"
  else
    ./deploy/build-cross-armv6.sh
  fi
fi

if [[ ! -x "$ARTIFACT" ]]; then
  echo "Missing built ARMv6 artifact: $ARTIFACT" >&2
  echo "Run deploy/build-cross-armv6.sh first, or omit --skip-build." >&2
  exit 1
fi

echo "checking SSH access to $PI ..."
ssh "${SSH_OPTS[@]}" "$PI" "true"

echo "uploading cross-built artifact to $PI ..."
ssh "${SSH_OPTS[@]}" "$PI" "mkdir -p '$REMOTE_TMP'"
cat "$ARTIFACT" | ssh "${SSH_OPTS[@]}" "$PI" "cat > '$REMOTE_TMP/moodlightpi' && chmod 0755 '$REMOTE_TMP/moodlightpi'"
cat deploy/moodlightpi.service | ssh "${SSH_OPTS[@]}" "$PI" "cat > '$REMOTE_TMP/moodlightpi.service' && chmod 0644 '$REMOTE_TMP/moodlightpi.service'"

echo "installing binary + unit and (re)starting service ..."
ssh "${SSH_OPTS[@]}" "$PI" "MLP_REMOTE_TMP='$REMOTE_TMP' bash -s" <<'REMOTE'
  set -euo pipefail
  TMP=$MLP_REMOTE_TMP
  ldd "$TMP/moodlightpi" >/dev/null
  systemctl stop moodlightpi 2>/dev/null || true
  if [ -x /usr/local/bin/moodlightpi ]; then
    cp -f /usr/local/bin/moodlightpi /usr/local/bin/moodlightpi.bak
  fi
  install -m 755 "$TMP/moodlightpi" /usr/local/bin/moodlightpi
  install -m 644 "$TMP/moodlightpi.service" /etc/systemd/system/moodlightpi.service
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
    rm -rf "$TMP"
    exit 1
  fi
  rm -rf "$TMP"
  systemctl is-active moodlightpi
  systemctl status --no-pager moodlightpi | sed -n '1,6p'
REMOTE
