#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${AIRLOCK_LOG_DIR:-/var/log/airlock}"
RESULT_JSON="$LOG_DIR/config-boot-verify.json"
STATUS_JSON="$LOG_DIR/config-boot-verify-status.json"
MARKER_LOG="$LOG_DIR/config-boot-verify-marker.log"
EXPECTED_HOST_SYSTEM_SOURCE="${AIRLOCK_EXPECTED_HOST_SYSTEM_SOURCE:-}"
EXPECTED_MESSAGE="${AIRLOCK_EXPECTED_APPLIED_MESSAGE:-}"

mkdir -p "$LOG_DIR"

json_string() {
  printf '%s' "$1" | jq -Rs . | tr -d '\n'
}

json_file_string_or_null() {
  local file="$1"

  if [ -r "$file" ]; then
    jq -Rs . < "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

json_file_value_or_null() {
  local file="$1"

  if [ -r "$file" ] && jq -e . "$file" >/dev/null 2>&1; then
    jq -c . "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

write_result() {
  local status="$1"
  local reason="$2"
  local host_profile_target=""
  local run_current_target=""
  local pending_reboot="null"
  local last_apply_source=""

  if [ -r "$STATUS_JSON" ]; then
    host_profile_target="$(jq -r '.system.host_system_profile_resolved_target // ""' "$STATUS_JSON")"
    run_current_target="$(jq -r '.system.run_current_system_resolved_target // ""' "$STATUS_JSON")"
    pending_reboot="$(jq -c '.pending_reboot // null' "$STATUS_JSON")"
    last_apply_source="$(jq -r '.system.last_apply.host_system_source // ""' "$STATUS_JSON")"
  fi

  {
    printf '{'
    printf '"schema":"airlock.config-boot-verify.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"reason":'
    json_string "$reason"
    printf ',"expected_host_system_source":'
    json_string "$EXPECTED_HOST_SYSTEM_SOURCE"
    printf ',"last_apply_host_system_source":'
    json_string "$last_apply_source"
    printf ',"host_system_profile_resolved_target":'
    json_string "$host_profile_target"
    printf ',"run_current_system_resolved_target":'
    json_string "$run_current_target"
    printf ',"pending_reboot":%s' "$pending_reboot"
    printf ',"status_json":'
    json_file_value_or_null "$STATUS_JSON"
    printf ',"marker":'
    json_file_string_or_null "$MARKER_LOG"
    printf '}'
    printf '\n'
  } > "$RESULT_JSON"
}

fail() {
  local reason="$1"

  write_result failed "$reason"
  printf 'config boot verify failed: %s\n' "$reason" >&2
  exit 1
}

airlock-config-status > "$STATUS_JSON" || fail "airlock-config-status failed"
jq -e '.schema == "airlock.config-status.v1"' "$STATUS_JSON" >/dev/null ||
  fail "config status JSON is invalid"

if [ -z "$EXPECTED_HOST_SYSTEM_SOURCE" ]; then
  EXPECTED_HOST_SYSTEM_SOURCE="$(jq -r '.system.last_apply.host_system_source // ""' "$STATUS_JSON")"
fi

[ -n "$EXPECTED_HOST_SYSTEM_SOURCE" ] ||
  fail "expected host system source is empty"

host_profile_target="$(jq -r '.system.host_system_profile_resolved_target // ""' "$STATUS_JSON")"
run_current_target="$(jq -r '.system.run_current_system_resolved_target // ""' "$STATUS_JSON")"
pending_reboot_required="$(
  jq -r 'if (.pending_reboot.required | type) == "boolean" then .pending_reboot.required else true end' "$STATUS_JSON"
)"

[ "$host_profile_target" = "$EXPECTED_HOST_SYSTEM_SOURCE" ] ||
  fail "host system profile does not point at expected generation"
[ "$run_current_target" = "$EXPECTED_HOST_SYSTEM_SOURCE" ] ||
  fail "running system does not point at expected generation"
[ "$pending_reboot_required" = false ] ||
  fail "config status still reports a pending reboot"

if [ -r /etc/airlock/inside-config-applied-message ]; then
  cat /etc/airlock/inside-config-applied-message > "$MARKER_LOG"
fi

if [ -n "$EXPECTED_MESSAGE" ]; then
  grep -qx "$EXPECTED_MESSAGE" "$MARKER_LOG" ||
    fail "inside-config marker did not match expected message"
fi

write_result passed "running dom0 booted into the staged NixOS generation"
cat "$RESULT_JSON"
