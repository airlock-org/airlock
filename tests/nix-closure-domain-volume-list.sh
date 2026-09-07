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
