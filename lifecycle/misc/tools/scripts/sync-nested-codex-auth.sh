#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
source "$ROOT/lifecycle/misc/tools/scripts/lib/airlock-env.sh"

LOG_DIR="${AIRLOCK_LOG_DIR:-$ROOT/.airlock-logs}"
RESULT_JSON="$LOG_DIR/vm-gui-sync-codex-auth.json"
SSH_PROBE_STDERR="$LOG_DIR/vm-gui-sync-codex-auth-ssh.stderr.log"
SSH_PORT="${AIRLOCK_VM_SSH_PORT:-2222}"
SSH_USER="${AIRLOCK_VM_SSH_USER:-airlock}"
SSH_PASSWORD="${AIRLOCK_VM_SSH_PASSWORD:-airlock}"
LOCAL_CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
LOCAL_AUTH_JSON="${AIRLOCK_CODEX_AUTH_JSON:-$LOCAL_CODEX_HOME/auth.json}"
LOCAL_CONFIG_TOML="${AIRLOCK_CODEX_CONFIG_TOML:-$LOCAL_CODEX_HOME/config.toml}"
CMD_VM="${AIRLOCK_CODEX_AUTH_VM:-cmd-vm}"
REMOTE_CODEX_HOME="${AIRLOCK_NESTED_CODEX_HOME:-/home/user/.codex}"
REMOTE_DEV_MODE_MARKER="${AIRLOCK_NESTED_DEV_MODE_MARKER:-/etc/airlock/dev-mode}"
SYNC_CONFIG="${AIRLOCK_SYNC_CODEX_CONFIG:-0}"

mkdir -p "$LOG_DIR"

shell_quote() {
  printf '%q' "$1"
}

write_result() {
  local status="$1"
  local reason="${2:-}"
  local config_synced="${3:-false}"

  jq -n \
    --arg schema "airlock.nested-codex-auth-sync.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg local_codex_home "$LOCAL_CODEX_HOME" \
    --arg remote_codex_home "$REMOTE_CODEX_HOME" \
    --arg target_vm "$CMD_VM" \
    --argjson config_synced "$config_synced" \
    '{
      schema: $schema,
      status: $status,
      local_codex_home: $local_codex_home,
      remote_codex_home: $remote_codex_home,
      target_vm: $target_vm,
      auth_json_synced: ($status == "passed"),
      config_toml_synced: $config_synced
    }
    + if $reason == "" then {} else {reason: $reason} end' |
    tee "$RESULT_JSON"
}

fail() {
  local reason="$1"

  write_result failed "$reason" false >&2
  exit 1
}

require_command() {
  local command="$1"

  command -v "$command" >/dev/null 2>&1 || fail "missing required command: $command"
}

ssh_base() {
  SSHPASS="$SSH_PASSWORD" "$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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
  SSHPASS="$SSH_PASSWORD" "$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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

for command in install jq mktemp rm; do
  require_command "$command"
done

if [ ! -f "$LOCAL_AUTH_JSON" ]; then
  fail "missing local Codex auth file: $LOCAL_AUTH_JSON"
fi
if [ ! -s "$LOCAL_AUTH_JSON" ]; then
  fail "local Codex auth file is empty: $LOCAL_AUTH_JSON"
fi
jq -e type "$LOCAL_AUTH_JSON" >/dev/null ||
  fail "local Codex auth file is not valid JSON: $LOCAL_AUTH_JSON"

case "$SYNC_CONFIG" in
  1|true|yes)
    if [ ! -f "$LOCAL_CONFIG_TOML" ]; then
      fail "missing local Codex config file: $LOCAL_CONFIG_TOML"
    fi
    sync_config=true
    ;;
  0|false|no)
    sync_config=false
    ;;
  *)
    fail "unsupported AIRLOCK_SYNC_CODEX_CONFIG value: $SYNC_CONFIG"
    ;;
esac

if ! ssh_base true >/dev/null 2>"$SSH_PROBE_STDERR"; then
  fail "nested dom0 SSH is not reachable on 127.0.0.1:$SSH_PORT"
fi

ssh_base "
  set -e
  marker=$(shell_quote "$REMOTE_DEV_MODE_MARKER")
  [ -f \"\$marker\" ] && grep -qx enabled \"\$marker\"
" >/dev/null || fail "remote nested dom0 is not a dev-mode image"

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/airlock-codex-auth.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT
install -m 0600 "$LOCAL_AUTH_JSON" "$tmpdir/auth.json"
if [ "$sync_config" = true ]; then
  install -m 0600 "$LOCAL_CONFIG_TOML" "$tmpdir/config.toml"
fi

remote_auth_tmp="/tmp/airlock-codex-auth.$$.json"
remote_config_tmp="/tmp/airlock-codex-config.$$.toml"
scp_base "$tmpdir/auth.json" "$SSH_USER@127.0.0.1:$remote_auth_tmp"
ssh_base "
  set -e
  auth_tmp=$(shell_quote "$remote_auth_tmp")
  cmd_vm=$(shell_quote "$CMD_VM")
  sudo -n qrexec-client -d \"\$cmd_vm\" 'DEFAULT:QUBESRPC airlock.CodexAuthImport+ dom0' < \"\$auth_tmp\"
  rm -f \"\$auth_tmp\"
"

if [ "$sync_config" = true ]; then
  scp_base "$tmpdir/config.toml" "$SSH_USER@127.0.0.1:$remote_config_tmp"
  ssh_base "
    set -e
    config_tmp=$(shell_quote "$remote_config_tmp")
    cmd_vm=$(shell_quote "$CMD_VM")
    sudo -n qrexec-client -d \"\$cmd_vm\" 'DEFAULT:QUBESRPC airlock.CodexConfigImport+ dom0' < \"\$config_tmp\"
    rm -f \"\$config_tmp\"
  "
fi

write_result passed "" "$sync_config"
