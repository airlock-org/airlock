#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
source "$ROOT/runtime/tools/scripts/lib/nixome-env.sh"

LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_JSON="$LOG_DIR/vm-gui-sync-codex-auth.json"
SSH_PROBE_STDERR="$LOG_DIR/vm-gui-sync-codex-auth-ssh.stderr.log"
SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
LOCAL_CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
LOCAL_AUTH_JSON="${NIXOME_CODEX_AUTH_JSON:-$LOCAL_CODEX_HOME/auth.json}"
LOCAL_CONFIG_TOML="${NIXOME_CODEX_CONFIG_TOML:-$LOCAL_CODEX_HOME/config.toml}"
REMOTE_CODEX_HOME="${NIXOME_NESTED_CODEX_HOME:-/home/$SSH_USER/.codex}"
REMOTE_DEV_MODE_MARKER="${NIXOME_NESTED_DEV_MODE_MARKER:-/etc/nixome/dev-mode}"
SYNC_CONFIG="${NIXOME_SYNC_CODEX_CONFIG:-0}"

mkdir -p "$LOG_DIR"

shell_quote() {
  printf '%q' "$1"
}

write_result() {
  local status="$1"
  local reason="${2:-}"
  local config_synced="${3:-false}"

  jq -n \
    --arg schema "nixome.nested-codex-auth-sync.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg local_codex_home "$LOCAL_CODEX_HOME" \
    --arg remote_codex_home "$REMOTE_CODEX_HOME" \
    --argjson config_synced "$config_synced" \
    '{
      schema: $schema,
      status: $status,
      local_codex_home: $local_codex_home,
      remote_codex_home: $remote_codex_home,
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
  SSHPASS="$SSH_PASSWORD" "$ROOT/runtime/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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
  SSHPASS="$SSH_PASSWORD" "$ROOT/runtime/tools/scripts/nixome-nix.sh" shell nixpkgs#sshpass -c sshpass -e \
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
    fail "unsupported NIXOME_SYNC_CODEX_CONFIG value: $SYNC_CONFIG"
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

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/nixome-codex-auth.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT
install -m 0600 "$LOCAL_AUTH_JSON" "$tmpdir/auth.json"
if [ "$sync_config" = true ]; then
  install -m 0600 "$LOCAL_CONFIG_TOML" "$tmpdir/config.toml"
fi

remote_auth_tmp="/tmp/nixome-codex-auth.$$.json"
remote_config_tmp="/tmp/nixome-codex-config.$$.toml"
scp_base "$tmpdir/auth.json" "$SSH_USER@127.0.0.1:$remote_auth_tmp"
ssh_base "
  set -e
  codex_home=$(shell_quote "$REMOTE_CODEX_HOME")
  auth_tmp=$(shell_quote "$remote_auth_tmp")
  install -d -m 0700 \"\$codex_home\"
  install -m 0600 \"\$auth_tmp\" \"\$codex_home/auth.json\"
  rm -f \"\$auth_tmp\"
"

if [ "$sync_config" = true ]; then
  scp_base "$tmpdir/config.toml" "$SSH_USER@127.0.0.1:$remote_config_tmp"
  ssh_base "
    set -e
    codex_home=$(shell_quote "$REMOTE_CODEX_HOME")
    config_tmp=$(shell_quote "$remote_config_tmp")
    install -d -m 0700 \"\$codex_home\"
    install -m 0600 \"\$config_tmp\" \"\$codex_home/config.toml\"
    rm -f \"\$config_tmp\"
  "
fi

write_result passed "" "$sync_config"
