#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
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
GUEST_VAULT_HOSTNAME_JSON="$WORK_DIR/guest-vault-hostname.json"
GUEST_STAGING_HOSTNAME_JSON="$WORK_DIR/guest-staging-hostname.json"
GUEST_VALIDATOR_HOSTNAME_JSON="$WORK_DIR/guest-validator-hostname.json"
GUEST_VALIDATOR_RPC_MODE_JSON="$WORK_DIR/guest-validator-rpc-mode.json"
GUEST_DEPLOYMENT_RPC_MODE_JSON="$WORK_DIR/guest-deployment-rpc-mode.json"
GUEST_VAULT_ROOT_IMAGE_DRV="$WORK_DIR/guest-vault-root-image.drv"
GUEST_STAGING_ROOT_IMAGE_DRV="$WORK_DIR/guest-staging-root-image.drv"
GUEST_VALIDATOR_ROOT_IMAGE_DRV="$WORK_DIR/guest-validator-root-image.drv"
SETUP_CONFIG_WANTED_BY_JSON="$WORK_DIR/setup-config-wanted-by.json"
SETUP_CONFIG_AFTER_JSON="$WORK_DIR/setup-config-after.json"
SETUP_CONFIG_WANTS_JSON="$WORK_DIR/setup-config-wants.json"
SETUP_CONFIG_EXECSTART_JSON="$WORK_DIR/setup-config-execstart.json"
SETUP_CONFIG_ENV_JSON="$WORK_DIR/setup-config-environment.json"
SETUP_CONFIG_RESTART_IF_CHANGED_JSON="$WORK_DIR/setup-config-restart-if-changed.json"
SETUP_CONFIG_STOP_IF_CHANGED_JSON="$WORK_DIR/setup-config-stop-if-changed.json"
QREXEC_POLICY_PATH_BEFORE_JSON="$WORK_DIR/qrexec-policy-path-before.json"
QREXEC_POLICY_DAEMON_AFTER_JSON="$WORK_DIR/qrexec-policy-daemon-after.json"
QREXEC_POLICY_DAEMON_REQUIRES_JSON="$WORK_DIR/qrexec-policy-daemon-requires.json"
QREXEC_POLICY_DAEMON_EXECSTART_JSON="$WORK_DIR/qrexec-policy-daemon-execstart.json"
CONFIG_ACCESS_POLICY_PKG="$WORK_DIR/config-access-policy-package"
CONFIG_ACCESS_POLICY_FILE="$WORK_DIR/30-nixome-config.policy"

rm -rf "$WORK_DIR"
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
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-vault.config.networking.hostName" > "$GUEST_VAULT_HOSTNAME_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-staging.config.networking.hostName" > "$GUEST_STAGING_HOSTNAME_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-validator.config.networking.hostName" > "$GUEST_VALIDATOR_HOSTNAME_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.Config.ValidateCandidate\".mode" > "$GUEST_VALIDATOR_RPC_MODE_JSON"
nix eval --json "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.Config.ArchiveDeployment\".mode" > "$GUEST_DEPLOYMENT_RPC_MODE_JSON"
nix path-info --derivation "$ROOT#nixome-nixos-config-vault-root-image" > "$GUEST_VAULT_ROOT_IMAGE_DRV"
nix path-info --derivation "$ROOT#nixome-nixos-config-staging-root-image" > "$GUEST_STAGING_ROOT_IMAGE_DRV"
nix path-info --derivation "$ROOT#nixome-nixos-config-validator-root-image" > "$GUEST_VALIDATOR_ROOT_IMAGE_DRV"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.wantedBy" > "$SETUP_CONFIG_WANTED_BY_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.after" > "$SETUP_CONFIG_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.wants" > "$SETUP_CONFIG_WANTS_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.serviceConfig.ExecStart" > "$SETUP_CONFIG_EXECSTART_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.environment" > "$SETUP_CONFIG_ENV_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.restartIfChanged" > "$SETUP_CONFIG_RESTART_IF_CHANGED_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-setup-config.stopIfChanged" > "$SETUP_CONFIG_STOP_IF_CHANGED_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.nixome-qubes-policy-path.before" > "$QREXEC_POLICY_PATH_BEFORE_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.after" > "$QREXEC_POLICY_DAEMON_AFTER_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.requires" > "$QREXEC_POLICY_DAEMON_REQUIRES_JSON"
nix eval --json "$ROOT#nixosConfigurations.vm-gui-config.config.systemd.services.qubes-qrexec-policy-daemon.serviceConfig.ExecStart" > "$QREXEC_POLICY_DAEMON_EXECSTART_JSON"
nix build --impure --expr '
let
  packages = (builtins.getFlake "git+file://'"$ROOT"'").nixosConfigurations."vm-gui-config".config.virtualisation.qubes.packages;
  matches = builtins.filter (package: package.name == "nixome-config-access-policy") packages;
in
  builtins.head matches
' --no-link --print-out-paths > "$CONFIG_ACCESS_POLICY_PKG"
cp "$(nixome_physical_store_path "$(cat "$CONFIG_ACCESS_POLICY_PKG")")/etc/qubes/policy.d/30-nixome-config.policy" "$CONFIG_ACCESS_POLICY_FILE"

jq -e '
  ."config-vault".class == "AppVM" and
  ."config-vault".template == "nixome-nixos-config-system" and
  ."config-vault".netvm == null and
  ."config-vault".features."nixome-role" == "config-vault" and
  ."config-staging".class == "AppVM" and
  ."config-staging".template == "nixome-nixos-config-system" and
  ."config-staging".netvm == null and
  ."config-staging".features."nixome-role" == "config-staging" and
  ."config-staging".features."nixome-config-applied-message" == "initial" and
  ."config-validator".class == "AppVM" and
  ."config-validator".template == "nixome-nixos-config-system" and
  ."config-validator".netvm == null and
  ."config-validator".features."nixome-role" == "config-validator"
' "$DOMAINS_JSON" >/dev/null

jq -e '
  ."6.12.64-1.fc41".source | test("qubes-kernel-6.12.64-1.fc41")
' "$KERNELS_JSON" >/dev/null

jq -e '
  ."nixome-nixos-config-system".import == true and
  ."nixome-nixos-config-system".skipStart == true and
  (."nixome-nixos-config-system".source | test("nixome-nixos-config-system")) and
  (keys | length) == 1
' "$TEMPLATES_JSON" >/dev/null

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  .templates."nixome-nixos-config-system".skip_start == true and
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
  . == "nixome-config-vault-template"
' "$GUEST_VAULT_HOSTNAME_JSON" >/dev/null

jq -e '
  . == "nixome-config-staging-template"
' "$GUEST_STAGING_HOSTNAME_JSON" >/dev/null

jq -e '
  . == "nixome-config-validator-template"
' "$GUEST_VALIDATOR_HOSTNAME_JSON" >/dev/null

grep -F 'nixome-nixos-config-vault-root-image.drv' "$GUEST_VAULT_ROOT_IMAGE_DRV" >/dev/null
grep -F 'nixome-nixos-config-staging-root-image.drv' "$GUEST_STAGING_ROOT_IMAGE_DRV" >/dev/null
grep -F 'nixome-nixos-config-validator-root-image.drv' "$GUEST_VALIDATOR_ROOT_IMAGE_DRV" >/dev/null

jq -e '
  . == "0555"
' "$GUEST_VALIDATOR_RPC_MODE_JSON" >/dev/null

jq -e '
  . == "0555"
' "$GUEST_DEPLOYMENT_RPC_MODE_JSON" >/dev/null

jq -e '
  index("multi-user.target") != null
' "$SETUP_CONFIG_WANTED_BY_JSON" >/dev/null

jq -e '
  index("libvirtd.service") != null and
  index("qubes-db-dom0.service") != null and
  index("qubesd.service") != null and
  index("qubes-qrexec-policy-daemon.service") != null
' "$SETUP_CONFIG_AFTER_JSON" >/dev/null

jq -e '
  index("libvirtd.service") != null and
  index("qubes-db-dom0.service") != null and
  index("qubesd.service") != null and
  index("qubes-qrexec-policy-daemon.service") != null
' "$SETUP_CONFIG_WANTS_JSON" >/dev/null

jq -e '
  type == "string" and contains("nixome-setup-config") and endswith(" --apply")
' "$SETUP_CONFIG_EXECSTART_JSON" >/dev/null

jq -e '
  .NIXOME_LOG_DIR == "/var/log/nixome"
' "$SETUP_CONFIG_ENV_JSON" >/dev/null

jq -e '
  . == false
' "$SETUP_CONFIG_RESTART_IF_CHANGED_JSON" >/dev/null

jq -e '
  . == false
' "$SETUP_CONFIG_STOP_IF_CHANGED_JSON" >/dev/null

jq -e '
  index("qubes-qrexec-policy-daemon.service") != null
' "$QREXEC_POLICY_PATH_BEFORE_JSON" >/dev/null

jq -e '
  index("nixome-qubes-policy-path.service") != null
' "$QREXEC_POLICY_DAEMON_AFTER_JSON" >/dev/null

jq -e '
  index("nixome-qubes-policy-path.service") != null
' "$QREXEC_POLICY_DAEMON_REQUIRES_JSON" >/dev/null

jq -e '
  type == "array" and
  index("") != null and
  any(.[]; contains("qrexec-policy-daemon --policy-path /var/lib/nixome/qubes-policy.d"))
' "$QREXEC_POLICY_DAEMON_EXECSTART_JSON" >/dev/null

cat > "$WORK_DIR/expected-30-nixome-config.policy" <<'EOF'
# Managed by Nixome. Only dom0 may fetch a candidate from config-staging,
# ask config-validator to validate it, or archive it into config-vault.
nixome.Config.CandidateExport * dom0 config-staging allow
nixome.Config.CandidateExport * @anyvm @anyvm deny
nixome.Config.ValidateCandidate * dom0 config-validator allow
nixome.Config.ValidateCandidate * @anyvm @anyvm deny
nixome.Config.ArchiveCandidate * dom0 config-vault allow
nixome.Config.ArchiveCandidate * @anyvm @anyvm deny
nixome.Config.ArchiveDeployment * dom0 config-vault allow
nixome.Config.ArchiveDeployment * @anyvm @anyvm deny
EOF
cmp -s "$WORK_DIR/expected-30-nixome-config.policy" "$CONFIG_ACCESS_POLICY_FILE"

printf 'config topology eval ok\n'
