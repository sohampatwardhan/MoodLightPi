#!/usr/bin/env bash
# Copy the Pi's ARMv6/Trixie headers and libraries into a local sysroot for
# native-speed cross compilation.
set -euo pipefail

cd "$(dirname "$0")/.."

PI=${MLP_PI:-root@192.168.1.230}
SYSROOT=${MLP_SYSROOT_DIR:-$(pwd)/target/pi-sysroot}
MLP_SSH_OPTS=${MLP_SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=8}
read -r -a SSH_OPTS <<< "$MLP_SSH_OPTS"

usage() {
  cat <<USAGE
Usage: deploy/sync-pi-sysroot.sh

Copies the Raspberry Pi Zero W ARMv6/Trixie runtime headers and libraries into:

  $SYSROOT

Environment overrides:
  MLP_PI          SSH target, default: $PI
  MLP_SYSROOT_DIR Local sysroot directory, default: $SYSROOT
  MLP_SSH_OPTS    SSH options, default: $MLP_SSH_OPTS
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

echo "checking SSH access to $PI ..."
ssh "${SSH_OPTS[@]}" "$PI" '
  set -euo pipefail
  arch=$(uname -m)
  if [ "$arch" != armv6l ]; then
    echo "Expected Raspberry Pi Zero W armv6l, got: $arch" >&2
    exit 1
  fi
  test -d /usr/include
  test -d /usr/lib/arm-linux-gnueabihf
  test -d /lib/arm-linux-gnueabihf
'

mkdir -p "$SYSROOT"

echo "syncing Pi sysroot from $PI to $SYSROOT ..."
ssh "${SSH_OPTS[@]}" "$PI" "tar --numeric-owner -cf - -C / \
  lib/ld-linux-armhf.so.3 \
  lib/arm-linux-gnueabihf \
  usr/include \
  usr/lib/arm-linux-gnueabihf \
  usr/lib/gcc/arm-linux-gnueabihf" | tar xf - -C "$SYSROOT"

echo "synced: $SYSROOT"
