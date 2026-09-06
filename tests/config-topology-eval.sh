#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="$LOG_DIR/config-topology"
DOMAINS_JSON="$WORK_DIR/domains.json"
KERNELS_JSON="$WORK_DIR/kernels.json"
TEMPLATES_JSON="$WORK_DIR/templates.json"
TOPOLOGY_JSON="$WORK_DIR/topology.json"
GUEST_INIT_WANTED_BY_JSON="$WORK_DIR/guest-init-wanted-by.json"
GUEST_INIT_AFTER_JSON="$WORK_DIR/guest-init-after.json"
GUEST_INIT_BEFORE_JSON="$WORK_DIR/guest-init-before.json"
GUEST_QREXEC_REQUIRES_JSON="$WORK_DIR/guest-qrexec-requires.json"
GUEST_QREXEC_AFTER_JSON="$WORK_DIR/guest-qrexec-after.json"
GUEST_VALIDATOR_ROLE_JSON="$WORK_DIR/guest-validator-role.json"
GUEST_VALIDATOR_RPC_MODE_JSON="$WORK_DIR/guest-validator-rpc-mode.json"

mkdir -p "$WORK_DIR"

nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.domains" > "$DOMAINS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.kernels" > "$KERNELS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.templates" > "$TEMPLATES_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.topology" > "$TOPOLOGY_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.wantedBy" > "$GUEST_INIT_WANTED_BY_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.after" > "$GUEST_INIT_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.nixome-config-guest-init.before" > "$GUEST_INIT_BEFORE_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.qubes-qrexec-agent.requires" > "$GUEST_QREXEC_REQUIRES_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.systemd.services.qubes-qrexec-agent.after" > "$GUEST_QREXEC_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-validator.config.nixome.qubeGuest.role" > "$GUEST_VALIDATOR_ROLE_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.Config.ValidateCandidate\".mode" > "$GUEST_VALIDATOR_RPC_MODE_JSON"

jq -e '
  ."config-vault".class == "AppVM" and
  ."config-vault".template == "nixome-nixos-config-vault" and
  ."config-vault".netvm == null and
  ."config-vault".features."nixome-role" == "config-vault" and
  ."config-staging".class == "AppVM" and
  ."config-staging".template == "nixome-nixos-config-staging" and
  ."config-staging".netvm == null and
  ."config-staging".features."nixome-role" == "config-staging" and
  ."config-validator".class == "AppVM" and
  ."config-validator".template == "nixome-nixos-config-validator" and
  ."config-validator".netvm == null and
  ."config-validator".features."nixome-role" == "config-validator"
' "$DOMAINS_JSON" >/dev/null

jq -e '
  ."6.12.64-1.fc41".source | test("qubes-kernel-6.12.64-1.fc41")
' "$KERNELS_JSON" >/dev/null

jq -e '
  ."nixome-nixos-config-vault".import == true and
  ."nixome-nixos-config-vault".skipStart == true and
  (."nixome-nixos-config-vault".source | test("nixome-nixos-config-vault")) and
  ."nixome-nixos-config-staging".import == true and
  ."nixome-nixos-config-staging".skipStart == true and
  (."nixome-nixos-config-staging".source | test("nixome-nixos-config-staging")) and
  ."nixome-nixos-config-validator".import == true and
  ."nixome-nixos-config-validator".skipStart == true and
  (."nixome-nixos-config-validator".source | test("nixome-nixos-config-validator"))
' "$TEMPLATES_JSON" >/dev/null

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  .templates."nixome-nixos-config-staging".skip_start == true and
  .domains."config-staging".prefs.autostart == false and
  .domains."config-staging".prefs.include_in_backups == true and
  .domains."config-validator".prefs.autostart == false and
  .domains."config-validator".prefs.include_in_backups == true
' "$TOPOLOGY_JSON" >/dev/null

jq -e '
  index("multi-user.target") != null
' "$GUEST_INIT_WANTED_BY_JSON" >/dev/null

jq -e '
  index("nixome-qubes-rw-init.service") != null and
  index("qubes-db.service") != null
' "$GUEST_INIT_AFTER_JSON" >/dev/null

jq -e '
  index("qubes-qrexec-agent.service") != null
' "$GUEST_INIT_BEFORE_JSON" >/dev/null

jq -e '
  index("nixome-config-guest-init.service") != null
' "$GUEST_QREXEC_REQUIRES_JSON" >/dev/null

jq -e '
  index("nixome-config-guest-init.service") != null
' "$GUEST_QREXEC_AFTER_JSON" >/dev/null

jq -e '
  . == "config-validator"
' "$GUEST_VALIDATOR_ROLE_JSON" >/dev/null

jq -e '
  . == "0555"
' "$GUEST_VALIDATOR_RPC_MODE_JSON" >/dev/null

printf 'config topology eval ok\n'
