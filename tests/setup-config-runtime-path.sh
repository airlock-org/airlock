#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
LOG="$LOG_DIR/setup-config-runtime-path.commands"

mkdir -p "$LOG_DIR"

out="$(nix build "$ROOT#packages.x86_64-linux.nixome-setup-config" --no-link --print-out-paths)"
wrapper="$(nixome_physical_store_path "$out")/bin/nixome-setup-config"

path_value="$(sed -n 's/^export PATH="\(.*\):\$PATH"$/\1/p' "$wrapper" | head -n 1)"
if [ -z "$path_value" ]; then
  printf 'could not extract runtime PATH from %s\n' "$wrapper" >&2
  exit 1
fi
path_value="$(nixome_physical_path_list "$path_value")"

required=(
  awk
  chmod
  cmp
  cp
  dd
  grep
  jq
  mkdir
  mkfs.erofs
  mktemp
  dmsetup
  losetup
  qubes-prefs
  qubesd-query
  python3
  qvm-create
  qvm-features
  qvm-ls
  qvm-prefs
  qvm-service
  qvm-template-postprocess
  qvm-volume
  rm
  runuser
  sudo
  systemctl
)

if ! env -i PATH="$path_value" "$BASH" -e -c '
  for cmd in "$@"; do
    command -v "$cmd"
  done
' _ "${required[@]}" > "$LOG"; then
  printf 'setup-config runtime PATH is missing a Qubes template import helper; see %s\n' "$LOG" >&2
  exit 1
fi

printf 'setup-config runtime path test ok\n'
