#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/config-candidate-flake"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-config-candidate-flake.XXXXXX")"
TOPOLOGY_JSON="$RESULT_DIR/topology.json"
LOCAL_TOPOLOGY_JSON="$RESULT_DIR/local-topology.json"
SYSTEM_JSON="$RESULT_DIR/system.json"
LOCAL_SYSTEM_JSON="$RESULT_DIR/local-system.json"
OVERRIDE_SYSTEM_JSON="$RESULT_DIR/override-system.json"
LOCAL_TEMPLATE_CONFIGURATION="$RESULT_DIR/local-template-configuration.txt"
PROFILE_LINK="$RESULT_DIR/dom0-profile"
PROFILE_JSON="$RESULT_DIR/dom0-profile.json"
HOST_MESSAGE="$RESULT_DIR/host-message.txt"
EXTRA_HOST_MESSAGE="$RESULT_DIR/extra-host-message.txt"
EXTRA_HOST_SERVICE="$RESULT_DIR/extra-host-service.txt"
EXTRA_HOST_CONFIGURATION="$RESULT_DIR/extra-host-configuration.txt"
EXTRA_HOST_PACKAGE="$RESULT_DIR/extra-host-package.json"
EXTRA_HOST_OVERLAY_PACKAGE="$RESULT_DIR/extra-host-overlay-package.json"
HOST_SYSTEM_DERIVATIONS="$RESULT_DIR/dom0-system-derivations.txt"
LOCAL_TEMPLATE_DERIVATION="$RESULT_DIR/local-template-derivation.txt"
CONFIG_VAULT_TEMPLATE_DERIVATION="$RESULT_DIR/config-vault-template-derivation.txt"
CONFIG_STAGING_TEMPLATE_DERIVATION="$RESULT_DIR/config-staging-template-derivation.txt"
CONFIG_VALIDATOR_TEMPLATE_DERIVATION="$RESULT_DIR/config-validator-template-derivation.txt"
NIX_NATIVE_HOSTNAME="$RESULT_DIR/nix-native-hostname.txt"
NIX_NATIVE_ETC="$RESULT_DIR/nix-native-etc.txt"
CONFIG_SHARED_GUEST_HOSTNAME="$RESULT_DIR/config-shared-guest-hostname.txt"
CONFIG_SHARED_GUEST_ETC="$RESULT_DIR/config-shared-guest-etc.txt"
CONFIG_VAULT_GUEST_ETC="$RESULT_DIR/config-vault-guest-etc.txt"
CONFIG_STAGING_GUEST_ETC="$RESULT_DIR/config-staging-guest-etc.txt"
CONFIG_STAGING_GUEST_CONFIGURATION="$RESULT_DIR/config-staging-guest-configuration.txt"
CONFIG_STAGING_GUEST_PACKAGE="$RESULT_DIR/config-staging-guest-package.json"
CONFIG_VALIDATOR_GUEST_HOSTNAME="$RESULT_DIR/config-validator-guest-hostname.txt"
CONFIG_VALIDATOR_GUEST_SERVICE="$RESULT_DIR/config-validator-guest-service.txt"
INVALID_TEMPLATE_LOG="$RESULT_DIR/invalid-template.log"
INVALID_CONFIG_VMS_LOG="$RESULT_DIR/invalid-config-vms.log"
INVALID_DOM0_HOST_APPLY_LOG="$RESULT_DIR/invalid-dom0-host-apply.log"
INVALID_DOM0_HOST_APPLY_REASON_LOG="$RESULT_DIR/invalid-dom0-host-apply-reason.log"
INVALID_DOM0_HOST_LOG="$RESULT_DIR/invalid-dom0-host.log"
INVALID_TOP_LEVEL_LOG="$RESULT_DIR/invalid-top-level.log"

trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"
mkdir -p "$WORK_DIR/nixome"
cp -R "$ROOT/seeds/config-staging/dom0-profile" "$WORK_DIR/dom0-profile"
cp "$ROOT/seeds/config-staging/flake.nix" "$WORK_DIR/flake.nix"
cp "$ROOT/seeds/config-staging/nixome/config-vms.nix" "$WORK_DIR/nixome/config-vms.nix"
cp "$ROOT/seeds/config-staging/nixome/dom0-host.nix" "$WORK_DIR/nixome/dom0-host.nix"
cp "$ROOT/seeds/config-staging/nixome/local.nix" "$WORK_DIR/nixome/local.nix"
cp "$ROOT/seeds/config-staging/nixome/machine.nix" "$WORK_DIR/nixome/machine.nix"
cp "$ROOT/seeds/config-staging/nixome/prebuilt-template-sources.nix" "$WORK_DIR/nixome/prebuilt-template-sources.nix"
cp "$ROOT/seeds/config-staging/nixome/system.nix" "$WORK_DIR/nixome/system.nix"
cp "$ROOT/seeds/config-staging/nixome/templates.nix" "$WORK_DIR/nixome/templates.nix"
cp "$ROOT/lib/nixome/system.nix" "$WORK_DIR/nixome/system-schema.nix"
cp "$ROOT/lib/nixome/topology.nix" "$WORK_DIR/nixome/topology.nix"

nixpkgs_path="$(nixome_physical_store_path "$(nix eval --raw "$ROOT#nixome.seedInputs.nixpkgs")")"
nixpkgs_libvirt_10_5_path="$(
  nixome_physical_store_path "$(nix eval --raw "$ROOT#nixome.seedInputs.nixpkgsLibvirt105")"
)"
nixos_qubes_path="$(nixome_physical_store_path "$(nix eval --raw "$ROOT#nixome.seedInputs.nixosQubes")")"
nixome_support_path="$(nixome_physical_store_path "$(nix eval --raw "$ROOT#nixome.seedInputs.nixomeSupport")")"
sed -i \
  -e "s|__NIXOME_NIXPKGS_URL__|path:$nixpkgs_path|g" \
  -e "s|__NIXPKGS_LIBVIRT_10_5_PATH__|$nixpkgs_libvirt_10_5_path|g" \
  -e "s|__NIXOME_NIXOS_QUBES_PATH__|$nixos_qubes_path|g" \
  -e "s|__NIXOME_SUPPORT_PATH__|$nixome_support_path|g" \
  "$WORK_DIR/flake.nix"

extracted_nixpkgs_url="$(
  sed -n 's/^[[:space:]]*\(inputs\.\)\{0,1\}nixpkgs\.url = "\(.*\)";$/\2/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
extracted_nixome_support_path="$(
  sed -n 's/^      nixomeSupport = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
extracted_nixpkgs_libvirt_10_5_path="$(
  sed -n 's/^      nixpkgsLibvirt105Source = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
extracted_nixos_qubes_path="$(
  sed -n 's/^      nixosQubesSource = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
[ "$extracted_nixpkgs_url" = "path:$nixpkgs_path" ]
[ "$extracted_nixome_support_path" = "$nixome_support_path" ]
[ "$extracted_nixpkgs_libvirt_10_5_path" = "$nixpkgs_libvirt_10_5_path" ]
[ "$extracted_nixos_qubes_path" = "$nixos_qubes_path" ]

(
  cd "$WORK_DIR"
  nix \
    --extra-experimental-features nix-command \
    --extra-experimental-features flakes \
    flake lock \
    --impure \
    --offline
)
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  flake check \
  --impure \
  --offline \
  --no-write-lock-file \
  "$WORK_DIR" --no-build --show-trace
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.topology" > "$TOPOLOGY_JSON"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.system" > "$SYSTEM_JSON"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.etc.\"nixome/inside-config-applied-message\".text" \
  > "$HOST_MESSAGE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  path-info \
  --impure \
  --offline \
  --derivation \
  --recursive \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.system.build.toplevel" \
  > "$HOST_SYSTEM_DERIVATIONS"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  build \
  --impure \
  --offline \
  --no-write-lock-file \
  --out-link "$PROFILE_LINK" \
  "$WORK_DIR#packages.x86_64-linux.nixome-dom0-profile"
profile_out="$(readlink "$PROFILE_LINK")"
profile_physical="$(nixome_physical_store_path "$profile_out")"
[ -x "$profile_physical/bin/nixome-dom0-profile-activate" ]
cp "$profile_physical/share/nixome/profile.json" "$PROFILE_JSON"

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  .templates."nixome-nixos-work".import == true and
  .templates."nixome-nixos-work".source == null and
  .templates."nixome-nixos-work".source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nixome-nixos-work-template",
    "subpath": "nixome-nixos-work"
  } and
  .kernels == {} and
  .prefs.default_kernel == "" and
  (.domains | type == "object") and
  .domains."config-vault".class == "AppVM" and
  .domains."config-vault".template == "nixome-nixos-config-system" and
  .domains."config-vault".backend.type == "qubes-template" and
  .domains."config-vault".backend.template == "nixome-nixos-config-system" and
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
  .domains."config-staging".template == "nixome-nixos-config-system" and
  .domains."config-staging".backend.type == "qubes-template" and
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
  .domains."config-validator".template == "nixome-nixos-config-system" and
  .domains."config-validator".backend.type == "qubes-template" and
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
  .domains."nixome-nixos-work".class == "TemplateVM" and
  .domains."nixome-nixos-work".backend.type == "qubes-template-root" and
  .domains."nixome-nixos-work".backend.template == null and
  .domains."nixome-nixos-work".label == "black" and
  .domains."nixome-nixos-work".netvm == null and
  .domains."nixome-nixos-work".prefs.default_user == "user" and
  .domains."nixome-nixos-work".prefs.kernel == "6.12.64-1.fc41" and
  (.domains."nixome-nixos-work".prefs.kernelopts | contains("init=/sbin/init")) and
  .domains."nixome-nixos-work".features."nixome-role" == "work-template" and
  .domains."nixome-nixos-work".features."nixome-template-source" == "staged-config" and
  .domains."nixome-work".class == "AppVM" and
  .domains."nixome-work".template == "nixome-nixos-work" and
  .domains."nixome-work".backend.type == "qubes-template" and
  .domains."nixome-work".backend.template == "nixome-nixos-work" and
  .domains."nixome-work".label == "green" and
  .domains."nixome-work".netvm == null and
  .domains."nixome-work".prefs.include_in_backups == true and
  .domains."nixome-work".features."nixome-role" == "work" and
  .domains."nixome-work".features."nixome-template-source" == "staged-config" and
  .domains."nixome-work".services."network-manager" == false and
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

jq -e '
  .schema == "nixome.system.v1" and
  .dom0.profile.source == "dom0-profile" and
  .dom0.profile.source_ref == null and
  .dom0.profile.activation_command == "bin/nixome-dom0-profile-activate" and
  .dom0.profile.activation_action == "run" and
  .dom0.profile.message == "initial" and
  .dom0.nixos.system.enable == true and
  .dom0.nixos.system.source == null and
  .dom0.nixos.system.source_ref == {
    "type": "flake-output",
    "attr": "nixosConfigurations.nixome-dom0.config.system.build.toplevel",
    "subpath": ""
  } and
  .dom0.nixos.system.switch_action == "switch" and
  .dom0.nixos.system.reason == "dom0 host userland config can be applied live" and
  .dom0.packages == [
    {
      "name": "nixome-dom0-profile"
    }
  ] and
  .dom0.systemd.services == {} and
  .dom0.nixos.kernel.enable == false and
  .dom0.nixos.xen.enable == false and
  .dom0.nixos.bootloader.enable == false and
  .dom0.nixos.qubes.enable == false and
  any(.plan.live_actions[]; .type == "dom0-profile" and .message == "initial") and
  any(.plan.live_actions[]; .type == "dom0-nixos-system" and .action == "switch" and .source_ref.attr == "nixosConfigurations.nixome-dom0.config.system.build.toplevel") and
  .plan.reboot_actions == [] and
  .plan.reboot_required == false and
  .plan.reboot_reasons == []
' "$SYSTEM_JSON" >/dev/null

jq -e '
  .schema == "nixome.dom0-profile.v1" and
  .message == "initial" and
  .built_by == "config-staging"
' "$PROFILE_JSON" >/dev/null
grep -qx initial "$HOST_MESSAGE"
if grep -E 'nixome-nixos-(config-system|work)-root-image|qubes-template-nixome-nixos-(config-system|work)' \
  "$HOST_SYSTEM_DERIVATIONS" >/dev/null; then
  printf 'staged dom0 system unexpectedly depends on Qube template image derivations\n' >&2
  cat "$HOST_SYSTEM_DERIVATIONS" >&2
  exit 1
fi

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{
  appliedMessage,
  pkgs,
  system,
  templateSet,
  ...
}:
{
  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [ ];
  };

  domains = {
    legacy-fedora-app = {
      backend.type = "qubes-template";
      template = "fedora-44-xfce";
      label = "red";
      netvm = null;
      features."nixome-role" = "legacy-fedora-app";
    };

    nix-native-work = {
      backend = {
        type = "nix-closure";
        system.source = "dom0-profile";
        kernel.name = "6.12.64-1.fc41";
        root.source = "dom0-profile";
        store.source = "dom0-profile";
      };
      label = "green";
      prefs.memory = 1536;
      features."nixome-role" = "nix-native";
    };
  };

  nixNative.domains.nix-native-generated = {
    label = "purple";
    netvm = null;
    hostName = "nix-native-generated";
    packages = with pkgs; [
      jq
    ];
    etc."nixome/nix-native-role".text = "generated\n";
    prefs.memory = 1280;
    features."nixome-role" = "nix-native-generated";
  };

  templates.definitions.nixome-nixos-dev = {
    role = "dev-template";
    sourceLabel = "local-config";
    hostName = "nixome-dev-template";
    etc."nixome/template-role".text = "dev\n";
    packages = with pkgs; [
      git
      jq
      ripgrep
    ];
    configuration = {
      environment.etc."nixome/template-configuration".text = "dev-configuration\n";
    };
    domain.features."nixome-dev-template" = "1";
    appVms.nixome-dev = {
      label = "blue";
      prefs = {
        memory = 1536;
        maxmem = 3072;
      };
      features = {
        "nixome-role" = "dev";
      };
    };
  };

  configVms = {
    templateMode = "dedicated";
    sharedGuest = {
      hostName = "nixome-config-system-custom";
      etc."nixome/config-role-content".text = "shared\n";
    };
    guests = {
      config-vault = {
        etc."nixome/config-role-content".text = "vault\n";
      };
      config-staging = {
        packages = with pkgs; [
          ripgrep
        ];
        etc."nixome/config-role-content".text = "staging\n";
        configuration = {
          environment.etc."nixome/config-role-configuration".text = "staging-configuration\n";
        };
      };
      config-validator = {
        hostName = "nixome-validator-custom";
        systemd.services.nixome-validator-fixture = {
          description = "Nixome validator role fixture";
          serviceConfig.Type = "oneshot";
          script = "${pkgs.coreutils}/bin/true";
        };
      };
    };
    domains = {
      config-vault.prefs.memory = 1536;
      config-staging.label = "yellow";
    };
  };

  dom0.host = {
    overlays = [
      (final: prev: {
        nixomeOverlayProbe = final.writeText "nixome-overlay-probe" "overlay\n";
      })
    ];
    packages = with pkgs; [
      hello
    ];
    etc."nixome/local-dom0-extra-module".text = "${appliedMessage}-extra\n";
    systemd.services.nixome-local-extra-module = {
      description = "Nixome local extra module fixture";
      serviceConfig.Type = "oneshot";
      script = "${pkgs.coreutils}/bin/true";
    };
    configuration = {
      environment.etc."nixome/local-dom0-host-configuration".text = "${appliedMessage}-configuration\n";
    };
    modules = [
      ({ pkgs, ... }: {
        environment.systemPackages = [
          pkgs.nixomeOverlayProbe
        ];
      })
    ];
  };

  system.args = { };
}
EOF

nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.etc.\"nixome/local-dom0-extra-module\".text" \
  > "$EXTRA_HOST_MESSAGE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.systemd.services.nixome-local-extra-module.description" \
  > "$EXTRA_HOST_SERVICE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.etc.\"nixome/local-dom0-host-configuration\".text" \
  > "$EXTRA_HOST_CONFIGURATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json \
  --apply 'packages: builtins.any (package: (package.pname or package.name or "") == "hello") packages' \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.systemPackages" \
  > "$EXTRA_HOST_PACKAGE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json \
  --apply 'packages: builtins.any (package: (package.pname or package.name or "") == "nixome-overlay-probe") packages' \
  "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.systemPackages" \
  > "$EXTRA_HOST_OVERLAY_PACKAGE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.topology" > "$LOCAL_TOPOLOGY_JSON"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.system" > "$LOCAL_SYSTEM_JSON"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.templateConfigurations.nixome-nixos-dev.config.environment.etc.\"nixome/template-configuration\".text" \
  > "$LOCAL_TEMPLATE_CONFIGURATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  path-info \
  --impure \
  --offline \
  --derivation \
  "$WORK_DIR#packages.x86_64-linux.nixome-nixos-dev-template" \
  > "$LOCAL_TEMPLATE_DERIVATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  path-info \
  --impure \
  --offline \
  --derivation \
  "$WORK_DIR#packages.x86_64-linux.nixome-nixos-config-vault-template" \
  > "$CONFIG_VAULT_TEMPLATE_DERIVATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  path-info \
  --impure \
  --offline \
  --derivation \
  "$WORK_DIR#packages.x86_64-linux.nixome-nixos-config-staging-template" \
  > "$CONFIG_STAGING_TEMPLATE_DERIVATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  path-info \
  --impure \
  --offline \
  --derivation \
  "$WORK_DIR#packages.x86_64-linux.nixome-nixos-config-validator-template" \
  > "$CONFIG_VALIDATOR_TEMPLATE_DERIVATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-domain-nix-native-generated.config.networking.hostName" \
  > "$NIX_NATIVE_HOSTNAME"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-domain-nix-native-generated.config.environment.etc.\"nixome/nix-native-role\".text" \
  > "$NIX_NATIVE_ETC"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configSystemTemplateConfiguration.config.networking.hostName" \
  > "$CONFIG_SHARED_GUEST_HOSTNAME"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configSystemTemplateConfiguration.config.environment.etc.\"nixome/config-role-content\".text" \
  > "$CONFIG_SHARED_GUEST_ETC"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-vault.config.environment.etc.\"nixome/config-role-content\".text" \
  > "$CONFIG_VAULT_GUEST_ETC"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-staging.config.environment.etc.\"nixome/config-role-content\".text" \
  > "$CONFIG_STAGING_GUEST_ETC"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-staging.config.environment.etc.\"nixome/config-role-configuration\".text" \
  > "$CONFIG_STAGING_GUEST_CONFIGURATION"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json \
  --apply 'packages: builtins.any (package: (package.pname or package.name or "") == "ripgrep") packages' \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-staging.config.environment.systemPackages" \
  > "$CONFIG_STAGING_GUEST_PACKAGE"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-validator.config.networking.hostName" \
  > "$CONFIG_VALIDATOR_GUEST_HOSTNAME"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-validator.config.systemd.services.nixome-validator-fixture.description" \
  > "$CONFIG_VALIDATOR_GUEST_SERVICE"

grep -qx initial-extra "$EXTRA_HOST_MESSAGE"
grep -qx "Nixome local extra module fixture" "$EXTRA_HOST_SERVICE"
grep -qx initial-configuration "$EXTRA_HOST_CONFIGURATION"
jq -e '. == true' "$EXTRA_HOST_PACKAGE" >/dev/null
jq -e '. == true' "$EXTRA_HOST_OVERLAY_PACKAGE" >/dev/null
grep -qx dev-configuration "$LOCAL_TEMPLATE_CONFIGURATION"
[ -s "$LOCAL_TEMPLATE_DERIVATION" ]
[ -s "$CONFIG_VAULT_TEMPLATE_DERIVATION" ]
[ -s "$CONFIG_STAGING_TEMPLATE_DERIVATION" ]
[ -s "$CONFIG_VALIDATOR_TEMPLATE_DERIVATION" ]
grep -qx nix-native-generated "$NIX_NATIVE_HOSTNAME"
grep -qx generated "$NIX_NATIVE_ETC"
grep -qx nixome-config-system-custom "$CONFIG_SHARED_GUEST_HOSTNAME"
grep -qx shared "$CONFIG_SHARED_GUEST_ETC"
grep -qx vault "$CONFIG_VAULT_GUEST_ETC"
grep -qx staging "$CONFIG_STAGING_GUEST_ETC"
grep -qx staging-configuration "$CONFIG_STAGING_GUEST_CONFIGURATION"
jq -e '. == true' "$CONFIG_STAGING_GUEST_PACKAGE" >/dev/null
grep -qx nixome-validator-custom "$CONFIG_VALIDATOR_GUEST_HOSTNAME"
grep -qx "Nixome validator role fixture" "$CONFIG_VALIDATOR_GUEST_SERVICE"
jq -e '
  .schema == "nixome.qubes-topology.v1" and
  .templates."nixome-nixos-config-vault".import == true and
  .templates."nixome-nixos-config-vault".source == null and
  .templates."nixome-nixos-config-vault".source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nixome-nixos-config-vault-template",
    "subpath": "nixome-nixos-config-vault"
  } and
  .templates."nixome-nixos-config-staging".import == true and
  .templates."nixome-nixos-config-staging".source_ref.attr == "packages.x86_64-linux.nixome-nixos-config-staging-template" and
  .templates."nixome-nixos-config-staging".source_ref.subpath == "nixome-nixos-config-staging" and
  .templates."nixome-nixos-config-validator".import == true and
  .templates."nixome-nixos-config-validator".source_ref.attr == "packages.x86_64-linux.nixome-nixos-config-validator-template" and
  .templates."nixome-nixos-config-validator".source_ref.subpath == "nixome-nixos-config-validator" and
  .templates."nixome-nixos-dev".import == true and
  .templates."nixome-nixos-dev".source == null and
  .templates."nixome-nixos-dev".source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nixome-nixos-dev-template",
    "subpath": "nixome-nixos-dev"
  } and
  .domains."nixome-nixos-config-vault".class == "TemplateVM" and
  .domains."nixome-nixos-config-vault".label == "black" and
  .domains."nixome-nixos-config-vault".netvm == null and
  .domains."nixome-nixos-config-vault".prefs.default_user == "user" and
  .domains."nixome-nixos-config-vault".prefs.kernel == "6.12.64-1.fc41" and
  (.domains."nixome-nixos-config-vault".prefs.kernelopts | contains("init=/sbin/init")) and
  .domains."nixome-nixos-config-vault".features."nixome-role" == "config-vault-template" and
  .domains."nixome-nixos-config-staging".class == "TemplateVM" and
  .domains."nixome-nixos-config-staging".label == "gray" and
  .domains."nixome-nixos-config-staging".features."nixome-role" == "config-staging-template" and
  .domains."nixome-nixos-config-validator".class == "TemplateVM" and
  .domains."nixome-nixos-config-validator".label == "blue" and
  .domains."nixome-nixos-config-validator".features."nixome-role" == "config-validator-template" and
  .domains."config-vault".template == "nixome-nixos-config-vault" and
  .domains."config-vault".prefs.memory == 1536 and
  .domains."config-staging".template == "nixome-nixos-config-staging" and
  .domains."config-staging".label == "yellow" and
  .domains."config-validator".template == "nixome-nixos-config-validator" and
  .domains."nixome-nixos-dev".class == "TemplateVM" and
  .domains."nixome-nixos-dev".label == "black" and
  .domains."nixome-nixos-dev".netvm == null and
  .domains."nixome-nixos-dev".prefs.default_user == "user" and
  .domains."nixome-nixos-dev".features."nixome-role" == "dev-template" and
  .domains."nixome-nixos-dev".features."nixome-template-source" == "local-config" and
  .domains."nixome-nixos-dev".features."nixome-dev-template" == "1" and
  .domains."nixome-dev".class == "AppVM" and
  .domains."nixome-dev".template == "nixome-nixos-dev" and
  .domains."nixome-dev".label == "blue" and
  .domains."nixome-dev".prefs.memory == 1536 and
  .domains."nixome-dev".prefs.maxmem == 3072 and
  .domains."nixome-dev".features."nixome-role" == "dev" and
  .domains."nixome-dev".features."nixome-template-source" == "local-config" and
  .domains."nixome-dev".services."network-manager" == false and
  .domains."legacy-fedora-app".class == "AppVM" and
  .domains."legacy-fedora-app".template == "fedora-44-xfce" and
  .domains."legacy-fedora-app".backend.type == "qubes-template" and
  .domains."legacy-fedora-app".backend.template == "fedora-44-xfce" and
  .domains."legacy-fedora-app".features."nixome-role" == "legacy-fedora-app" and
  .domains."nix-native-work".class == "StandaloneVM" and
  .domains."nix-native-work".template == null and
  .domains."nix-native-work".label == "green" and
  .domains."nix-native-work".prefs.memory == 1536 and
  .domains."nix-native-work".backend.type == "nix-closure" and
  .domains."nix-native-work".backend.system.source == "dom0-profile" and
  .domains."nix-native-work".backend.kernel.name == "6.12.64-1.fc41" and
  .domains."nix-native-work".backend.root.source == "dom0-profile" and
  .domains."nix-native-work".backend.root.mode == "qubes-root-image" and
  .domains."nix-native-work".backend.store.source == "dom0-profile" and
  .domains."nix-native-work".backend.store.mode == "closure-image" and
  .domains."nix-native-work".backend.store.read_only == true and
  .domains."nix-native-generated".class == "StandaloneVM" and
  .domains."nix-native-generated".template == null and
  .domains."nix-native-generated".label == "purple" and
  .domains."nix-native-generated".netvm == null and
  .domains."nix-native-generated".prefs.memory == 1280 and
  .domains."nix-native-generated".prefs.kernel == "6.12.64-1.fc41" and
  (.domains."nix-native-generated".prefs.kernelopts | contains("init=/sbin/init")) and
  .domains."nix-native-generated".features."nixome-backend" == "nix-closure" and
  .domains."nix-native-generated".features."nixome-role" == "nix-native-generated" and
  .domains."nix-native-generated".backend.type == "nix-closure" and
  .domains."nix-native-generated".backend.system.source_ref.attr == "nixosConfigurations.nixome-domain-nix-native-generated.config.system.build.toplevel" and
  .domains."nix-native-generated".backend.root.source_ref.attr == "packages.x86_64-linux.nixome-domain-nix-native-generated-root-image" and
  .domains."nix-native-generated".backend.root.source_ref.subpath == "root.img" and
  .domains."nix-native-generated".backend.store.source_ref.attr == "packages.x86_64-linux.nixome-domain-nix-native-generated-store-image" and
  .domains."nix-native-generated".backend.store.source_ref.subpath == "store.img"
' "$LOCAL_TOPOLOGY_JSON" >/dev/null

jq -e '
  .schema == "nixome.system.v1" and
  .dom0.nixos.system.switch_action == "boot" and
  .dom0.nixos.system.reason == "dom0 host config includes arbitrary NixOS modules or overlays" and
  .dom0.systemd.services == {} and
  any(.plan.live_actions[]; .type == "dom0-nixos-system" and .action == "boot") and
  .plan.reboot_required == true and
  (.plan.reboot_actions | length) == 1 and
  .plan.reboot_actions[0].type == "dom0-nixos-system" and
  .plan.reboot_actions[0].switch_action == "boot" and
  .plan.reboot_actions[0].reason == "dom0 host config includes arbitrary NixOS modules or overlays" and
  .plan.reboot_reasons == ["dom0 host config includes arbitrary NixOS modules or overlays"]
' "$LOCAL_SYSTEM_JSON" >/dev/null

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  dom0.host = {
    apply = {
      mode = "switch";
      reason = "operator classified host module safe for live switch";
    };
    modules = [
      ({ ... }: {
        environment.etc."nixome/manual-live-host-module".text = "manual-live\n";
      })
    ];
  };
}
EOF

nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.system" > "$OVERRIDE_SYSTEM_JSON"
jq -e '
  .schema == "nixome.system.v1" and
  .dom0.nixos.system.switch_action == "switch" and
  .dom0.nixos.system.reason == "operator classified host module safe for live switch" and
  any(.plan.live_actions[]; .type == "dom0-nixos-system" and .action == "switch") and
  .plan.reboot_required == false and
  .plan.reboot_actions == [] and
  .plan.reboot_reasons == []
' "$OVERRIDE_SYSTEM_JSON" >/dev/null

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  templates.definitions.nixome-nixos-bad = {
    packagesTypo = [ ];
  };
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.topology" > /dev/null 2> "$INVALID_TEMPLATE_LOG"; then
  printf 'invalid template definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.templates.definitions.nixome-nixos-bad: unknown option(s): packagesTypo' \
  "$INVALID_TEMPLATE_LOG"

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  configVms.guests.config-stagin = { };
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw "$WORK_DIR#nixome.configRoleTemplateConfigurations.config-staging.config.networking.hostName" > /dev/null 2> "$INVALID_CONFIG_VMS_LOG"; then
  printf 'invalid configVms guests definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.configVms.guests: unknown role(s): config-stagin' \
  "$INVALID_CONFIG_VMS_LOG"

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  dom0.host.apply.mode = "live";
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.system" > /dev/null 2> "$INVALID_DOM0_HOST_APPLY_LOG"; then
  printf 'invalid dom0.host.apply definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.dom0.host.apply.mode: expected one of auto, switch, boot' \
  "$INVALID_DOM0_HOST_APPLY_LOG"

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  dom0.host.apply.reason = true;
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.system" > /dev/null 2> "$INVALID_DOM0_HOST_APPLY_REASON_LOG"; then
  printf 'invalid dom0.host.apply reason unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.dom0.host.apply.reason: expected a string or path, got bool' \
  "$INVALID_DOM0_HOST_APPLY_REASON_LOG"

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  dom0.host.pakages = [ ];
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixosConfigurations.nixome-dom0.config.environment.systemPackages" > /dev/null 2> "$INVALID_DOM0_HOST_LOG"; then
  printf 'invalid dom0.host definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.dom0.host: unknown option(s): pakages' \
  "$INVALID_DOM0_HOST_LOG"

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  templats.definitions = { };
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.topology" > /dev/null 2> "$INVALID_TOP_LEVEL_LOG"; then
  printf 'invalid top-level machine definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.machine: unknown option(s): templats' \
  "$INVALID_TOP_LEVEL_LOG"

printf 'config candidate flake eval ok\n'
