#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/nixome-minimal-closure"
RESULT_JSON="$RESULT_DIR/result.json"

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"

declare -A paths

build_role() {
  role="$1"
  attr="$ROOT#nixosConfigurations.nixome-domain-$role.config.system.build.toplevel"
  nix build --no-link --print-out-paths "$attr" --print-build-logs --show-trace
}

closure_file() {
  role="$1"
  printf '%s/closure-%s.txt' "$RESULT_DIR" "$role"
}

closure_has() {
  role="$1"
  pattern="$2"
  rg -q "$pattern" "$(closure_file "$role")"
}

assert_has() {
  role="$1"
  pattern="$2"
  description="$3"

  if ! closure_has "$role" "$pattern"; then
    printf 'missing expected %s in %s closure\n' "$description" "$role" >&2
    printf 'pattern: %s\n' "$pattern" >&2
    return 1
  fi
}

assert_absent() {
  role="$1"
  pattern="$2"
  description="$3"

  if closure_has "$role" "$pattern"; then
    printf 'unexpected %s in %s closure\n' "$description" "$role" >&2
    printf 'pattern: %s\n' "$pattern" >&2
    rg "$pattern" "$(closure_file "$role")" >&2
    return 1
  fi
}

write_role_json() {
  role="$1"
  path="${paths[$role]}"
  count="$(wc -l < "$(closure_file "$role")" | tr -d ' ')"
  size="$(nix path-info --json-format 1 --json -S "$path" | jq -r 'to_entries[0].value.closureSize')"

  jq -n \
    --arg role "$role" \
    --arg path "$path" \
    --argjson store_paths "$count" \
    --argjson closure_size_bytes "$size" \
    '{role: $role, path: $path, store_paths: $store_paths, closure_size_bytes: $closure_size_bytes}'
}

roles=(
  config-staging
  config-validator
  config-vault
)

for role in "${roles[@]}"; do
  paths[$role]="$(build_role "$role")"
  nix path-info -r "${paths[$role]}" > "$(closure_file "$role")"
done

for role in "${roles[@]}"; do
  assert_has "$role" '/nix/store/[[:alnum:]]+-qubes-core-qrexec-domU-' "qrexec domU runtime"
  assert_has "$role" '/nix/store/[[:alnum:]]+-qubes-core-qubesdb-client-' "QubesDB client"
  assert_has "$role" '/nix/store/[[:alnum:]]+-qubes-core-qubesdb-daemon-' "QubesDB daemon"
  assert_has "$role" '/nix/store/[[:alnum:]]+-qubes-core-agent-linux-[0-9]' "upstream Qubes core agent package"
  assert_has "$role" '/nix/store/[[:alnum:]]+-python[^/]*-qubes-core-qrexec' "upstream Python qrexec package"
  assert_has "$role" '/nix/store/[[:alnum:]]+-systemd-[0-9]' "full systemd package"
  assert_has "$role" '/nix/store/[[:alnum:]]+-userborn-' "userborn passwd/group generator"

  assert_absent "$role" '/nix/store/[[:alnum:]]+-(source|nixpkgs|nixpkgs-libvirt)' "runtime source tree"
  assert_absent "$role" '/nix/store/[[:alnum:]]+-openvswitch-' "Open vSwitch"
  assert_absent "$role" '/nix/store/[[:alnum:]]+-(gtk3|pygobject|librsvg)-' "GUI stack"
  assert_absent "$role" '/nix/store/[[:alnum:]]+-nixos-(build-vms|enter|generate-config|install|option)' "NixOS installer/admin tooling"
  assert_absent "$role" '/nix/store/[[:alnum:]]+-(lvm2|dosfstools|bcache-tools|btrfs-progs|kexec-tools)-' "generic storage/admin tools"
done

for role in config-staging config-validator; do
  assert_has "$role" '/nix/store/[[:alnum:]]+-nix-[0-9]' "role-specific Nix CLI"
  assert_has "$role" '/nix/store/[[:alnum:]]+-nix-flake' "role-specific nix flake support"
  assert_has "$role" '/nix/store/[[:alnum:]]+-nixome-config-nix' "role-specific Nixome Nix helper"
done

assert_absent config-vault '/nix/store/[[:alnum:]]+-nix-[0-9]' "Nix CLI"
assert_absent config-vault '/nix/store/[[:alnum:]]+-nix-flake' "nix flake support"
assert_absent config-vault '/nix/store/[[:alnum:]]+-nixome-config-nix' "Nixome Nix helper"

{
  printf '{"schema":"nixome.minimal-closure-test.v1","status":"passed","roles":['
  first=1
  for role in "${roles[@]}"; do
    if [ "$first" = "1" ]; then
      first=0
    else
      printf ','
    fi
    write_role_json "$role"
  done
  printf ']}\n'
} > "$RESULT_JSON"

printf 'nixome minimal closure test ok\n'
