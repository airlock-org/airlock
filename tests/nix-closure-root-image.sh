#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nix-closure-root-image"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

nix build \
  --out-link "$RESULT_DIR/root-image" \
  "$ROOT#nixome-nix-closure-root-image-test" \
  > "$RESULT_DIR/nix-build.stdout" \
  2> "$RESULT_DIR/nix-build.stderr"

ROOT_IMAGE_OUT="$(nixome_physical_store_path "$(readlink "$RESULT_DIR/root-image")")"

jq -e '
  .schema == "nixome.nix-closure-root-image.v1" and
  .contains_store == false and
  .store_mountpoint == "/nix/store" and
  .store_device_default == "/dev/xvde"
' "$ROOT_IMAGE_OUT/manifest.json" >/dev/null

grep -Fx './sbin/init' "$ROOT_IMAGE_OUT/root-files.txt" >/dev/null
grep -Fx './nix/store' "$ROOT_IMAGE_OUT/root-files.txt" >/dev/null

if grep -E '^\./nix/store/.+' "$ROOT_IMAGE_OUT/root-files.txt" >/dev/null; then
  printf 'root image file list contains store entries beyond the mountpoint\n' >&2
  exit 1
fi

debugfs -R 'ls -p /nix/store' "$ROOT_IMAGE_OUT/root.img" \
  > "$RESULT_DIR/debugfs-nix-store.txt" \
  2> "$RESULT_DIR/debugfs-nix-store.stderr"

if grep -E '/[A-Za-z0-9]{32}-' "$RESULT_DIR/debugfs-nix-store.txt" >/dev/null; then
  printf 'root image /nix/store contains Nix store paths\n' >&2
  cat "$RESULT_DIR/debugfs-nix-store.txt" >&2
  exit 1
fi

printf 'nix-closure root image separation test ok\n'
