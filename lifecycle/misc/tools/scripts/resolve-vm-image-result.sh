#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/airlock-env.sh"

IMAGE_ATTR="${1:-${AIRLOCK_VM_IMAGE_ATTR:-airlock-vm-gui-config-dev-image}}"
OUT_LINK="${2:-${AIRLOCK_VM_SHARED_IMAGE_LINK:-$AIRLOCK_RESULT_LINK_DIR/${IMAGE_ATTR}-result}}"

if [ -z "$IMAGE_ATTR" ]; then
  printf 'error: image attr must not be empty\n' >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT_LINK")"
if [ -d "$OUT_LINK" ] && [ ! -L "$OUT_LINK" ]; then
  printf 'error: refusing to replace non-symlink directory: %s\n' "$OUT_LINK" >&2
  exit 1
fi

printf 'building/resolving shared Airlock VM image: .#%s\n' "$IMAGE_ATTR" >&2
image_out="$(
  nix build "$ROOT#$IMAGE_ATTR" \
    --no-link \
    --print-out-paths \
    --print-build-logs \
    --show-trace
)"
image_out="$(awk 'NF { last = $0 } END { print last }' <<< "$image_out")"
if [ -z "$image_out" ]; then
  printf 'error: nix build did not print an output path for .#%s\n' "$IMAGE_ATTR" >&2
  exit 1
fi

image_out_physical="$(airlock_physical_store_path "$image_out")"
if [ ! -d "$image_out_physical" ]; then
  printf 'error: resolved VM image output is missing: %s\n' "$image_out_physical" >&2
  exit 1
fi

ln -sfn "$image_out_physical" "$OUT_LINK"
printf 'shared Airlock VM image result link: %s -> %s\n' "$OUT_LINK" "$image_out_physical" >&2
printf '%s\n' "$image_out_physical"
