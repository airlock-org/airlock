#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

WORK_DIR="${NIXOME_GC_TEST_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/gc-nixome-store}"
DATA_ROOT="$WORK_DIR/data"
NIX_ROOT="$DATA_ROOT/nix-root"
STORE_DIR="$NIX_ROOT/nix/store"
VM_DIR="$DATA_ROOT/vm"
LOG_DIR="$DATA_ROOT/logs"
BIN_DIR="$WORK_DIR/bin"
MOCK_LOG="$WORK_DIR/mock.log"

rm -rf "$WORK_DIR"
mkdir -p \
  "$BIN_DIR" \
  "$STORE_DIR/11111111111111111111111111111111-vm-image" \
  "$STORE_DIR/22222222222222222222222222222222-backing-image" \
  "$STORE_DIR/33333333333333333333333333333333-system" \
  "$VM_DIR" \
  "$LOG_DIR"

ln -s "$STORE_DIR/11111111111111111111111111111111-vm-image" "$VM_DIR/nixome-vm-gui-config-image-result"
touch "$VM_DIR/nixome-vm-gui-config-image.qcow2"

cat > "$LOG_DIR/config-apply.json" <<'EOF'
{
  "schema": "nixome.config-apply.v1",
  "system_apply_result": {
    "host_system_source": "/nix/store/33333333333333333333333333333333-system"
  },
  "apply_log": "this noisy log may mention /nix/store/99999999999999999999999999999999-do-not-root"
}
EOF

cat > "$BIN_DIR/nix-store" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

{
  printf 'NIX_CONFIG=%s\n' "${NIX_CONFIG:-}"
  printf 'ARGS='
  printf '[%s]' "$@"
  printf '\n'
} >> "$NIXOME_GC_TEST_MOCK_LOG"

case "$*" in
  "--gc --print-dead")
    printf '/nix/store/deadbeefdeadbeefdeadbeefdeadbeef-dead\n'
    ;;
  "--gc")
    printf 'freed 0 bytes\n'
    ;;
  *)
    printf 'unexpected nix-store args: %s\n' "$*" >&2
    exit 2
    ;;
esac
EOF
chmod 0755 "$BIN_DIR/nix-store"

cat > "$BIN_DIR/qemu-img" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "$1" = "info" ]; then
  cat <<'INFO'
image: test.qcow2
file format: qcow2
backing file: /nix/store/22222222222222222222222222222222-backing-image/base.qcow2
INFO
  exit 0
fi

printf 'unexpected qemu-img args: %s\n' "$*" >&2
exit 2
EOF
chmod 0755 "$BIN_DIR/qemu-img"

env \
  PATH="$BIN_DIR:$PATH" \
  NIXOME_GC_ALLOW_NONSTANDARD_DATA_ROOT=1 \
  NIXOME_GC_TEST_MOCK_LOG="$MOCK_LOG" \
  NIXOME_DATA_ROOT="$DATA_ROOT" \
  NIXOME_NIX_ROOT="$NIX_ROOT" \
  NIXOME_NIX_STORE="$NIX_ROOT" \
  NIXOME_VM_DIR="$VM_DIR" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_NIX_STORE_BIN="$BIN_DIR/nix-store" \
  NIXOME_QEMU_IMG_BIN="$BIN_DIR/qemu-img" \
  "$ROOT/scripts/gc-nixome-store.sh" dry-run > "$WORK_DIR/dry-run.out"

ROOTS_DIR="$NIX_ROOT/nix/var/nix/gcroots/nixome"
MANIFEST="$ROOTS_DIR/manifest.tsv"

grep -F 'store = '"$NIX_ROOT" "$MOCK_LOG" >/dev/null
grep -F 'ARGS=[--gc][--print-dead]' "$MOCK_LOG" >/dev/null
grep -F '/nix/store/11111111111111111111111111111111-vm-image' "$MANIFEST" >/dev/null
grep -F '/nix/store/22222222222222222222222222222222-backing-image' "$MANIFEST" >/dev/null
grep -F '/nix/store/33333333333333333333333333333333-system' "$MANIFEST" >/dev/null
! grep -F '99999999999999999999999999999999-do-not-root' "$MANIFEST" >/dev/null
grep -F 'Dead path count:  1' "$WORK_DIR/dry-run.out" >/dev/null

env \
  PATH="$BIN_DIR:$PATH" \
  NIXOME_GC_ALLOW_NONSTANDARD_DATA_ROOT=1 \
  NIXOME_GC_TEST_MOCK_LOG="$MOCK_LOG" \
  NIXOME_DATA_ROOT="$DATA_ROOT" \
  NIXOME_NIX_ROOT="$NIX_ROOT" \
  NIXOME_NIX_STORE="$NIX_ROOT" \
  NIXOME_VM_DIR="$VM_DIR" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_NIX_STORE_BIN="$BIN_DIR/nix-store" \
  NIXOME_QEMU_IMG_BIN="$BIN_DIR/qemu-img" \
  "$ROOT/scripts/gc-nixome-store.sh" run > "$WORK_DIR/run.out"

grep -F 'ARGS=[--gc]' "$MOCK_LOG" >/dev/null

if env \
  NIXOME_DATA_ROOT="/tmp/nixome-gc-nonstandard-$$" \
  NIXOME_NIX_ROOT="/tmp/nixome-gc-nonstandard-$$/nix-root" \
  NIXOME_NIX_STORE="/tmp/nixome-gc-nonstandard-$$/nix-root" \
  NIXOME_LOG_DIR="/tmp/nixome-gc-nonstandard-$$/logs" \
  NIXOME_VM_DIR="/tmp/nixome-gc-nonstandard-$$/vm" \
  "$ROOT/scripts/gc-nixome-store.sh" dry-run > "$WORK_DIR/nonstandard.out" 2>&1; then
  printf 'expected nonstandard data root guard to fail\n' >&2
  exit 1
fi
grep -F 'refusing nonstandard data root' "$WORK_DIR/nonstandard.out" >/dev/null

printf 'gc nixome store test ok\n'
