#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nixome-minimal-guest"
RESULT_JSON="$RESULT_DIR/result.json"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

nix eval --impure --json --expr '
let
  flake = builtins.getFlake "path:'"$ROOT"'";
  lib = flake.inputs.nixpkgs.lib;
  cfgs = {
    staging = flake.nixosConfigurations."nixome-domain-config-staging".config;
    validator = flake.nixosConfigurations."nixome-domain-config-validator".config;
    vault = flake.nixosConfigurations."nixome-domain-config-vault".config;
  };
  cfgList = builtins.attrValues cfgs;
  packageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.systemPackages;
  corePackageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.corePackages;
  enabledServices =
    cfg:
    lib.attrNames (
      lib.filterAttrs (_name: service: service.enable or false) cfg.systemd.services
    );
  etcNames = cfg: lib.attrNames cfg.environment.etc;
  has = name: values: builtins.elem name values;
  allCfgs = pred: lib.all pred cfgList;
  servicesAbsent =
    names: cfg:
    lib.all (name: !(has name (enabledServices cfg))) names;
  packagesAbsent =
    names: cfg:
    lib.all (name: !(has name (packageNames cfg))) names;
  noSeedEtc =
    cfg:
    lib.all (name: !(lib.hasPrefix "nixome/config-seed-inputs/" name)) (etcNames cfg);
  genericServices = [
    "console-getty"
    "container-getty@"
    "getty@"
    "nscd"
    "post-boot"
    "prepare-kexec"
    "serial-getty@"
    "sleep-actions"
    "systemd-backlight"
    "systemd-backlight@"
    "systemd-hostnamed"
    "systemd-importd"
    "systemd-localed"
    "systemd-nspawn@"
    "systemd-oomd"
    "systemd-pstore"
    "systemd-timedated"
  ];
in
{
  package_names = lib.mapAttrs (_name: cfg: packageNames cfg) cfgs;
  core_package_names = lib.mapAttrs (_name: cfg: corePackageNames cfg) cfgs;
  enabled_services = lib.mapAttrs (_name: cfg: enabledServices cfg) cfgs;
  service_counts = lib.mapAttrs (_name: cfg: builtins.length (enabledServices cfg)) cfgs;

  minimal_enabled = allCfgs (cfg: cfg.nixome.minimalGuest.enable);
  default_packages_empty = allCfgs (cfg: cfg.environment.defaultPackages == [ ]);
  core_packages_trimmed = allCfgs (
    cfg:
    corePackageNames cfg == [
      "bash-interactive"
      "coreutils"
      "findutils"
      "gawk"
      "gnugrep"
      "gnused"
      "gnutar"
      "gzip"
      "kmod"
      "procps"
      "shadow"
      "systemd-minimal"
      "util-linux"
    ]
  );
  documentation_disabled = allCfgs (
    cfg:
    cfg.documentation.enable == false
    && cfg.documentation.doc.enable == false
    && cfg.documentation.info.enable == false
    && cfg.documentation.man.enable == false
    && cfg.documentation.nixos.enable == false
  );
  perlless_mechanics_enabled = allCfgs (
    cfg:
    cfg.system.etc.overlay.enable == true
    && cfg.services.userborn.enable == true
  );
  installer_tools_disabled = allCfgs (cfg: cfg.system.disableInstallerTools);
  suid_wrappers_disabled = allCfgs (
    cfg:
    cfg.security.wrappers == { }
    && ((cfg.systemd.services.suid-sgid-wrappers.enable or false) == false)
  );
  no_installer_tool_packages = allCfgs (
    packagesAbsent [
      "nixos-build-vms"
      "nixos-enter"
      "nixos-generate-config"
      "nixos-install"
      "nixos-option"
    ]
  );

  nix_module_disabled = allCfgs (cfg: cfg.nix.enable == false);
  no_nix_daemon_services = allCfgs (
    servicesAbsent [
      "nix-daemon"
      "nix-gc"
      "nix-optimise"
    ]
  );
  nix_cli_capability = {
    staging = cfgs.staging.nixome.minimalGuest.nix.enable;
    validator = cfgs.validator.nixome.minimalGuest.nix.enable;
    vault = cfgs.vault.nixome.minimalGuest.nix.enable;
  };
  staging_has_nix_cli = has "nix" (packageNames cfgs.staging) && has "nixome-config-nix" (packageNames cfgs.staging);
  validator_has_nix_cli = has "nix" (packageNames cfgs.validator) && has "nixome-config-nix" (packageNames cfgs.validator);
  vault_has_no_nix_cli = !(has "nix" (packageNames cfgs.vault)) && !(has "nixome-config-nix" (packageNames cfgs.vault));

  offline_networking_disabled = allCfgs (
    cfg:
    cfg.nixome.minimalGuest.networking.enable == false
    && cfg.networking.useDHCP == false
    && cfg.networking.dhcpcd.enable == false
    && cfg.networking.firewall.enable == false
    && cfg.networking.resolvconf.enable == false
  );

  no_guest_seed_input_etc = allCfgs noSeedEtc;
  required_qubes_services_retained = allCfgs (
    cfg:
    lib.all (name: has name (enabledServices cfg)) [
      "nixome-qubes-rw-init"
      "qubes-db"
      "qubes-qrexec-agent"
      "qubes-sysinit"
    ]
  );
  generic_services_disabled = allCfgs (servicesAbsent genericServices);

  seed_inputs_import_rpc = {
    staging = cfgs.staging.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
    validator = cfgs.validator.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
    vault = cfgs.vault.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
  };
  qrexec_domU_is_explicit = allCfgs (
    cfg:
    has "qubes-core-qrexec-domU" (packageNames cfg)
    && !(has "qubes-core-qrexec-base" (packageNames cfg))
  );
  broad_qubes_tools_not_in_system_packages = allCfgs (
    packagesAbsent [
      "qubes-core-agent-linux"
      "qubes-linux-utils"
    ]
  );
}
' > "$RESULT_JSON"

jq -e '
  .minimal_enabled == true and
  .default_packages_empty == true and
  .core_packages_trimmed == true and
  .documentation_disabled == true and
  .perlless_mechanics_enabled == true and
  .installer_tools_disabled == true and
  .suid_wrappers_disabled == true and
  .no_installer_tool_packages == true and
  .nix_module_disabled == true and
  .no_nix_daemon_services == true and
  .nix_cli_capability == {
    "staging": true,
    "validator": true,
    "vault": false
  } and
  .staging_has_nix_cli == true and
  .validator_has_nix_cli == true and
  .vault_has_no_nix_cli == true and
  .offline_networking_disabled == true and
  .no_guest_seed_input_etc == true and
  .required_qubes_services_retained == true and
  .generic_services_disabled == true and
  .seed_inputs_import_rpc == {
    "staging": true,
    "validator": true,
    "vault": false
  } and
  .qrexec_domU_is_explicit == true and
  .broad_qubes_tools_not_in_system_packages == true
' "$RESULT_JSON" >/dev/null

printf 'nixome minimal guest test ok\n'
