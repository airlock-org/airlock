#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/dev-role"
ROLE_JSON="$RESULT_DIR/role.json"
CODEX_VERSION="$RESULT_DIR/codex-version.txt"
CODEX_OUT="$RESULT_DIR/codex-out.txt"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

nix eval --impure --json --expr '
let
  flake = builtins.getFlake "path:'"$ROOT"'";
  lib = flake.inputs.nixpkgs.lib;
  devCfg = flake.nixosConfigurations.nixome-domain-dev.config;
  builders = import '"$ROOT"'/lib/nixome/qube-template.nix {
    nixpkgs = flake.inputs.nixpkgs;
    nixpkgsLibvirt105Source = "${flake.inputs.nixpkgs-libvirt_10_5}";
    nixosQubesSource = "${flake.inputs.nixos-qubes}";
    nixomeSupport = "'"$ROOT"'";
    system = "x86_64-linux";
  };
  baseCfg = (builders.mkNixNativeDomain {
    name = "minimal-base";
    guestRole = "base";
    modules = [ ];
  }).configuration.config;
  packageNames = cfg: map (package: package.pname or package.name or "") cfg.environment.systemPackages;
  devPackages = packageNames devCfg;
  basePackages = packageNames baseCfg;
  has = name: values: builtins.elem name values;
  hasAnyPrefix = prefix: values: lib.any (value: lib.hasPrefix prefix value) values;
  etcNames = cfg: lib.attrNames cfg.environment.etc;
  forbiddenConfigRpc = [
    "qubes-rpc/nixome.Config.ArchiveCandidate"
    "qubes-rpc/nixome.Config.ArchiveDeployment"
    "qubes-rpc/nixome.Config.CandidateExport"
    "qubes-rpc/nixome.Config.SeedInputsImport"
    "qubes-rpc/nixome.Config.ValidateCandidate"
    "qubes-rpc/nixome.PolicyProbe"
  ];
  forbiddenDevPackages = [
    "cargo"
    "clippy"
    "codex"
    "git"
    "nixome"
    "nixome-config-nix"
    "rustc"
    "rustfmt"
    "uv"
  ];
  secretEnvNames = [
    "CODEX_AUTH_TOKEN"
    "OPENAI_API_KEY"
    "OPENAI_API_KEY_FILE"
  ];
  devServiceScript = devCfg.systemd.services.nixome-dev-guest-init.script;
in
{
  role = devCfg.nixome.qubeGuest.role;
  hostname = devCfg.networking.hostName;
  package_names = devPackages;
  required_tools_present = lib.all (name: has name devPackages) [
    "cargo"
    "clippy"
    "codex"
    "git"
    "gnumake"
    "jq"
    "less"
    "nix"
    "nixfmt"
    "nixome"
    "nixome-config-nix"
    "openssh"
    "ripgrep"
    "rustc"
    "rustfmt"
    "uv"
    "vim"
  ] && hasAnyPrefix "python3" devPackages;
  private_nix_enabled = devCfg.nixome.minimalGuest.nix.enable;
  nix_features = devCfg.nix.settings.experimental-features;
  nix_allowed_users = devCfg.nix.settings.allowed-users;
  codex_home = devCfg.nixome.qubeGuest.codexHome;
  codex_home_env = devCfg.environment.sessionVariables.CODEX_HOME or null;
  dev_persistent_paths = devCfg.nixome.qubeGuest.devPersistentPaths;
  dev_init_contains_dev_path = lib.hasInfix "/home/user/dev" devServiceScript;
  dev_init_contains_codex_home = lib.hasInfix "/home/user/.codex" devServiceScript;
  dev_init_contains_private_nix_root = lib.hasInfix "/rw/home/user/nixome-config-nix-root" devServiceScript;
  dev_qrexec_requires = devCfg.systemd.services.qubes-qrexec-agent.requires;
  dev_qrexec_after = devCfg.systemd.services.qubes-qrexec-agent.after;
  dev_forbidden_rpc_absent = lib.all (name: !(has name (etcNames devCfg))) forbiddenConfigRpc;
  dev_allowed_submitters = flake.nixosConfigurations.vm-gui-config.config.nixome.qubes.configAccessPolicy.allowedSubmitters;
  dev_no_secret_env = lib.all (name: !(builtins.hasAttr name devCfg.environment.sessionVariables)) secretEnvNames;
  dev_no_codex_auth_etc = lib.all (
    name:
    !(lib.hasInfix "auth.json" name)
    && !(lib.hasInfix "OPENAI_API_KEY" name)
    && !(lib.hasInfix ".codex" name)
  ) (etcNames devCfg);
  base_role = baseCfg.nixome.qubeGuest.role;
  base_private_nix_enabled = baseCfg.nixome.minimalGuest.nix.enable;
  base_dev_packages_absent = lib.all (name: !(has name basePackages)) forbiddenDevPackages;
  base_forbidden_rpc_absent = lib.all (name: !(has name (etcNames baseCfg))) forbiddenConfigRpc;
}
' > "$ROLE_JSON"

jq -e '
  .role == "dev" and
  .hostname == "dev" and
  .required_tools_present == true and
  .private_nix_enabled == true and
  (.nix_features | index("nix-command") != null) and
  (.nix_features | index("flakes") != null) and
  .nix_allowed_users == ["user"] and
  .codex_home == "/home/user/.codex" and
  .codex_home_env == "/home/user/.codex" and
  (.dev_persistent_paths | index("/home/user/dev") != null) and
  .dev_init_contains_dev_path == true and
  .dev_init_contains_codex_home == true and
  .dev_init_contains_private_nix_root == true and
  (.dev_qrexec_requires | index("nixome-dev-guest-init.service") != null) and
  (.dev_qrexec_after | index("nixome-dev-guest-init.service") != null) and
  .dev_forbidden_rpc_absent == true and
  .dev_allowed_submitters == ["dev"] and
  .dev_no_secret_env == true and
  .dev_no_codex_auth_etc == true and
  .base_role == "base" and
  .base_private_nix_enabled == false and
  .base_dev_packages_absent == true and
  .base_forbidden_rpc_absent == true
' "$ROLE_JSON" >/dev/null

nix build "$ROOT#packages.x86_64-linux.codex" --no-link --print-build-logs --print-out-paths > "$CODEX_OUT"
codex_path="$(tail -n 1 "$CODEX_OUT")"
nixome_run_store_path "$codex_path/bin/codex" --version > "$CODEX_VERSION"
grep -Eq '^codex-cli [0-9]+\.[0-9]+\.[0-9]+$' "$CODEX_VERSION"
if find "$(nixome_physical_store_path "$codex_path")" -name auth.json -print -quit | grep -q .; then
  printf 'codex package output unexpectedly contains auth.json\n' >&2
  exit 1
fi

printf 'dev role test ok\n'
