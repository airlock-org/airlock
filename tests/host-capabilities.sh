#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
OUTPUT=""

if [ "${1:-}" = "--json" ]; then
  OUTPUT="${2:-}"
fi

if [ -z "$OUTPUT" ]; then
  OUTPUT="$LOG_DIR/host-capabilities.json"
fi

mkdir -p "$(dirname "$OUTPUT")"

json_escape() {
  printf '%s' "$1" | sed \
    -e 's/\\/\\\\/g' \
    -e 's/"/\\"/g' \
    -e ':a;N;$!ba;s/\n/\\n/g'
}

json_string() {
  printf '"'
  json_escape "$1"
  printf '"'
}

bool() {
  if "$@"; then
    printf true
  else
    printf false
  fi
}

command_available() {
  command -v "$1" >/dev/null 2>&1
}

file_contains_word() {
  file="$1"
  word="$2"

  [ -r "$file" ] && grep -qw "$word" "$file"
}

cpu_has_flag() {
  flag="$1"
  awk -F: '/flags|Features/ { print $2; exit }' /proc/cpuinfo 2>/dev/null | grep -qw "$flag"
}

nonempty_glob() {
  pattern="$1"
  set -- $pattern
  [ -e "$1" ]
}

virt="$(systemd-detect-virt 2>/dev/null || true)"
xen_caps=""
if [ -r /proc/xen/capabilities ]; then
  xen_caps="$(cat /proc/xen/capabilities)"
fi

kvm_nested=""
if [ -r /sys/module/kvm_intel/parameters/nested ]; then
  kvm_nested="$(cat /sys/module/kvm_intel/parameters/nested)"
elif [ -r /sys/module/kvm_amd/parameters/nested ]; then
  kvm_nested="$(cat /sys/module/kvm_amd/parameters/nested)"
fi

{
  printf '{'
  printf '"schema":"nixome.host-capabilities.v1"'
  printf ',"host":{'
  printf '"kernel":'
  json_string "$(uname -r)"
  printf ',"architecture":'
  json_string "$(uname -m)"
  printf ',"virtualization":'
  json_string "$virt"
  printf '}'
  printf ',"nix":{'
  printf '"available":'
  bool command_available nix
  printf ',"version":'
  json_string "$(nix --version 2>/dev/null || true)"
  printf '}'
  printf ',"xen":{'
  printf '"proc_xen":'
  bool test -e /proc/xen
  printf ',"control_domain":'
  bool file_contains_word /proc/xen/capabilities control_d
  printf ',"capabilities":'
  json_string "$xen_caps"
  printf '}'
  printf ',"hardware_virtualization":{'
  printf '"vmx":'
  bool cpu_has_flag vmx
  printf ',"svm":'
  bool cpu_has_flag svm
  printf ',"ept":'
  bool cpu_has_flag ept
  printf ',"npt":'
  bool cpu_has_flag npt
  printf ',"iommu_groups":'
  bool nonempty_glob "/sys/kernel/iommu_groups/*"
  printf ',"kvm_nested":'
  json_string "$kvm_nested"
  printf '}'
  printf ',"qubes_tools":{'
  printf '"qvm_run":'
  bool command_available qvm-run
  printf ',"qvm_create":'
  bool command_available qvm-create
  printf ',"qubes_prefs":'
  bool command_available qubes-prefs
  printf '}'
  printf '}\n'
} > "$OUTPUT"

cat "$OUTPUT"
