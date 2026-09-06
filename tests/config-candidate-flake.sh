#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
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
cp "$ROOT/seeds/config-staging/nixome/prebuilt-template-sources.nix" "$WORK_DIR/nixome/prebuilt-template-sources.nix"
cp "$ROOT/seeds/config-staging/nixome/templates.nix" "$WORK_DIR/nixome/templates.nix"
cp "$ROOT/lib/nixome/topology.nix" "$WORK_DIR/nixome/topology.nix"

nixpkgs_path="$(nix eval --raw "$ROOT#nixome.seedInputs.nixpkgs")"
nixos_qubes_path="$(nix eval --raw "$ROOT#nixome.seedInputs.nixosQubes")"
nixome_support_path="$(nix eval --raw "$ROOT#nixome.seedInputs.nixomeSupport")"
sed -i \
  -e "s|__NIXOME_NIXPKGS_URL__|path:$nixpkgs_path|g" \
  -e "s|__NIXOME_NIXOS_QUBES_PATH__|$nixos_qubes_path|g" \
  -e "s|__NIXOME_SUPPORT_PATH__|$nixome_support_path|g" \
  "$WORK_DIR/flake.nix"

extracted_nixpkgs_url="$(
  sed -n 's/^  inputs\.nixpkgs\.url = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
extracted_nixome_support_path="$(
  sed -n 's/^      nixomeSupport = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
extracted_nixos_qubes_path="$(
  sed -n 's/^      nixosQubesSource = "\(.*\)";$/\1/p' "$WORK_DIR/flake.nix" |
    head -n 1
)"
[ "$extracted_nixpkgs_url" = "path:$nixpkgs_path" ]
[ "$extracted_nixome_support_path" = "$nixome_support_path" ]
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
  .domains."nixome-nixos-work".label == "black" and
  .domains."nixome-nixos-work".netvm == null and
  .domains."nixome-nixos-work".prefs.default_user == "user" and
  .domains."nixome-nixos-work".prefs.kernel == "6.12.64-1.fc41" and
  (.domains."nixome-nixos-work".prefs.kernelopts | contains("init=/sbin/init")) and
  .domains."nixome-nixos-work".features."nixome-role" == "work-template" and
  .domains."nixome-nixos-work".features."nixome-template-source" == "staged-config" and
  .domains."nixome-work".class == "AppVM" and
  .domains."nixome-work".template == "nixome-nixos-work" and
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

printf 'config candidate flake eval ok\n'
