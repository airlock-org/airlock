#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="$LOG_DIR/config-topology"
DOMAINS_JSON="$WORK_DIR/domains.json"
KERNELS_JSON="$WORK_DIR/kernels.json"
TEMPLATES_JSON="$WORK_DIR/templates.json"

mkdir -p "$WORK_DIR"

nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.domains" > "$DOMAINS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.kernels" > "$KERNELS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.templates" > "$TEMPLATES_JSON"

jq -e '
  ."config-vault".class == "AppVM" and
  ."config-vault".template == "nixome-nixos-config-vault" and
  ."config-vault".netvm == null and
  ."config-vault".features."nixome-role" == "config-vault" and
  ."config-mod".class == "AppVM" and
  ."config-mod".template == "nixome-nixos-config-mod" and
  ."config-mod".netvm == null and
  ."config-mod".features."nixome-role" == "config-mod"
' "$DOMAINS_JSON" >/dev/null

jq -e '
  ."6.12.64-1.fc41".source | test("qubes-kernel-6.12.64-1.fc41")
' "$KERNELS_JSON" >/dev/null

jq -e '
  ."nixome-nixos-config-vault".import == true and
  ."nixome-nixos-config-vault".skipStart == true and
  (."nixome-nixos-config-vault".source | test("nixome-nixos-config-vault")) and
  ."nixome-nixos-config-mod".import == true and
  ."nixome-nixos-config-mod".skipStart == true and
  (."nixome-nixos-config-mod".source | test("nixome-nixos-config-mod"))
' "$TEMPLATES_JSON" >/dev/null

printf 'config topology eval ok\n'
