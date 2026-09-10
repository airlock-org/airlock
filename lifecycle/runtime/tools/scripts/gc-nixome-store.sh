#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
MODE="${1:-dry-run}"
shift || true

source "$ROOT/lifecycle/runtime/tools/scripts/lib/nixome-env.sh"

NIX_STORE_BIN="${NIXOME_NIX_STORE_BIN:-nix-store}"
QEMU_IMG_BIN="${NIXOME_QEMU_IMG_BIN:-qemu-img}"
ALLOW_NONSTANDARD_DATA_ROOT="${NIXOME_GC_ALLOW_NONSTANDARD_DATA_ROOT:-0}"
ALLOW_RUNNING_VM="${NIXOME_GC_ALLOW_RUNNING_VM:-0}"
AUTO_THRESHOLD_GIB="${NIXOME_GC_AUTO_THRESHOLD_GIB:-80}"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
VM_DIR="${NIXOME_VM_DIR:-$ROOT/.nixome-vm}"
RESULT_LINK_DIR="${NIXOME_RESULT_LINK_DIR:-${NIXOME_DATA_ROOT:-$ROOT/.nixome}/result-links}"

usage() {
  cat <<'EOF'
usage: gc-nixome-store.sh MODE

Modes:
  auto      Refresh roots and run full GC when the Nixome store is large.
  dry-run   Refresh Nixome GC roots and list dead paths without deleting them.
  run       Refresh Nixome GC roots and delete all unreachable store paths.
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
  auto|dry-run|run|roots) ;;
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
dead_stderr_log="$LOG_DIR/nixome-store-gc-dead-paths.stderr.log"
gc_log="$LOG_DIR/nixome-store-gc.log"
result_json="$LOG_DIR/nixome-store-gc.json"
manifest="$gcroots_dir/manifest.tsv"
dead_count=0
gc_ran=false
store_bytes_before=0
store_bytes_after=0
threshold_bytes=0

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
if [ ! -d "$store_dir" ]; then
  if [ "$MODE" = auto ]; then
    printf 'Nixome store directory is missing; auto GC skipped: %s\n' "$store_dir"
    exit 0
  fi
  die "Nixome store directory is missing: $store_dir"
fi

case "$AUTO_THRESHOLD_GIB" in
  ''|*[!0-9]*)
    die "NIXOME_GC_AUTO_THRESHOLD_GIB must be a non-negative integer"
    ;;
esac

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
  for link in "$ROOT"/result "$ROOT"/result-* "$VM_DIR"/*-result "$RESULT_LINK_DIR"/*; do
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
    auto|dry-run)
      env NIX_CONFIG="$nix_config" "$NIX_STORE_BIN" --gc --print-dead \
        > "$dead_log" 2> "$dead_stderr_log"
      ;;
    run)
      run_full_gc
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

run_full_gc() {
  local nix_config
  nix_config="store = $nix_root"

  env NIX_CONFIG="$nix_config" "$NIX_STORE_BIN" --gc > "$gc_log" 2>&1
  gc_ran=true
  if [ -s "$gc_log" ]; then
    printf 'GC output tail:\n'
    tail -n 20 "$gc_log"
  fi
}

store_size_bytes() {
  du -s -B1 "$nix_root" | awk '{ print $1 }'
}

write_result_json() {
  jq -c -n \
    --arg schema "nixome.store-gc.v1" \
    --arg mode "$MODE" \
    --arg data_root "$data_root" \
    --arg nix_root "$nix_root" \
    --arg store_dir "$store_dir" \
    --arg gcroots_dir "$gcroots_dir" \
    --arg manifest "$manifest" \
    --arg dead_log "$dead_log" \
    --arg dead_stderr_log "$dead_stderr_log" \
    --arg gc_log "$gc_log" \
    --arg result_json "$result_json" \
    --argjson root_count "$root_count" \
    --argjson auto_threshold_gib "$AUTO_THRESHOLD_GIB" \
    --argjson threshold_bytes "$threshold_bytes" \
    --argjson store_bytes_before "$store_bytes_before" \
    --argjson store_bytes_after "$store_bytes_after" \
    --argjson dead_count "$dead_count" \
    --argjson gc_ran "$gc_ran" \
    '{
      schema: $schema,
      mode: $mode,
      data_root: $data_root,
      nix_root: $nix_root,
      store_dir: $store_dir,
      gcroots_dir: $gcroots_dir,
      root_manifest: $manifest,
      root_count: $root_count,
      auto_threshold_gib: $auto_threshold_gib,
      threshold_bytes: $threshold_bytes,
      store_bytes_before: $store_bytes_before,
      store_bytes_after: $store_bytes_after,
      dead_count: $dead_count,
      gc_ran: $gc_ran,
      dead_log: $dead_log,
      dead_stderr_log: $dead_stderr_log,
      gc_log: $gc_log,
      result_json: $result_json
    }' > "$result_json"
}

refresh_roots
store_bytes_before="$(store_size_bytes)"
threshold_bytes=$((AUTO_THRESHOLD_GIB * 1024 * 1024 * 1024))
print_summary

case "$MODE" in
  roots)
    printf 'Root manifest:    %s\n' "$manifest"
    ;;
  auto)
    run_nix_store_gc
    dead_count="$(grep -c '^/nix/store/' "$dead_log" || true)"
    printf 'Auto threshold:   %s GiB\n' "$AUTO_THRESHOLD_GIB"
    printf 'Store bytes:      %s\n' "$store_bytes_before"
    printf 'Dead path list:   %s\n' "$dead_log"
    printf 'Dead stderr log:  %s\n' "$dead_stderr_log"
    printf 'Dead path count:  %s\n' "$dead_count"
    if [ "$dead_count" -gt 0 ] && [ "$store_bytes_before" -ge "$threshold_bytes" ]; then
      printf 'Auto decision:    running full GC\n'
      run_full_gc
      printf 'GC log:           %s\n' "$gc_log"
    else
      printf 'Auto decision:    skipped\n'
    fi
    ;;
  dry-run)
    run_nix_store_gc
    dead_count="$(grep -c '^/nix/store/' "$dead_log" || true)"
    printf 'Dead path list:   %s\n' "$dead_log"
    printf 'Dead stderr log:  %s\n' "$dead_stderr_log"
    printf 'Dead path count:  %s\n' "$dead_count"
    ;;
  run)
    printf 'GC mode:          full\n'
    run_nix_store_gc
    printf 'GC log:           %s\n' "$gc_log"
    ;;
esac

store_bytes_after="$(store_size_bytes)"
write_result_json
printf 'Result JSON:      %s\n' "$result_json"
