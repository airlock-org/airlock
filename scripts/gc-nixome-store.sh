#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-dry-run}"
shift || true

source "$ROOT/scripts/lib/nixome-env.sh"

NIX_STORE_BIN="${NIXOME_NIX_STORE_BIN:-nix-store}"
QEMU_IMG_BIN="${NIXOME_QEMU_IMG_BIN:-qemu-img}"
GC_MAX="${NIXOME_GC_MAX:-10G}"
ALLOW_NONSTANDARD_DATA_ROOT="${NIXOME_GC_ALLOW_NONSTANDARD_DATA_ROOT:-0}"
ALLOW_RUNNING_VM="${NIXOME_GC_ALLOW_RUNNING_VM:-0}"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
VM_DIR="${NIXOME_VM_DIR:-$ROOT/.nixome-vm}"

usage() {
  cat <<'EOF'
usage: gc-nixome-store.sh MODE [--max SIZE]

Modes:
  dry-run   Refresh Nixome GC roots and list dead paths without deleting them.
  run       Refresh Nixome GC roots and collect up to SIZE, default 10G.
  roots     Refresh Nixome GC roots only.

This script refuses to operate on the host /nix/store. It is intended for the
local Nixome store at /run/media/user/data/nixome/nix-root.
EOF
}

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --max)
      shift
      [ "$#" -gt 0 ] || die "--max requires a size"
      GC_MAX="$1"
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

case "$MODE" in
  dry-run|run|roots) ;;
  --help|-h)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

realpath_m() {
  realpath -m "$1"
}

data_root="$(realpath_m "${NIXOME_DATA_ROOT:-}")"
nix_root="$(realpath_m "${NIXOME_NIX_ROOT:-}")"
expected_data_root="/run/media/user/data/nixome"
expected_nix_root="$data_root/nix-root"
store_dir="$nix_root/nix/store"
gcroots_dir="$nix_root/nix/var/nix/gcroots/nixome"
lock_file="${NIXOME_DATA_ROOT:-$ROOT/.nixome}/gc.lock"
dead_log="$LOG_DIR/nixome-store-gc-dead-paths.log"
gc_log="$LOG_DIR/nixome-store-gc.log"
manifest="$gcroots_dir/manifest.tsv"

[ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] ||
  die "refusing to GC while NIXOME_USE_HOST_NIX_STORE=1"
[ -n "${NIXOME_DATA_ROOT:-}" ] || die "NIXOME_DATA_ROOT is not set"
[ -n "${NIXOME_NIX_ROOT:-}" ] || die "NIXOME_NIX_ROOT is not set"

case "$data_root" in
  "$expected_data_root"|"$expected_data_root"/*) ;;
  *)
    [ "$ALLOW_NONSTANDARD_DATA_ROOT" = "1" ] ||
      die "refusing nonstandard data root: $data_root"
    ;;
esac

[ "$nix_root" = "$expected_nix_root" ] ||
  die "refusing NIXOME_NIX_ROOT outside the expected Nixome store: $nix_root"
[ "$nix_root" != "/" ] || die "refusing to use / as Nix root"
[ "$nix_root" != "/nix" ] || die "refusing to use /nix as Nix root"
[ "$nix_root" != "/nix/store" ] || die "refusing to use /nix/store as Nix root"
[ "$store_dir" != "/nix/store" ] || die "refusing to use host /nix/store"
[ -d "$store_dir" ] || die "Nixome store directory is missing: $store_dir"

if [ "$ALLOW_RUNNING_VM" != "1" ]; then
  if pgrep -f "qemu-system.*${VM_DIR}" >/dev/null 2>&1; then
    die "refusing to GC while a Nixome QEMU VM appears to be running"
  fi
fi

mkdir -p "$LOG_DIR" "$(dirname "$lock_file")" "$gcroots_dir"
exec 9>"$lock_file"
flock -n 9 || die "another Nixome GC operation is running"

declare -A rooted=()
root_count=0

safe_name() {
  printf '%s' "$1" | sed -e 's/[^A-Za-z0-9_.-]/_/g'
}

logical_store_root_from_path() {
  local path="$1"
  local rest
  local name

  case "$path" in
    /nix/store/*)
      rest="${path#/nix/store/}"
      name="${rest%%/*}"
      [ -n "$name" ] || return 1
      [ -e "$store_dir/$name" ] || return 1
      printf '/nix/store/%s\n' "$name"
      ;;
    "$store_dir"/*)
      rest="${path#$store_dir/}"
      name="${rest%%/*}"
      [ -n "$name" ] || return 1
      [ -e "$store_dir/$name" ] || return 1
      printf '/nix/store/%s\n' "$name"
      ;;
    *)
      return 1
      ;;
  esac
}

readlink_target() {
  local link="$1"
  local target

  target="$(readlink "$link")" || return 1
  case "$target" in
    /*)
      printf '%s\n' "$target"
      ;;
    *)
      printf '%s/%s\n' "$(dirname "$link")" "$target"
      ;;
  esac
}

add_gc_root() {
  local name="$1"
  local candidate="$2"
  local reason="$3"
  local logical
  local link_name

  logical="$(logical_store_root_from_path "$candidate")" || return 0
  if [ -n "${rooted[$logical]+x}" ]; then
    return 0
  fi

  link_name="$(safe_name "$name")"
  ln -sfn "$logical" "$gcroots_dir/$link_name"
  printf '%s\t%s\t%s\n' "$link_name" "$logical" "$reason" >> "$manifest"
  rooted[$logical]=1
  root_count=$((root_count + 1))
}

clear_managed_roots() {
  local path

  if [ -d "$gcroots_dir" ]; then
    while IFS= read -r -d '' path; do
      [ -L "$path" ] || [ -f "$path" ] || continue
      unlink "$path"
    done < <(find "$gcroots_dir" -mindepth 1 -maxdepth 1 \( -type l -o -type f \) -print0)
  fi
  : > "$manifest"
}

root_out_links() {
  local link
  local target

  shopt -s nullglob
  for link in "$ROOT"/result "$ROOT"/result-* "$VM_DIR"/*-result; do
    [ -L "$link" ] || continue
    target="$(readlink_target "$link")" || continue
    add_gc_root "outlink-$(basename "$link")" "$target" "out-link:$link"
  done
  shopt -u nullglob
}

root_qcow_backing_files() {
  local image
  local backing

  command -v "$QEMU_IMG_BIN" >/dev/null 2>&1 || {
    printf 'warning: qemu-img not found; skipping qcow2 backing-file roots\n' >&2
    return 0
  }

  while IFS= read -r -d '' image; do
    while IFS= read -r backing; do
      [ -n "$backing" ] || continue
      add_gc_root "qcow-backing-$(basename "$image")-$(basename "$backing")" \
        "$backing" \
        "qcow2-backing:$image"
    done < <(
      "$QEMU_IMG_BIN" info --backing-chain "$image" 2>/dev/null |
        sed -n 's/^backing file: \(.*\)$/\1/p'
    )
  done < <(find "$VM_DIR" -maxdepth 1 -type f -name '*.qcow2' -print0 2>/dev/null || true)
}

root_json_artifacts() {
  local file
  local candidate

  command -v jq >/dev/null 2>&1 || {
    printf 'warning: jq not found; skipping JSON artifact roots\n' >&2
    return 0
  }

  while IFS= read -r -d '' file; do
    while IFS= read -r candidate; do
      add_gc_root "json-$(basename "$file")-$(basename "$candidate")" \
        "$candidate" \
        "json-artifact:$file"
    done < <(
      jq -r '
      .. | objects | to_entries[]
      | select(.key | test("^(source|source_path|output_path|host_system_source|profile_source)$"))
      | .value
      | strings
      | select(startswith("/nix/store/"))
    ' "$file" 2>/dev/null
    )
  done < <(
    find "$LOG_DIR" -maxdepth 1 -type f \
      \( -name '*config*.json' -o -name '*deployment*.json' -o -name '*builds.json' \) \
      -print0 2>/dev/null || true
  )
}

refresh_roots() {
  clear_managed_roots
  root_out_links
  root_qcow_backing_files
  root_json_artifacts
}

run_nix_store_gc() {
  local nix_config
  nix_config="store = $nix_root"

  case "$MODE" in
    dry-run)
      env NIX_CONFIG="$nix_config" "$NIX_STORE_BIN" --gc --print-dead > "$dead_log"
      ;;
    run)
      env NIX_CONFIG="$nix_config" "$NIX_STORE_BIN" --gc --max-freed "$GC_MAX" | tee "$gc_log"
      ;;
  esac
}

print_summary() {
  printf 'Nixome data root: %s\n' "$data_root"
  printf 'Nixome Nix root:  %s\n' "$nix_root"
  printf 'Nixome store dir: %s\n' "$store_dir"
  printf 'Managed GC roots: %s\n' "$gcroots_dir"
  printf 'Rooted paths:     %s\n' "$root_count"
}

refresh_roots
print_summary

case "$MODE" in
  roots)
    printf 'Root manifest:    %s\n' "$manifest"
    ;;
  dry-run)
    run_nix_store_gc
    dead_count="$(grep -c '^/nix/store/' "$dead_log" || true)"
    printf 'Dead path list:   %s\n' "$dead_log"
    printf 'Dead path count:  %s\n' "$dead_count"
    ;;
  run)
    printf 'GC max freed:     %s\n' "$GC_MAX"
    run_nix_store_gc
    printf 'GC log:           %s\n' "$gc_log"
    ;;
esac
