#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-usage}"
if [ -n "${NIXOME_DATA_ROOT:-}" ]; then
  DEFAULT_NIXOME_DATA_ROOT="$NIXOME_DATA_ROOT"
elif [ -d /run/media/user/data ] && [ -w /run/media/user/data ]; then
  DEFAULT_NIXOME_DATA_ROOT="/run/media/user/data/nixome"
else
  DEFAULT_NIXOME_DATA_ROOT=""
fi
VM_DIR="${NIXOME_VM_DIR:-${DEFAULT_NIXOME_DATA_ROOT:+$DEFAULT_NIXOME_DATA_ROOT/vm}}"
VM_DIR="${VM_DIR:-$ROOT/.nixome-vm}"
LOG_DIR="${NIXOME_LOG_DIR:-${DEFAULT_NIXOME_DATA_ROOT:+$DEFAULT_NIXOME_DATA_ROOT/logs}}"
LOG_DIR="${LOG_DIR:-$ROOT/.nixome-logs}"
CACHE_DIR="${NIXOME_QUBES_CACHE_DIR:-${DEFAULT_NIXOME_DATA_ROOT:+$DEFAULT_NIXOME_DATA_ROOT/cache/rpms}}"
CACHE_DIR="${CACHE_DIR:-$ROOT/.nixome-cache/rpms}"
CACHE_ROOT="${CACHE_DIR%/rpms}"

usage() {
  cat <<EOF
usage: $0 MODE

Modes:
  disk-usage  Show generated Nixome directory sizes.
  vm-disks    Remove generated overlays/out-links in the VM work directory.
  vm-workdirs Remove generated VM work directories.
  logs        Remove generated logs.
  cache       Remove generated cache, including cached Qubes RPM/template assets.
EOF
}

delete_path() {
  local path="$1"

  [ -e "$path" ] || return 0
  printf '%s\n' "$path"

  if [ -d "$path" ] && [ ! -L "$path" ]; then
    find "$path" -mindepth 1 -depth -delete
    rmdir "$path"
  else
    unlink "$path"
  fi
}

clean_vm_disks() {
  [ -d "$VM_DIR" ] || return 0

  while IFS= read -r -d '' path; do
    delete_path "$path"
  done < <(
    find "$VM_DIR" -mindepth 1 -maxdepth 1 \
      \( \
        -name '*-result' -o \
        -name '*.qcow2' -o \
        -name '*.qcow2.*.bak' -o \
        -name '*.monitor' -o \
        -name '*-OVMF_VARS.fd' -o \
        -name 'serial.log' \
      \) -print0
  )
}

clean_vm_workdirs() {
  delete_path "$VM_DIR"

  while IFS= read -r -d '' path; do
    [ "$path" = "$VM_DIR" ] && continue
    delete_path "$path"
  done < <(find "$ROOT" -maxdepth 1 -type d -name '.nixome-vm*' -print0)
}

show_disk_usage() {
  du -sh \
    "$VM_DIR" \
    "$LOG_DIR" \
    "$CACHE_ROOT" \
    "$ROOT"/.nixome-* \
    "$ROOT"/result \
    "$ROOT"/result-* \
    2>/dev/null || true
}

case "$MODE" in
  disk-usage)
    show_disk_usage
    ;;
  vm-disks)
    clean_vm_disks
    ;;
  vm-workdirs)
    clean_vm_workdirs
    ;;
  logs)
    delete_path "$LOG_DIR"
    ;;
  cache)
    delete_path "$CACHE_ROOT"
    ;;
  usage | --help | -h)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
