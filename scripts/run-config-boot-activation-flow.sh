#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
mkdir -p "$LOG_DIR"

STAGE_JSON="$LOG_DIR/vm-config-boot-activate-stage.json"
VERIFY_JSON="$LOG_DIR/vm-config-boot-activate-verify.json"
RESULT_JSON="$LOG_DIR/vm-config-boot-activate-flow.json"
CONFIG_VMS_JSON="$LOG_DIR/vm-gui-config-vms-smoke.json"
CONFIG_BOOT_VERIFY_JSON="$LOG_DIR/vm-gui-config-boot-verify.json"

json_string() {
  printf '%s' "$1" | jq -Rs . | tr -d '\n'
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
  local expected="${3:-}"

  {
    printf '{'
    printf '"schema":"nixome.config-boot-activate-flow.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"reason":'
    json_string "$reason"
    printf ',"expected_host_system_source":'
    json_string "$expected"
    printf ',"stage":'
    json_file_value_or_null "$STAGE_JSON"
    printf ',"verify":'
    json_file_value_or_null "$VERIFY_JSON"
    printf '}'
    printf '\n'
  } > "$RESULT_JSON"
}

fail() {
  local reason="$1"
  local expected="${2:-}"

  write_result failed "$reason" "$expected"
  printf 'config boot activation flow failed: %s\n' "$reason" >&2
  exit 1
}

printf 'config-boot-activate: stage next dom0 boot generation\n'
env \
  NIXOME_AUTO_GC_AFTER_RUN=0 \
  NIXOME_VM_IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-gui-config-image}" \
  NIXOME_VM_DISPLAY="${NIXOME_VM_DISPLAY:-none}" \
  NIXOME_VM_FRESH=1 \
  NIXOME_SHUTDOWN_VM_ON_SUCCESS=1 \
  NIXOME_BOOTSTRAP_QUBES=0 \
  NIXOME_RUN_SETUP_CONFIG=1 \
  NIXOME_RUN_CONFIG_VMS_SMOKE=1 \
  NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot \
  NIXOME_RUN_CONFIG_BOOT_VERIFY=0 \
  NIXOME_RUN_QUBES_SMOKE=0 \
  NIXOME_RUN_GUI_SMOKE=0 \
  "$ROOT/scripts/run-nested-gui-smoke.sh"

[ -r "$CONFIG_VMS_JSON" ] ||
  fail "stage run did not write config VMs result"
cp "$CONFIG_VMS_JSON" "$STAGE_JSON"

expected_host_system_source="$(jq -r '.dom0_system_apply.host_system_source // ""' "$STAGE_JSON")"
[ -n "$expected_host_system_source" ] ||
  fail "stage run did not report host system source"

jq -e '
  .schema == "nixome.config-vms-smoke.v1" and
  .status == "passed" and
  .dom0_system_apply.status == "passed" and
  .dom0_system_apply.host_system_switch_action == "boot" and
  .dom0_system_apply.reboot_required == true and
  .config_status.pending_reboot.required == true
' "$STAGE_JSON" >/dev/null ||
  fail "stage run did not leave a pending boot generation" "$expected_host_system_source"

printf 'config-boot-activate: boot staged dom0 generation\n'
env \
  NIXOME_AUTO_GC_AFTER_RUN=1 \
  NIXOME_VM_IMAGE_ATTR="${NIXOME_VM_IMAGE_ATTR:-nixome-vm-gui-config-image}" \
  NIXOME_VM_DISPLAY="${NIXOME_VM_DISPLAY:-none}" \
  NIXOME_VM_FRESH=0 \
  NIXOME_SHUTDOWN_VM_ON_SUCCESS=1 \
  NIXOME_BOOTSTRAP_QUBES=0 \
  NIXOME_RUN_SETUP_CONFIG=0 \
  NIXOME_RUN_CONFIG_VMS_SMOKE=0 \
  NIXOME_RUN_CONFIG_BOOT_VERIFY=1 \
  NIXOME_EXPECTED_HOST_SYSTEM_SOURCE="$expected_host_system_source" \
  NIXOME_RUN_QUBES_SMOKE=0 \
  NIXOME_RUN_GUI_SMOKE=0 \
  "$ROOT/scripts/run-nested-gui-smoke.sh"

[ -r "$CONFIG_BOOT_VERIFY_JSON" ] ||
  fail "verify run did not write boot verify result" "$expected_host_system_source"
cp "$CONFIG_BOOT_VERIFY_JSON" "$VERIFY_JSON"

jq -e \
  --arg expected "$expected_host_system_source" \
  '
    .schema == "nixome.config-boot-verify.v1" and
    .status == "passed" and
    .expected_host_system_source == $expected and
    .host_system_profile_resolved_target == $expected and
    .run_current_system_resolved_target == $expected and
    .pending_reboot.required == false
  ' "$VERIFY_JSON" >/dev/null ||
  fail "verify run did not boot into staged dom0 generation" "$expected_host_system_source"

write_result passed "staged dom0 generation became the running boot generation" "$expected_host_system_source"
jq -c '{
  schema,
  status,
  reason,
  expected_host_system_source,
  stage_status: .stage.status,
  verify_status: .verify.status
}' "$RESULT_JSON"
