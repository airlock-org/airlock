#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-system-schema.XXXXXX")"
VALID_JSON="$LOG_DIR/system-schema-valid.json"
REBOOT_JSON="$LOG_DIR/system-schema-reboot.json"

trap 'rm -rf "$WORK_DIR"' EXIT

mkdir -p "$LOG_DIR" "$WORK_DIR/valid/nixome" "$WORK_DIR/reboot/nixome"
cp "$ROOT/lib/nixome/system.nix" "$WORK_DIR/valid/nixome/system-schema.nix"
cp "$ROOT/lib/nixome/system.nix" "$WORK_DIR/reboot/nixome/system-schema.nix"

cat > "$WORK_DIR/valid/flake.nix" <<'EOF'
{
  outputs =
    { self }:
    let
      systemSchema = import ./nixome/system-schema.nix;
    in
    {
      nixome.system = systemSchema.mkSystem {
        dom0 = {
          profile = {
            source = {
              build = "packages.x86_64-linux.nixome-dom0-profile";
            };
            activationCommand = "bin/nixome-dom0-profile-activate";
            message = "hello";
          };
          packages = [
            { name = "nixome-dom0-profile"; }
          ];
          systemd.services."nixome-example" = {
            action = "restart";
          };
        };
      };
    };
}
EOF

cat > "$WORK_DIR/reboot/flake.nix" <<'EOF'
{
  outputs =
    { self }:
    let
      systemSchema = import ./nixome/system-schema.nix;
    in
    {
      nixome.system = systemSchema.mkSystem {
        dom0 = {
          nixos = {
            system = {
              source = {
                build = "nixosConfigurations.dom0.config.system.build.toplevel";
              };
              switchAction = "switch";
              reason = "new dom0 system generation";
            };
            kernel.enable = true;
            xen.enable = true;
            bootloader.enable = true;
            qubes.enable = true;
          };
        };
      };
    };
}
EOF

nix eval --json "$WORK_DIR/valid#nixome.system" > "$VALID_JSON"
nix eval --json "$WORK_DIR/reboot#nixome.system" > "$REBOOT_JSON"

jq -e '
  .schema == "nixome.system.v1" and
  .dom0.profile.source_ref.attr == "packages.x86_64-linux.nixome-dom0-profile" and
  .dom0.profile.activation_command == "bin/nixome-dom0-profile-activate" and
  .dom0.profile.message == "hello" and
  .dom0.packages[0].name == "nixome-dom0-profile" and
  .dom0.systemd.services."nixome-example".action == "restart" and
  .plan.reboot_required == false and
  any(.plan.live_actions[]; .type == "dom0-profile" and .message == "hello") and
  any(.plan.live_actions[]; .type == "systemd-service" and .name == "nixome-example" and .action == "restart")
' "$VALID_JSON" >/dev/null

jq -e '
  .schema == "nixome.system.v1" and
  .dom0.nixos.system.source_ref.attr == "nixosConfigurations.dom0.config.system.build.toplevel" and
  .dom0.nixos.system.switch_action == "switch" and
  .dom0.nixos.system.enable == true and
  .dom0.nixos.kernel.enable == true and
  .dom0.nixos.xen.enable == true and
  .dom0.nixos.bootloader.enable == true and
  .dom0.nixos.qubes.enable == true and
  .plan.reboot_required == true and
  any(.plan.live_actions[]; .type == "dom0-nixos-system" and .action == "switch" and .reason == "new dom0 system generation") and
  (.plan.reboot_reasons | length) == 4 and
  any(.plan.reboot_reasons[]; . == "dom0 kernel change requires reboot") and
  any(.plan.reboot_reasons[]; . == "dom0 xen change requires reboot") and
  any(.plan.reboot_reasons[]; . == "dom0 bootloader change requires reboot") and
  any(.plan.reboot_reasons[]; . == "dom0 qubes change requires reboot")
' "$REBOOT_JSON" >/dev/null

printf 'system schema eval ok\n'
