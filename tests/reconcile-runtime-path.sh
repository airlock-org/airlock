#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
LOG="$LOG_DIR/reconcile-runtime-path.commands"

mkdir -p "$LOG_DIR"

out="$(nix build "$ROOT#packages.x86_64-linux.nixome-qubes-reconcile" --no-link --print-out-paths)"
wrapper="$out/bin/nixome-qubes-reconcile"

path_value="$(sed -n 's/^export PATH="\(.*\):\$PATH"$/\1/p' "$wrapper" | head -n 1)"
if [ -z "$path_value" ]; then
  printf 'could not extract runtime PATH from %s\n' "$wrapper" >&2
  exit 1
fi

required=(
  dd
  mktemp
  qvm-appmenus
  qvm-template-postprocess
  qubesd-query
  runuser
  sudo
)

if ! env -i PATH="$path_value" "$BASH" -c '
  for cmd in "$@"; do
    command -v "$cmd"
  done
' _ "${required[@]}" > "$LOG"; then
  printf 'reconciler runtime PATH is missing a Qubes template import helper; see %s\n' "$LOG" >&2
  exit 1
fi

printf 'reconcile runtime path test ok\n'
