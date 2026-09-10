#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/nixome-env.sh"

LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_JSON="$LOG_DIR/vm-gui-sync-dom0-tools.json"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
REMOTE_BIN_DIR="${NIXOME_NESTED_WARM_BIN_DIR:-/run/nixome-warm/bin}"
REMOTE_TOPOLOGY_PATH="${NIXOME_NESTED_WARM_TOPOLOGY_PATH:-/run/nixome-warm/current-qubes-topology.json}"
REMOTE_CONFIG_INPUTS_PATH="${NIXOME_NESTED_WARM_CONFIG_INPUTS_PATH:-/run/nixome-warm/config-inputs}"
REMOTE_CONFIG_INPUTS_SYSTEM_PATH="${NIXOME_NESTED_WARM_CONFIG_INPUTS_SYSTEM_PATH:-/etc/nixome/config-inputs}"
REMOTE_DROPIN="/run/systemd/system/qubesd.service.d/zz-nixome-warm-tools.conf"
REMOTE_OLD_DROPIN="/run/systemd/system/qubesd.service.d/90-nixome-warm-tools.conf"
RESTART_QUBESD="${NIXOME_NESTED_WARM_RESTART_QUBESD:-1}"
SYNC_TOPOLOGY="${NIXOME_NESTED_WARM_SYNC_TOPOLOGY:-1}"
SYNC_CONFIG_INPUTS="${NIXOME_NESTED_WARM_SYNC_CONFIG_INPUTS:-1}"
WARM_NIX_DOMAINS="${NIXOME_NESTED_WARM_NIX_DOMAINS:-config-staging config-validator config-vault}"
TOPOLOGY_JSON="$LOG_DIR/vm-gui-current-qubes-topology.json"
CONFIG_INPUTS_JSON="$LOG_DIR/vm-gui-current-config-inputs.json"
CONFIG_INPUTS_MANIFEST_JSON="$LOG_DIR/vm-gui-current-config-inputs-manifest.json"
CONFIG_INPUTS_JSONL="$LOG_DIR/vm-gui-sync-dom0-tools-config-inputs.jsonl"
DOMAIN_ARTIFACTS_JSONL="$LOG_DIR/vm-gui-sync-dom0-tools-nix-domains.jsonl"

mkdir -p "$LOG_DIR"
: > "$CONFIG_INPUTS_JSONL"
: > "$DOMAIN_ARTIFACTS_JSONL"

shell_quote() {
  printf '%q' "$1"
}

ssh_base() {
  SSHPASS="$SSH_PASSWORD" "$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
    ssh \
      -F /dev/null \
      -o LogLevel=ERROR \
      -o StrictHostKeyChecking=no \
      -o UserKnownHostsFile=/dev/null \
      -o ConnectTimeout=5 \
      -o NumberOfPasswordPrompts=1 \
      -o PubkeyAuthentication=no \
      -p "$SSH_PORT" \
      "$SSH_USER@127.0.0.1" \
      "$@"
}

scp_base() {
  SSHPASS="$SSH_PASSWORD" "$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
    scp \
      -F /dev/null \
      -q \
      -o LogLevel=ERROR \
      -o StrictHostKeyChecking=no \
      -o UserKnownHostsFile=/dev/null \
      -o ConnectTimeout=5 \
      -o NumberOfPasswordPrompts=1 \
      -o PubkeyAuthentication=no \
      -P "$SSH_PORT" \
      "$@"
}

build_output() {
  local attr="$1"
  local out

  out="$("$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" build "$ROOT#$attr" --no-link --print-out-paths --print-build-logs --show-trace)"
  awk 'NF { last = $0 } END { print last }' <<< "$out"
}

find_output_with_bin() {
  local binary="$1"
  local output
  local physical

  for output in "${closure_paths[@]}"; do
    physical="$(nixome_physical_store_path "$output")"
    if [ -x "$physical/bin/$binary" ]; then
      printf '%s\n' "$output"
      return 0
    fi
  done

  printf 'missing closure path containing bin/%s\n' "$binary" >&2
  return 1
}

printf 'warm-sync: building current dom0 tools\n' >&2
setup_config_out="$(build_output packages.x86_64-linux.nixome-setup-config)"
config_inputs_import_out="$(build_output packages.x86_64-linux.nixome-config-inputs-import)"
config_vms_smoke_out="$(build_output packages.x86_64-linux.nixome-config-vms-smoke)"
nix_closure_out="$(build_output packages.x86_64-linux.nixome-nix-closure-domain-smoke)"
storage_out="$(build_output packages.x86_64-linux.nixome-qubes-storage)"

topology_synced=false
topology_sources=()
case "$SYNC_TOPOLOGY" in
  1|true|yes)
    printf 'warm-sync: evaluating current concrete Qubes topology\n' >&2
    "$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" eval \
      --impure \
      --json \
      "$ROOT#nixosConfigurations.vm-gui-config.config.nixome.qubes.topology" \
      > "$TOPOLOGY_JSON"
    jq -e '.schema == "nixome.qubes-topology.v1"' "$TOPOLOGY_JSON" >/dev/null

    printf 'warm-sync: building current config-domain artifacts\n' >&2
    qubes_kernel_out="$(build_output packages.x86_64-linux.nixome-qubes-vm-kernel)"
    topology_sources+=("$qubes_kernel_out")
    for domain in $WARM_NIX_DOMAINS; do
      system_out="$(build_output "nixosConfigurations.nixome-domain-$domain.config.system.build.toplevel")"
      root_out="$(build_output "packages.x86_64-linux.nixome-domain-$domain-root-image")"
      store_out="$(build_output "packages.x86_64-linux.nixome-domain-$domain-store-closure")"
      topology_sources+=("$system_out" "$root_out" "$store_out")
      jq -c -n \
        --arg domain "$domain" \
        --arg system "$system_out" \
        --arg root "$root_out" \
        --arg store "$store_out" \
        '{
          domain: $domain,
          system: $system,
          root: $root,
          store: $store
        }' >> "$DOMAIN_ARTIFACTS_JSONL"
    done

    mapfile -t topology_store_roots < <(
      jq -r '
        ..
        | strings
        | select(startswith("/nix/store/"))
        | capture("^(?<root>/nix/store/[^/]+)").root
      ' "$TOPOLOGY_JSON" | sort -u
    )
    topology_sources+=("${topology_store_roots[@]}")
    mapfile -t topology_sources < <(printf '%s\n' "${topology_sources[@]}" | sed '/^$/d' | sort -u)
    topology_synced=true
    ;;
  0|false|no)
    printf 'warm-sync: topology artifact sync disabled\n' >&2
    ;;
  *)
    printf 'unsupported NIXOME_NESTED_WARM_SYNC_TOPOLOGY value: %s\n' "$SYNC_TOPOLOGY" >&2
    exit 1
    ;;
esac

config_inputs_synced=false
config_input_sources=()
case "$SYNC_CONFIG_INPUTS" in
  1|true|yes)
    printf 'warm-sync: evaluating current config inputs\n' >&2
    "$ROOT/lifecycle/misc/tools/scripts/nixome-nix.sh" eval \
      --impure \
      --json \
      "$ROOT#nixome.configInputs" \
      > "$CONFIG_INPUTS_JSON"
    jq -e '
      (.nixpkgs | startswith("/nix/store/")) and
      (.nixpkgsLibvirt105 | startswith("/nix/store/")) and
      (.nixosQubes | startswith("/nix/store/")) and
      (.nixomeSupport | startswith("/nix/store/"))
    ' "$CONFIG_INPUTS_JSON" >/dev/null
    jq '
      {
        schema: "nixome.config-inputs.v1",
        inputs: {
          nixpkgs: .nixpkgs,
          "nixpkgs-libvirt_10_5": .nixpkgsLibvirt105,
          "nixos-qubes": .nixosQubes,
          "nixome-support": .nixomeSupport
        }
      }
    ' "$CONFIG_INPUTS_JSON" > "$CONFIG_INPUTS_MANIFEST_JSON"

    jq -r '
      to_entries[]
      | .value
      | capture("^(?<root>/nix/store/[^/]+)").root
    ' "$CONFIG_INPUTS_JSON" | sort -u > "$LOG_DIR/vm-gui-current-config-input-store-roots.txt"
    mapfile -t config_input_sources < "$LOG_DIR/vm-gui-current-config-input-store-roots.txt"

    jq -r '
      [
        ["nixpkgs", .nixpkgs],
        ["nixpkgs-libvirt_10_5", .nixpkgsLibvirt105],
        ["nixos-qubes", .nixosQubes],
        ["nixome-support", .nixomeSupport]
      ][]
      | @tsv
    ' "$CONFIG_INPUTS_JSON" > "$CONFIG_INPUTS_JSONL"
    config_inputs_synced=true
    ;;
  0|false|no)
    printf 'warm-sync: config input sync disabled\n' >&2
    ;;
  *)
    printf 'unsupported NIXOME_NESTED_WARM_SYNC_CONFIG_INPUTS value: %s\n' "$SYNC_CONFIG_INPUTS" >&2
    exit 1
    ;;
esac

mapfile -t closure_paths < <(
  nix-store -qR \
    "$setup_config_out" \
    "$config_inputs_import_out" \
    "$config_vms_smoke_out" \
    "$nix_closure_out" \
    "$storage_out" \
    "${topology_sources[@]}" \
    "${config_input_sources[@]}" |
    sort -u
)

storage_physical="$(nixome_physical_store_path "$storage_out")"
storage_site_physical="$(find "$storage_physical/lib" -type d -name site-packages -print -quit)"
[ -n "$storage_site_physical" ] || {
  printf 'could not locate site-packages under %s\n' "$storage_out" >&2
  exit 1
}
storage_site="$storage_out/${storage_site_physical#"$storage_physical"/}"

dmsetup_out="$(find_output_with_bin dmsetup)"
losetup_out="$(find_output_with_bin losetup)"
erofs_out="$(find_output_with_bin mkfs.erofs)"
domain_storage_out="$(find_output_with_bin nixome-nix-domain-storage)"
dmsetup_bin="$dmsetup_out/bin"
losetup_bin="$losetup_out/bin"
erofs_bin="$erofs_out/bin"

printf 'warm-sync: checking missing store paths in nested dom0\n' >&2
missing_paths_text="$(
  printf '%s\n' "${closure_paths[@]}" |
    ssh_base 'while IFS= read -r path; do [ -e "$path" ] || printf "%s\n" "$path"; done'
)"
mapfile -t missing_paths < <(printf '%s\n' "$missing_paths_text" | sed '/^$/d')

if [ "${#missing_paths[@]}" -gt 0 ]; then
  printf 'warm-sync: importing %s missing store paths into nested dom0\n' "${#missing_paths[@]}" >&2
  nix-store --export "${missing_paths[@]}" |
    ssh_base 'sudo -n nix-store --import >/tmp/nixome-warm-tools-import.log'
else
  printf 'warm-sync: all tool closure paths already present in nested dom0\n' >&2
fi

old_pythonpath="$(
  ssh_base 'systemctl show -p Environment --value qubesd.service | tr " " "\n" | sed -n "s/^PYTHONPATH=//p" | head -n 1' || true
)"
if [ -n "$old_pythonpath" ]; then
  qubesd_pythonpath="$storage_site:$old_pythonpath"
else
  qubesd_pythonpath="$storage_site"
fi
qubesd_path="$dmsetup_bin:$losetup_bin:$erofs_bin:/run/wrappers/bin:/run/current-system/sw/bin"

dropin="$LOG_DIR/vm-gui-sync-dom0-tools-qubesd.conf"
{
  printf '[Service]\n'
  printf 'Environment=PYTHONPATH=%s\n' "$qubesd_pythonpath"
  printf 'Environment=PATH=%s\n' "$qubesd_path"
} > "$dropin"

printf 'warm-sync: installing warm tool symlinks and qubesd override\n' >&2
ssh_base "sudo -n mkdir -p $(shell_quote "$REMOTE_BIN_DIR") /run/systemd/system/qubesd.service.d"
ssh_base "sudo -n ln -sfn $(shell_quote "$setup_config_out/bin/nixome-setup-config") $(shell_quote "$REMOTE_BIN_DIR/nixome-setup-config")"
ssh_base "sudo -n ln -sfn $(shell_quote "$config_inputs_import_out/bin/nixome-config-inputs-import") $(shell_quote "$REMOTE_BIN_DIR/nixome-config-inputs-import")"
ssh_base "sudo -n ln -sfn $(shell_quote "$config_vms_smoke_out/bin/nixome-config-vms-smoke") $(shell_quote "$REMOTE_BIN_DIR/nixome-config-vms-smoke")"
ssh_base "sudo -n ln -sfn $(shell_quote "$nix_closure_out/bin/nixome-nix-closure-domain-smoke") $(shell_quote "$REMOTE_BIN_DIR/nixome-nix-closure-domain-smoke")"
ssh_base "sudo -n ln -sfn $(shell_quote "$storage_out/bin/nixome-closure-store") $(shell_quote "$REMOTE_BIN_DIR/nixome-closure-store")"
ssh_base "sudo -n ln -sfn $(shell_quote "$domain_storage_out/bin/nixome-nix-domain-storage") $(shell_quote "$REMOTE_BIN_DIR/nixome-nix-domain-storage")"
scp_base "$dropin" "$SSH_USER@127.0.0.1:/tmp/nixome-warm-tools-qubesd.conf"
ssh_base "sudo -n unlink $(shell_quote "$REMOTE_OLD_DROPIN") 2>/dev/null || true; sudo -n mv /tmp/nixome-warm-tools-qubesd.conf $(shell_quote "$REMOTE_DROPIN") && sudo -n systemctl daemon-reload"

if [ "$topology_synced" = true ]; then
  remote_topology_dir="${REMOTE_TOPOLOGY_PATH%/*}"
  ssh_base "sudo -n mkdir -p $(shell_quote "$remote_topology_dir")"
  scp_base "$TOPOLOGY_JSON" "$SSH_USER@127.0.0.1:/tmp/nixome-warm-current-qubes-topology.json"
  ssh_base "sudo -n mv /tmp/nixome-warm-current-qubes-topology.json $(shell_quote "$REMOTE_TOPOLOGY_PATH")"
fi

if [ "$config_inputs_synced" = true ]; then
  remote_config_inputs_parent="${REMOTE_CONFIG_INPUTS_PATH%/*}"
  mapfile -t config_input_entries < "$CONFIG_INPUTS_JSONL"
  ssh_base "sudo -n mkdir -p $(shell_quote "$remote_config_inputs_parent") $(shell_quote "$REMOTE_CONFIG_INPUTS_PATH")"
  remote_config_inputs_system_parent="${REMOTE_CONFIG_INPUTS_SYSTEM_PATH%/*}"
  ssh_base "sudo -n mkdir -p $(shell_quote "$remote_config_inputs_system_parent") $(shell_quote "$REMOTE_CONFIG_INPUTS_SYSTEM_PATH")"
  scp_base "$CONFIG_INPUTS_MANIFEST_JSON" "$SSH_USER@127.0.0.1:/tmp/nixome-warm-config-inputs-manifest.json"
  ssh_base "
    set -e
    manifest_tmp=/tmp/nixome-warm-config-inputs-manifest.json
    sudo -n install -m 0644 \"\$manifest_tmp\" $(shell_quote "$REMOTE_CONFIG_INPUTS_PATH/manifest.json")
    sudo -n install -m 0644 \"\$manifest_tmp\" $(shell_quote "$REMOTE_CONFIG_INPUTS_SYSTEM_PATH/manifest.json")
    sudo -n rm -f \"\$manifest_tmp\"
  "
  for remote_config_inputs_root in "$REMOTE_CONFIG_INPUTS_PATH" "$REMOTE_CONFIG_INPUTS_SYSTEM_PATH"; do
    for config_input_entry in "${config_input_entries[@]}"; do
      IFS=$'\t' read -r input_name input_path <<< "$config_input_entry"
      [ -n "$input_name" ] || continue
      input_dest="$remote_config_inputs_root/$input_name"
      ssh_base "
        set -e
        input_dest=$(shell_quote "$input_dest")
        input_path=$(shell_quote "$input_path")
        if mountpoint -q \"\$input_dest\"; then
          sudo -n umount \"\$input_dest\"
        fi
        if [ -L \"\$input_dest\" ] || { [ -e \"\$input_dest\" ] && [ ! -d \"\$input_dest\" ]; }; then
          sudo -n rm -rf \"\$input_dest\"
        fi
        sudo -n mkdir -p \"\$input_dest\"
        sudo -n mount --bind \"\$input_path\" \"\$input_dest\"
      "
    done
  done
fi

qubesd_restarted=false
case "$RESTART_QUBESD" in
  1|true|yes)
    ssh_base 'sudo -n systemctl reset-failed qubesd.service || true'
    ssh_base 'sudo -n systemctl restart qubesd.service'
    qubesd_restarted=true
    ;;
  0|false|no)
    ;;
  *)
    printf 'unsupported NIXOME_NESTED_WARM_RESTART_QUBESD value: %s\n' "$RESTART_QUBESD" >&2
    exit 1
    ;;
esac

if [ "${#topology_sources[@]}" -gt 0 ]; then
  topology_sources_json="$(printf '%s\n' "${topology_sources[@]}" | jq -R . | jq -s .)"
else
  topology_sources_json='[]'
fi
if [ "${#config_input_sources[@]}" -gt 0 ]; then
  config_input_sources_json="$(printf '%s\n' "${config_input_sources[@]}" | jq -R . | jq -s .)"
else
  config_input_sources_json='[]'
fi
domain_artifacts_json="$(jq -s '.' "$DOMAIN_ARTIFACTS_JSONL")"
config_inputs_json="$(jq -Rn '
  reduce inputs as $line (
    {};
    ($line | split("\t")) as $fields
    | if ($fields | length) == 2 then .[$fields[0]] = $fields[1] else . end
  )
' < "$CONFIG_INPUTS_JSONL")"

jq -n \
  --arg schema "nixome.nested-dom0-tool-sync.v1" \
  --arg status "passed" \
  --arg setup_config "$setup_config_out" \
  --arg config_inputs_import "$config_inputs_import_out" \
  --arg config_vms_smoke "$config_vms_smoke_out" \
  --arg nix_closure_smoke "$nix_closure_out" \
  --arg storage "$storage_out" \
  --arg domain_storage "$domain_storage_out" \
  --arg remote_topology "$REMOTE_TOPOLOGY_PATH" \
  --arg remote_config_inputs "$REMOTE_CONFIG_INPUTS_PATH" \
  --arg remote_config_inputs_system "$REMOTE_CONFIG_INPUTS_SYSTEM_PATH" \
  --arg storage_site "$storage_site" \
  --arg remote_bin_dir "$REMOTE_BIN_DIR" \
  --arg qubesd_pythonpath "$qubesd_pythonpath" \
  --arg qubesd_path "$qubesd_path" \
  --argjson topology_synced "$topology_synced" \
  --argjson topology_sources "$topology_sources_json" \
  --argjson domain_artifacts "$domain_artifacts_json" \
  --argjson config_inputs_synced "$config_inputs_synced" \
  --argjson config_input_sources "$config_input_sources_json" \
  --argjson config_inputs "$config_inputs_json" \
  --argjson closure_paths_total "${#closure_paths[@]}" \
  --argjson imported_paths "${#missing_paths[@]}" \
  --argjson qubesd_restarted "$qubesd_restarted" \
  '{
    schema: $schema,
    status: $status,
    setup_config: $setup_config,
    config_inputs_import: $config_inputs_import,
    config_vms_smoke: $config_vms_smoke,
    nix_closure_smoke: $nix_closure_smoke,
    storage: $storage,
    domain_storage: $domain_storage,
    topology: {
      synced: $topology_synced,
      remote_path: $remote_topology,
      sources: $topology_sources,
      domains: $domain_artifacts
    },
    config_inputs: {
      synced: $config_inputs_synced,
      remote_path: $remote_config_inputs,
      system_path: $remote_config_inputs_system,
      sources: $config_input_sources,
      inputs: $config_inputs
    },
    storage_site: $storage_site,
    remote_bin_dir: $remote_bin_dir,
    qubesd: {
      pythonpath: $qubesd_pythonpath,
      path: $qubesd_path,
      restarted: $qubesd_restarted
    },
    closure_paths_total: $closure_paths_total,
    imported_paths: $imported_paths
  }' | tee "$RESULT_JSON"
