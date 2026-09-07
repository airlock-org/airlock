#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nix-closure-store-image"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

nix build \
  --out-link "$RESULT_DIR/store-image" \
  "$ROOT#nixome-nix-closure-store-image-test" \
  > "$RESULT_DIR/nix-build.stdout" \
  2> "$RESULT_DIR/nix-build.stderr"

STORE_IMAGE_OUT="$(nixome_physical_store_path "$(readlink "$RESULT_DIR/store-image")")"
STORE_PATH="$(head -n 1 "$STORE_IMAGE_OUT/store-paths")"
STORE_ENTRY="${STORE_PATH#/nix/store/}"

case "$STORE_ENTRY" in
  ""|*/*)
    printf 'unexpected store path in closure list: %s\n' "$STORE_PATH" >&2
    exit 1
    ;;
esac

jq -e '
  .schema == "nixome.store-closure-image.v1" and
  .mountpoint == "/nix/store" and
  .device == "/dev/xvde" and
  .read_only == true
' "$STORE_IMAGE_OUT/manifest.json" >/dev/null

debugfs -R "stat /$STORE_ENTRY" "$STORE_IMAGE_OUT/store.img" \
  > "$RESULT_DIR/debugfs-store-entry.txt" \
  2> "$RESULT_DIR/debugfs-store-entry.stderr"

debugfs -R 'stat /nix' "$STORE_IMAGE_OUT/store.img" \
  > "$RESULT_DIR/debugfs-nix-dir.txt" \
  2> "$RESULT_DIR/debugfs-nix-dir.stderr" || true

if ! grep -qi 'File not found' "$RESULT_DIR/debugfs-nix-dir.txt" "$RESULT_DIR/debugfs-nix-dir.stderr"; then
  printf 'store image unexpectedly contains /nix; it should mount directly at /nix/store\n' >&2
  cat "$RESULT_DIR/debugfs-nix-dir.txt" >&2
  cat "$RESULT_DIR/debugfs-nix-dir.stderr" >&2
  exit 1
fi

printf 'nix-closure store image layout test ok\n'
