#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/config-candidate-nix-native"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-config-candidate-nix-native.XXXXXX")"
TOPOLOGY_JSON="$RESULT_DIR/topology.json"
HOSTNAME_TXT="$RESULT_DIR/hostname.txt"
ETC_TXT="$RESULT_DIR/etc.txt"
RUNTIME_CHECK="$RESULT_DIR/runtime-check.sh"
INVALID_LOG="$RESULT_DIR/invalid.log"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/nixome"

cp -R "$ROOT/seeds/config-staging/dom0-profile" "$WORK_DIR/dom0-profile"
cp "$ROOT/seeds/config-staging/flake.nix" "$WORK_DIR/flake.nix"
cp "$ROOT/seeds/config-staging/nixome/config-vms.nix" "$WORK_DIR/nixome/config-vms.nix"
cp "$ROOT/seeds/config-staging/nixome/dom0-host.nix" "$WORK_DIR/nixome/dom0-host.nix"
cp "$ROOT/seeds/config-staging/nixome/local.nix" "$WORK_DIR/nixome/local.nix"
cp "$ROOT/seeds/config-staging/nixome/prebuilt-template-sources.nix" "$WORK_DIR/nixome/prebuilt-template-sources.nix"
cp "$ROOT/seeds/config-staging/nixome/system.nix" "$WORK_DIR/nixome/system.nix"
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

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ pkgs, ... }:
{
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
}
EOF

(
  cd "$WORK_DIR"
  nix \
    --extra-experimental-features nix-command \
    --extra-experimental-features flakes \
    flake lock \
    --impure \
    --offline >/dev/null
)

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
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-domain-nix-native-generated.config.networking.hostName" \
  > "$HOSTNAME_TXT"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-domain-nix-native-generated.config.environment.etc.\"nixome/nix-native-role\".text" \
  > "$ETC_TXT"
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --raw \
  "$WORK_DIR#nixosConfigurations.nixome-domain-nix-native-generated.config.environment.etc.\"qubes-rpc/nixome.NixStoreRuntimeCheck\".text" \
  > "$RUNTIME_CHECK"

grep -qx nix-native-generated "$HOSTNAME_TXT"
grep -qx generated "$ETC_TXT"
grep -Fq "sed -n 's|^nixome.storedev=||p'" "$RUNTIME_CHECK"
grep -Fq 'mount_source="$(findmnt -rn -T /nix/store -o SOURCE)"' "$RUNTIME_CHECK"
grep -Fq 'case ",$mount_options," in' "$RUNTIME_CHECK"
grep -Fq '*,ro,*)' "$RUNTIME_CHECK"
jq -e '
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
  .domains."nix-native-generated".backend.root.mode == "qubes-root-image" and
  .domains."nix-native-generated".backend.root.source_ref.attr == "packages.x86_64-linux.nixome-domain-nix-native-generated-root-image" and
  .domains."nix-native-generated".backend.root.source_ref.subpath == "root.img" and
  .domains."nix-native-generated".backend.store.mode == "shared-closure" and
  .domains."nix-native-generated".backend.store.source_ref.attr == "packages.x86_64-linux.nixome-domain-nix-native-generated-store-closure" and
  .domains."nix-native-generated".backend.store.source_ref.subpath == "" and
  .domains."nix-native-generated".backend.store.mountpoint == "/nix/store" and
  .domains."nix-native-generated".backend.store.device == "/dev/xvde" and
  .domains."nix-native-generated".backend.store.read_only == true and
  (.domains."nix-native-generated".prefs.kernelopts | contains("nixome.system=/nix/store/")) and
  (.domains."nix-native-generated".prefs.kernelopts | contains("nixome.storedev=/dev/xvde"))
' "$TOPOLOGY_JSON" >/dev/null

cat > "$WORK_DIR/nixome/machine.nix" <<'EOF'
{ ... }:
{
  nixNative.domains.bad.pakages = [ ];
}
EOF

if nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$WORK_DIR#nixome.topology" > /dev/null 2> "$INVALID_LOG"; then
  printf 'invalid Nix-native domain definition unexpectedly evaluated\n' >&2
  exit 1
fi
grep -Fq 'nixome.nixNative.domains.bad: unknown option(s): pakages' "$INVALID_LOG"

printf 'config candidate Nix-native test ok\n'
