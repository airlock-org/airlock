#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${AIRLOCK_LOG_DIR:-/var/log/airlock}"
CONTROL_LOG_DIR="${AIRLOCK_CONFIG_CONTROL_LOG_DIR:-$LOG_DIR/config-control}"
SYSTEM_STATE_DIR="${AIRLOCK_SYSTEM_STATE_DIR:-/var/lib/airlock/system}"
HOST_SYSTEM_PROFILE="${AIRLOCK_HOST_SYSTEM_PROFILE:-/nix/var/nix/profiles/system}"
HOST_CURRENT_SYSTEM_LINK="${AIRLOCK_HOST_CURRENT_SYSTEM_LINK:-/run/current-system}"
STAGING="${AIRLOCK_CONFIG_STAGING_VM:-sys-config-staging}"
VALIDATOR="${AIRLOCK_CONFIG_VALIDATOR_VM:-sys-config-validator}"
VAULT="${AIRLOCK_CONFIG_VAULT_VM:-sys-config-vault}"

operation="${1:-}"
tmpdir=""

usage() {
  cat >&2 <<'EOF'
Usage: airlock-config-control (status|check|apply|rollback)

Dom0 qrexec entry point for narrow Airlock config generation operations.
EOF
  exit 1
}

cleanup() {
  if [ -n "$tmpdir" ]; then
    rm -rf "$tmpdir"
  fi
}
trap cleanup EXIT

json_string() {
  printf '%s' "$1" | jq -Rs . | tr -d '\n'
}

write_control_result() {
  local status="$1"
  local reason="$2"
  local schema="airlock.config-control.v1"

  printf '{"schema":'
  json_string "$schema"
  printf ',"operation":'
  json_string "$operation"
  printf ',"status":'
  json_string "$status"
  printf ',"reason":'
  json_string "$reason"
  printf '}\n'
}

fail_json() {
  local reason="$1"

  write_control_result failed "$reason"
  printf 'airlock config control failed: %s\n' "$reason" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail_json "missing command: $1"
  fi
}

operation_log_dir() {
  local stamp

  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  mkdir -p "$CONTROL_LOG_DIR"
  mktemp -d "$CONTROL_LOG_DIR/$operation-$stamp.XXXXXX"
}

verify_staged_candidate_tar() {
  local candidate_tar="$1"

  tar -tf "$candidate_tar" |
    awk '
      $0 != "config.bundle" && $0 != "manifest.json" {
        printf "unexpected staged candidate tar member: %s\n", $0 > "/dev/stderr"
        exit 1
      }
    '
}

read_candidate_tar() {
  tmpdir="$(mktemp -d)"
  candidate_tar="$tmpdir/candidate.tar"
  cat > "$candidate_tar"
  verify_staged_candidate_tar "$candidate_tar" ||
    fail_json "candidate tar contains unexpected paths"
}

run_check() {
  local candidate_tar="$1"
  local op_log
  local stdout_log
  local stderr_log
  local result_json
  local rc=0

  op_log="$(operation_log_dir)"
  stdout_log="$op_log/stdout.log"
  stderr_log="$op_log/stderr.log"
  result_json="$op_log/config-archive.json"

  env \
    AIRLOCK_LOG_DIR="$op_log" \
    AIRLOCK_CONFIG_STAGING_VM="$STAGING" \
    AIRLOCK_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    AIRLOCK_CONFIG_VAULT_VM="$VAULT" \
    airlock-config-archive-candidate \
      --candidate-tar "$candidate_tar" \
      --validate-only \
      --staging "$STAGING" \
      --validator "$VALIDATOR" \
      --vault "$VAULT" \
      > "$stdout_log" 2> "$stderr_log" || rc="$?"

  if [ -r "$result_json" ]; then
    cat "$result_json"
  else
    write_control_result failed "candidate validation did not produce result JSON"
  fi

  return "$rc"
}

run_apply() {
  local candidate_tar="$1"
  local op_log
  local stdout_log
  local stderr_log
  local result_json
  local rc=0

  op_log="$(operation_log_dir)"
  stdout_log="$op_log/stdout.log"
  stderr_log="$op_log/stderr.log"
  result_json="$op_log/config-apply.json"

  env \
    AIRLOCK_LOG_DIR="$op_log" \
    AIRLOCK_CONFIG_STAGING_VM="$STAGING" \
    AIRLOCK_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    AIRLOCK_CONFIG_VAULT_VM="$VAULT" \
    airlock-config-apply \
      --apply \
      --candidate-tar "$candidate_tar" \
      --staging "$STAGING" \
      --validator "$VALIDATOR" \
      --vault "$VAULT" \
      > "$stdout_log" 2> "$stderr_log" || rc="$?"

  if [ -r "$result_json" ]; then
    cat "$result_json"
  else
    write_control_result failed "candidate apply did not produce result JSON"
  fi

  return "$rc"
}

rollback_host_system() {
  local op_log
  local result_json
  local current_target=""
  local previous_target=""
  local method=""
  local switch_log
  local profile_log
  local rc=0

  op_log="$(operation_log_dir)"
  result_json="$op_log/config-rollback.json"
  switch_log="$op_log/switch-to-configuration.log"
  profile_log="$op_log/profile-rollback.log"

  mkdir -p "$SYSTEM_STATE_DIR"

  if [ -e "$HOST_SYSTEM_PROFILE" ] || [ -L "$HOST_SYSTEM_PROFILE" ]; then
    current_target="$(readlink -f "$HOST_SYSTEM_PROFILE" || true)"
  fi

  if [ -r "$SYSTEM_STATE_DIR/previous-host-system" ]; then
    previous_target="$(sed -n '1p' "$SYSTEM_STATE_DIR/previous-host-system")"
    method="state"
    ln -sfn "$previous_target" "$HOST_SYSTEM_PROFILE"
  else
    method="nix-profile"
    if ! nix-env -p "$HOST_SYSTEM_PROFILE" --rollback > "$profile_log" 2>&1; then
      rc="$?"
    fi
    previous_target="$(readlink -f "$HOST_SYSTEM_PROFILE" || true)"
  fi

  if [ "$rc" != 0 ]; then
    jq -c -n \
      --arg schema "airlock.config-rollback.v1" \
      --arg status "failed" \
      --arg reason "host system profile rollback failed" \
      --arg method "$method" \
      --arg current "$current_target" \
      --arg target "$previous_target" \
      '{schema: $schema, status: $status, reason: $reason, method: $method, previous_current_system: $current, target_system: $target}' \
      > "$result_json"
    cat "$result_json"
    return "$rc"
  fi

  if [ -z "$previous_target" ] || [ "$previous_target" = "$current_target" ]; then
    jq -c -n \
      --arg schema "airlock.config-rollback.v1" \
      --arg status "failed" \
      --arg reason "no previous host system generation is available" \
      --arg method "$method" \
      --arg current "$current_target" \
      --arg target "$previous_target" \
      '{schema: $schema, status: $status, reason: $reason, method: $method, previous_current_system: $current, target_system: $target}' \
      > "$result_json"
    cat "$result_json"
    return 1
  fi

  if [ ! -x "$previous_target/bin/switch-to-configuration" ]; then
    jq -c -n \
      --arg schema "airlock.config-rollback.v1" \
      --arg status "failed" \
      --arg reason "previous host system has no switch-to-configuration" \
      --arg method "$method" \
      --arg current "$current_target" \
      --arg target "$previous_target" \
      '{schema: $schema, status: $status, reason: $reason, method: $method, previous_current_system: $current, target_system: $target}' \
      > "$result_json"
    cat "$result_json"
    return 1
  fi

  if ! AIRLOCK_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
    AIRLOCK_HOST_CURRENT_SYSTEM_LINK="$HOST_CURRENT_SYSTEM_LINK" \
    "$previous_target/bin/switch-to-configuration" switch > "$switch_log" 2>&1; then
    rc="$?"
  fi

  if [ "$rc" = 0 ]; then
    printf '%s\n' "$previous_target" > "$SYSTEM_STATE_DIR/current-host-system"
    printf '%s\n' "$HOST_SYSTEM_PROFILE" > "$SYSTEM_STATE_DIR/current-host-system-profile"
    printf 'switch\n' > "$SYSTEM_STATE_DIR/current-host-system-switch-action"
    jq -c -n \
      --arg schema "airlock.config-rollback.v1" \
      --arg status "passed" \
      --arg reason "rolled back host system profile to previous generation" \
      --arg method "$method" \
      --arg current "$current_target" \
      --arg target "$previous_target" \
      --arg switch_log "$switch_log" \
      '{schema: $schema, status: $status, reason: $reason, method: $method, previous_current_system: $current, target_system: $target, switch_log: $switch_log}' \
      > "$result_json"
  else
    jq -c -n \
      --arg schema "airlock.config-rollback.v1" \
      --arg status "failed" \
      --arg reason "switch-to-configuration failed for previous generation" \
      --arg method "$method" \
      --arg current "$current_target" \
      --arg target "$previous_target" \
      --arg switch_log "$switch_log" \
      '{schema: $schema, status: $status, reason: $reason, method: $method, previous_current_system: $current, target_system: $target, switch_log: $switch_log}' \
      > "$result_json"
  fi

  cat "$result_json"
  return "$rc"
}

case "$operation" in
  status)
    require_command airlock-config-status
    exec airlock-config-status
    ;;
  check)
    require_command awk
    require_command date
    require_command jq
    require_command mktemp
    require_command airlock-config-archive-candidate
    require_command tar
    read_candidate_tar
    run_check "$candidate_tar"
    ;;
  apply)
    require_command awk
    require_command date
    require_command jq
    require_command mktemp
    require_command airlock-config-apply
    require_command tar
    read_candidate_tar
    run_apply "$candidate_tar"
    ;;
  rollback)
    require_command date
    require_command jq
    require_command nix-env
    require_command readlink
    require_command sed
    rollback_host_system
    ;;
  --help|-h|"")
    usage
    ;;
  *)
    usage
    ;;
esac
