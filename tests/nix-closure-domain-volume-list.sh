#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/nix-closure-domain-smoke.sh"
WORK_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/nix-closure-domain-volume-list"

mkdir -p "$WORK_DIR"

cat > "$WORK_DIR/qvm-volume-list.txt" <<'EOF'
POOL:VOLUME                                       VMNAME                VOLUME_NAME  REVERT_POSSIBLE
linux-kernel:6.12.64-1.fc41                       nixome-closure-smoke  kernel       No
varlibqubes:appvms/nixome-closure-smoke/private   nixome-closure-smoke  private      No
varlibqubes:appvms/nixome-closure-smoke/root      nixome-closure-smoke  root         No
varlibqubes:appvms/nixome-closure-smoke/store     nixome-closure-smoke  store        No
varlibqubes:appvms/nixome-closure-smoke/volatile  nixome-closure-smoke  volatile     No
EOF

awk -v volume=store '
  NR == 1 { next }
  $3 == volume || $1 ~ "/" volume "$" { found = 1 }
  END { exit found ? 0 : 1 }
' "$WORK_DIR/qvm-volume-list.txt"

if grep -F 'grep -F "$DOMAIN:store"' "$SCRIPT" >/dev/null; then
  printf 'nix-closure-domain-smoke.sh still assumes qvm-volume list prints DOMAIN:store\n' >&2
  exit 1
fi

grep -F 'qvm_volume_list_has_volume "$DOMAIN" store' "$SCRIPT" >/dev/null
grep -F 'touch "$home_dir/nixome-closure-smoke-home"' "$SCRIPT" >/dev/null

if grep -F 'touch /rw/nixome-closure-smoke-persistent' "$SCRIPT" >/dev/null; then
  printf 'nix-closure-domain-smoke.sh still writes directly to /rw as the qrexec user\n' >&2
  exit 1
fi

printf 'store_options=ro,nodev,relatime\n' > "$WORK_DIR/store-options.log"
grep -E '^store_options=([^,]+,)*ro(,|$)' "$WORK_DIR/store-options.log" >/dev/null

printf 'store_options=nodev,rw,relatime\n' > "$WORK_DIR/store-options-rw.log"
if grep -E '^store_options=([^,]+,)*ro(,|$)' "$WORK_DIR/store-options-rw.log" >/dev/null; then
  printf 'read-only mount check matched writable store options\n' >&2
  exit 1
fi
