#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_JSON="$LOG_DIR/vm-gui-sync-dom0-tools.json"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
REMOTE_BIN_DIR="${NIXOME_NESTED_WARM_BIN_DIR:-/run/nixome-warm/bin}"
REMOTE_DROPIN="/run/systemd/system/qubesd.service.d/90-nixome-warm-tools.conf"
RESTART_QUBESD="${NIXOME_NESTED_WARM_RESTART_QUBESD:-1}"

mkdir -p "$LOG_DIR"

shell_quote() {
  printf '%q' "$1"
}

ssh_base() {
  SSHPASS="$SSH_PASSWORD" "$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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
  SSHPASS="$SSH_PASSWORD" "$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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

  out="$("$ROOT/scripts/nixome-nix.sh" build "$ROOT#$attr" --no-link --print-out-paths --print-build-logs --show-trace)"
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
nix_closure_out="$(build_output packages.x86_64-linux.nixome-nix-closure-domain-smoke)"
storage_out="$(build_output packages.x86_64-linux.nixome-qubes-storage)"

mapfile -t closure_paths < <(
  nix-store -qR "$setup_config_out" "$nix_closure_out" "$storage_out" | sort -u
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
ssh_base "sudo -n ln -sfn $(shell_quote "$nix_closure_out/bin/nixome-nix-closure-domain-smoke") $(shell_quote "$REMOTE_BIN_DIR/nixome-nix-closure-domain-smoke")"
scp_base "$dropin" "$SSH_USER@127.0.0.1:/tmp/nixome-warm-tools-qubesd.conf"
ssh_base "sudo -n mv /tmp/nixome-warm-tools-qubesd.conf $(shell_quote "$REMOTE_DROPIN") && sudo -n systemctl daemon-reload"

qubesd_restarted=false
case "$RESTART_QUBESD" in
  1|true|yes)
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

jq -n \
  --arg schema "nixome.nested-dom0-tool-sync.v1" \
  --arg status "passed" \
  --arg setup_config "$setup_config_out" \
  --arg nix_closure_smoke "$nix_closure_out" \
  --arg storage "$storage_out" \
  --arg storage_site "$storage_site" \
  --arg remote_bin_dir "$REMOTE_BIN_DIR" \
  --arg qubesd_pythonpath "$qubesd_pythonpath" \
  --arg qubesd_path "$qubesd_path" \
  --argjson closure_paths_total "${#closure_paths[@]}" \
  --argjson imported_paths "${#missing_paths[@]}" \
  --argjson qubesd_restarted "$qubesd_restarted" \
  '{
    schema: $schema,
    status: $status,
    setup_config: $setup_config,
    nix_closure_smoke: $nix_closure_smoke,
    storage: $storage,
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
