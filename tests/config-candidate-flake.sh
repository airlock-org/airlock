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
cp "$ROOT/seeds/config-staging/nixome/config-vms.nix" "$WORK_DIR/nixome/config-vms.nix"
cp "$ROOT/lib/nixome/topology.nix" "$WORK_DIR/nixome/topology.nix"

nix flake check "$WORK_DIR" --no-build --show-trace
nix eval --json "$WORK_DIR#nixome.topology" > "$TOPOLOGY_JSON"

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  .templates == {} and
  .kernels == {} and
  .prefs.default_kernel == "" and
  (.domains | type == "object") and
  .domains."config-vault".class == "AppVM" and
  .domains."config-vault".template == "nixome-nixos-config-vault" and
  .domains."config-vault".label == "black" and
  .domains."config-vault".netvm == null and
  .domains."config-vault".prefs.autostart == false and
  .domains."config-vault".prefs.include_in_backups == true and
  .domains."config-vault".prefs.memory == 1024 and
  .domains."config-vault".prefs.maxmem == 2048 and
  .domains."config-vault".prefs.vcpus == 1 and
  .domains."config-vault".features."nixome-managed" == "1" and
  .domains."config-vault".features."nixome-role" == "config-vault" and
  .domains."config-vault".features."nixome-config-authority" == "1" and
  .domains."config-vault".services."network-manager" == false and
  .domains."config-staging".class == "AppVM" and
  .domains."config-staging".template == "nixome-nixos-config-staging" and
  .domains."config-staging".label == "gray" and
  .domains."config-staging".netvm == null and
  .domains."config-staging".prefs.autostart == false and
  .domains."config-staging".prefs.include_in_backups == true and
  .domains."config-staging".prefs.memory == 2048 and
  .domains."config-staging".prefs.maxmem == 4096 and
  .domains."config-staging".prefs.vcpus == 2 and
  .domains."config-staging".features."nixome-managed" == "1" and
  .domains."config-staging".features."nixome-role" == "config-staging" and
  .domains."config-staging".features."nixome-config-staging" == "1" and
  .domains."config-staging".services == {} and
  .domains."config-staging".features."nixome-config-applied-message" == "initial" and
  .domains."config-validator".class == "AppVM" and
  .domains."config-validator".template == "nixome-nixos-config-validator" and
  .domains."config-validator".label == "blue" and
  .domains."config-validator".netvm == null and
  .domains."config-validator".prefs.autostart == false and
  .domains."config-validator".prefs.include_in_backups == true and
  .domains."config-validator".prefs.memory == 2048 and
  .domains."config-validator".prefs.maxmem == 4096 and
  .domains."config-validator".prefs.vcpus == 2 and
  .domains."config-validator".features."nixome-managed" == "1" and
  .domains."config-validator".features."nixome-role" == "config-validator" and
  .domains."config-validator".features."nixome-config-validator" == "1" and
  .domains."config-validator".services == {} and
  .qrexec_policy.rules == [
    {
      "service": "nixome.PolicyProbe",
      "argument": "*",
      "source": "dom0",
      "target": "config-vault",
      "action": "allow"
    },
    {
      "service": "nixome.PolicyProbe",
      "argument": "*",
      "source": "@anyvm",
      "target": "@anyvm",
      "action": "deny"
    }
  ]
' "$TOPOLOGY_JSON" >/dev/null

printf 'config candidate flake eval ok\n'
