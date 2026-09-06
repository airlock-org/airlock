#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-usage}"

usage() {
  cat <<EOF
usage: $0 MODE

Modes:
  disk-usage  Show generated Nixome directory sizes.
  vm-disks    Remove generated overlays/out-links in .nixome-vm only.
  vm-workdirs Remove all generated .nixome-vm* directories.
  logs        Remove .nixome-logs.
  cache       Remove .nixome-cache, including cached Qubes RPM/template assets.
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
  [ -d "$ROOT/.nixome-vm" ] || return 0

  while IFS= read -r -d '' path; do
    delete_path "$path"
  done < <(
    find "$ROOT/.nixome-vm" -mindepth 1 -maxdepth 1 \
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
  while IFS= read -r -d '' path; do
    delete_path "$path"
  done < <(find "$ROOT" -maxdepth 1 -type d -name '.nixome-vm*' -print0)
}

show_disk_usage() {
  du -sh "$ROOT"/.nixome-* "$ROOT"/result "$ROOT"/result-* 2>/dev/null || true
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
    delete_path "$ROOT/.nixome-logs"
    ;;
  cache)
    delete_path "$ROOT/.nixome-cache"
    ;;
  usage | --help | -h)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
