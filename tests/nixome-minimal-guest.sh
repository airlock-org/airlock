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
  packagesAbsent =
    names: cfg:
    lib.all (name: !(has name (packageNames cfg))) names;
  noSeedEtc =
    cfg:
    lib.all (name: !(lib.hasPrefix "nixome/config-seed-inputs/" name)) (etcNames cfg);
in
{
  package_names = lib.mapAttrs (_name: cfg: packageNames cfg) cfgs;
  core_package_names = lib.mapAttrs (_name: cfg: corePackageNames cfg) cfgs;
  enabled_services = lib.mapAttrs (_name: cfg: enabledServices cfg) cfgs;
  service_counts = lib.mapAttrs (_name: cfg: builtins.length (enabledServices cfg)) cfgs;

  minimal_enabled = allCfgs (cfg: cfg.nixome.minimalGuest.enable);
  default_packages_empty = allCfgs (cfg: cfg.environment.defaultPackages == [ ]);
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
  no_installer_tool_packages = allCfgs (
    packagesAbsent [
      "nixos-build-vms"
      "nixos-enter"
      "nixos-generate-config"
      "nixos-install"
      "nixos-option"
    ]
  );

  nix_cli_capability = {
    staging = cfgs.staging.nixome.minimalGuest.nix.enable;
    validator = cfgs.validator.nixome.minimalGuest.nix.enable;
    vault = cfgs.vault.nixome.minimalGuest.nix.enable;
  };
  staging_has_config_nix = has "nix" (packageNames cfgs.staging) && has "nixome-config-nix" (packageNames cfgs.staging);
  validator_has_config_nix = has "nix" (packageNames cfgs.validator) && has "nixome-config-nix" (packageNames cfgs.validator);
  vault_has_no_config_nix = !(has "nixome-config-nix" (packageNames cfgs.vault));

  stock_runtime_retained = allCfgs (
    cfg:
    (cfg.systemd.package.pname or "") == "systemd"
    && cfg.services.dbus.enable == true
    && cfg.networking.useDHCP == true
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

  seed_inputs_import_rpc = {
    staging = cfgs.staging.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
    validator = cfgs.validator.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
    vault = cfgs.vault.environment.etc ? "qubes-rpc/nixome.Config.SeedInputsImport";
  };
  qubes_runtime_packages_retained = allCfgs (
    cfg:
    has "qubes-core-agent-linux" (packageNames cfg)
    && has "qubes-linux-utils" (packageNames cfg)
    && has "qubes-core-qrexec-base" (packageNames cfg)
    && has "qubes-core-qrexec-domU" (packageNames cfg)
  );
}
' > "$RESULT_JSON"

jq -e '
  .minimal_enabled == true and
  .default_packages_empty == true and
  .documentation_disabled == true and
  .perlless_mechanics_enabled == true and
  .installer_tools_disabled == true and
  .no_installer_tool_packages == true and
  .nix_cli_capability == {
    "staging": true,
    "validator": true,
    "vault": false
  } and
  .staging_has_config_nix == true and
  .validator_has_config_nix == true and
  .vault_has_no_config_nix == true and
  .stock_runtime_retained == true and
  .no_guest_seed_input_etc == true and
  .required_qubes_services_retained == true and
  .seed_inputs_import_rpc == {
    "staging": true,
    "validator": true,
    "vault": false
  } and
  .qubes_runtime_packages_retained == true
' "$RESULT_JSON" >/dev/null

printf 'nixome minimal guest test ok\n'
