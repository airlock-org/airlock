#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/config-candidate-flake"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-config-candidate-flake.XXXXXX")"
TOPOLOGY_JSON="$RESULT_DIR/topology.json"

trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"
mkdir -p "$WORK_DIR/nixome"
cp "$ROOT/seeds/config-staging/flake.nix" "$WORK_DIR/flake.nix"
cp "$ROOT/lib/nixome/topology.nix" "$WORK_DIR/nixome/topology.nix"

nix flake check "$WORK_DIR" --no-build --show-trace
nix eval --json "$WORK_DIR#nixome.topology" > "$TOPOLOGY_JSON"

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  (.domains | type == "object") and
  .domains."config-staging".class == "AppVM" and
  .domains."config-staging".netvm == null and
  .domains."config-staging".prefs.autostart == false and
  .domains."config-staging".prefs.include_in_backups == true and
  .domains."config-staging".services == {} and
  .domains."config-staging".features."nixome-config-applied-message" == "initial"
' "$TOPOLOGY_JSON" >/dev/null

printf 'config candidate flake eval ok\n'
