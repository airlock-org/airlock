#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="$LOG_DIR/config-topology"
DOMAINS_JSON="$WORK_DIR/domains.json"
TEMPLATES_JSON="$WORK_DIR/templates.json"

mkdir -p "$WORK_DIR"

nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.domains" > "$DOMAINS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.templates" > "$TEMPLATES_JSON"

jq -e '
  ."sys-config-vault".class == "AppVM" and
  ."sys-config-vault".template == "nixome-nixos-config-vault" and
  ."sys-config-vault".netvm == null and
  ."sys-config-vault".features."nixome-role" == "config-vault" and
  ."sys-agent".class == "AppVM" and
  ."sys-agent".template == "nixome-nixos-agent" and
  ."sys-agent".netvm == null and
  ."sys-agent".features."nixome-role" == "agent"
' "$DOMAINS_JSON" >/dev/null

jq -e '
  ."nixome-nixos-config-vault".import == true and
  ."nixome-nixos-config-vault".skipStart == true and
  (."nixome-nixos-config-vault".source | test("nixome-nixos-config-vault")) and
  ."nixome-nixos-agent".import == true and
  ."nixome-nixos-agent".skipStart == true and
  (."nixome-nixos-agent".source | test("nixome-nixos-agent"))
' "$TEMPLATES_JSON" >/dev/null

printf 'config topology eval ok\n'
