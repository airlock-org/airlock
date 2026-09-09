#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/nix-closure-domain-smoke.json"
COMMAND_LOG="$LOG_DIR/nix-closure-domain-smoke.commands.log"
STAGING_UPDATE_SCRIPT="$LOG_DIR/nix-closure-domain-staging-update.sh"
STAGING_UPDATE_LOG="$LOG_DIR/nix-closure-domain-staging-update.log"
APPLY_LOG="$LOG_DIR/nix-closure-domain-apply.log"
APPLY_RESULT_JSON="$LOG_DIR/nix-closure-domain-config-apply.json"
QVM_LS_LOG="$LOG_DIR/nix-closure-domain-qvm-ls.log"
QVM_PREFS_LOG="$LOG_DIR/nix-closure-domain-qvm-prefs.log"
QVM_FEATURES_LOG="$LOG_DIR/nix-closure-domain-qvm-features.log"
QVM_VOLUMES_LOG="$LOG_DIR/nix-closure-domain-qvm-volumes.log"
ROOT_VOLUME_LOG="$LOG_DIR/nix-closure-domain-root-volume.log"
STORE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-store-volume.log"
PRIVATE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-private-volume.log"
VOLATILE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-volatile-volume.log"
KERNEL_OPTS_LOG="$LOG_DIR/nix-closure-domain-kernelopts.log"
ROOT_IMAGE_CHECK_LOG="$LOG_DIR/nix-closure-domain-root-image-check.log"
STORE_VIEW_CHECK_LOG="$LOG_DIR/nix-closure-domain-store-view-check.log"
RUNTIME_SERVICE_LOG="$LOG_DIR/nix-closure-domain-runtime-service.log"
RUNTIME_SERVICE_ERR="$LOG_DIR/nix-closure-domain-runtime-service.err.log"
PEER_RUNTIME_SERVICE_LOG="$LOG_DIR/nix-closure-domain-peer-runtime-service.log"
PEER_RUNTIME_SERVICE_ERR="$LOG_DIR/nix-closure-domain-peer-runtime-service.err.log"
GUEST_PROBE_SCRIPT="$LOG_DIR/nix-closure-domain-guest-probe.sh"
GUEST_PROBE_LOG="$LOG_DIR/nix-closure-domain-guest-probe.log"
GUEST_PROBE_AFTER_REBOOT_LOG="$LOG_DIR/nix-closure-domain-guest-probe-after-reboot.log"
GUEST_PROBE_AFTER_RESTART_LOG="$LOG_DIR/nix-closure-domain-guest-probe-after-restart.log"
GUEST_PERSISTENCE_LOG="$LOG_DIR/nix-closure-domain-persistence.log"
REBOOT_LOG="$LOG_DIR/nix-closure-domain-reboot.log"
SHUTDOWN_LOG="$LOG_DIR/nix-closure-domain-shutdown.log"
RESTART_LOG="$LOG_DIR/nix-closure-domain-restart.log"
GUI_LOG="$LOG_DIR/nix-closure-domain-gui.log"
CONSOLE_LOG="$LOG_DIR/nix-closure-domain-console.log"
JOURNAL_LOG="$LOG_DIR/nix-closure-domain-journal.log"
LIBXL_LOG="$LOG_DIR/nix-closure-domain-libxl.log"
APPLIED_TOPOLOGY_LOG="$LOG_DIR/nix-closure-domain-applied-topology.json"
MATERIALIZATIONS_LOG="$LOG_DIR/nix-closure-domain-materializations.json"
BUILDS_LOG="$LOG_DIR/nix-closure-domain-builds.json"
SETUP_LOG="$LOG_DIR/nix-closure-domain-setup.json"
POLICY_LOG="$LOG_DIR/nix-closure-domain-policy.log"
QUBES_XML_LOG="$LOG_DIR/nix-closure-domain-qubes.xml"
PEER_QVM_FEATURES_LOG="$LOG_DIR/nix-closure-domain-peer-qvm-features.log"
PEER_STORE_VOLUME_LOG="$LOG_DIR/nix-closure-domain-peer-store-volume.log"
PEER_STORE_DATA_VOLUME_LOG="$LOG_DIR/nix-closure-domain-peer-store-data-volume.log"
PEER_GUEST_PROBE_LOG="$LOG_DIR/nix-closure-domain-peer-guest-probe.log"
PEER_VISIBILITY_LOG="$LOG_DIR/nix-closure-domain-peer-visibility.log"
DOMAIN_VISIBILITY_LOG="$LOG_DIR/nix-closure-domain-visibility.log"
LIBVIRT_XML_LOG="$LOG_DIR/nix-closure-domain-libvirt.xml"
PEER_LIBVIRT_XML_LOG="$LOG_DIR/nix-closure-domain-peer-libvirt.xml"
SHARING_PROOF_LOG="$LOG_DIR/nix-closure-domain-sharing-proof.json"
GEN2_STAGING_UPDATE_SCRIPT="$LOG_DIR/nix-closure-domain-staging-update-gen2.sh"
GEN2_STAGING_UPDATE_LOG="$LOG_DIR/nix-closure-domain-staging-update-gen2.log"
GEN2_APPLY_LOG="$LOG_DIR/nix-closure-domain-apply-gen2.log"
GEN2_APPLY_RESULT_JSON="$LOG_DIR/nix-closure-domain-config-apply-gen2.json"
GEN2_SHARING_PROOF_LOG="$LOG_DIR/nix-closure-domain-sharing-proof-gen2.json"
PRE_APPLY_SHUTDOWN_LOG="$LOG_DIR/nix-closure-domain-pre-apply-shutdown.log"
DIRECT_TOPOLOGY_LOG="$LOG_DIR/nix-closure-domain-direct-topology.json"
DIRECT_GEN2_TOPOLOGY_LOG="$LOG_DIR/nix-closure-domain-direct-topology-gen2.json"
RUNNING_STATUS_LOG="$LOG_DIR/nix-closure-domain-running-status.json"
GC_RUNNING_LOG="$LOG_DIR/nix-closure-domain-gc-running.json"
ROLLBACK_SWITCH_LOG="$LOG_DIR/nix-closure-domain-rollback-switch.json"
ROLLBACK_IDENTITY_LOG="$LOG_DIR/nix-closure-domain-rollback-identity.log"
ROLLBACK_GUEST_PROBE_LOG="$LOG_DIR/nix-closure-domain-guest-probe-after-rollback.log"
RETENTION_LOG="$LOG_DIR/nix-closure-domain-set-retention-gen2.json"
GC_AFTER_ROLLBACK_LOG="$LOG_DIR/nix-closure-domain-gc-after-rollback.json"

DOMAIN="${NIXOME_NIX_CLOSURE_DOMAIN:-nixome-closure-smoke}"
PEER_DOMAIN="${NIXOME_NIX_CLOSURE_PEER_DOMAIN:-${DOMAIN}-peer}"
APPLY_MODE="${NIXOME_NIX_CLOSURE_APPLY_MODE:-direct}"
DIRECT_PRIMARY_SOURCE_VM="${NIXOME_NIX_CLOSURE_DIRECT_PRIMARY_SOURCE_VM:-config-staging}"
DIRECT_PEER_SOURCE_VM="${NIXOME_NIX_CLOSURE_DIRECT_PEER_SOURCE_VM:-config-validator}"
DIRECT_GEN2_SOURCE_VM="${NIXOME_NIX_CLOSURE_DIRECT_GEN2_SOURCE_VM:-config-vault}"
STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
APPLY_MESSAGE="${NIXOME_NIX_CLOSURE_APPLY_MESSAGE:-nix-closure-smoke-$(date -u +%Y%m%dT%H%M%SZ)}"
APPLY_MESSAGE_GEN2="${NIXOME_NIX_CLOSURE_APPLY_MESSAGE_GEN2:-$APPLY_MESSAGE-gen2}"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"
WAIT_SECONDS="${NIXOME_NIX_CLOSURE_WAIT_SECONDS:-240}"
QREXEC_TIMEOUT_SECONDS="${NIXOME_NIX_CLOSURE_QREXEC_TIMEOUT_SECONDS:-45}"
RUN_GUI_PROBE="${NIXOME_NIX_CLOSURE_GUI_PROBE:-0}"
RUN_REBOOT_PROBE="${NIXOME_NIX_CLOSURE_REBOOT_PROBE:-0}"
CONFIG_VM_TEST_MEMORY="${NIXOME_NIX_CLOSURE_CONFIG_VM_MEMORY:-512}"
CONFIG_VM_TEST_MAXMEM="${NIXOME_NIX_CLOSURE_CONFIG_VM_MAXMEM:-512}"
VALIDATOR_TEST_MEMORY="${NIXOME_NIX_CLOSURE_VALIDATOR_MEMORY:-1024}"
VALIDATOR_TEST_MAXMEM="${NIXOME_NIX_CLOSURE_VALIDATOR_MAXMEM:-1024}"
CONFIG_APPLY_NIX_OFFLINE="${NIXOME_CONFIG_APPLY_NIX_OFFLINE:-0}"
SETUP_CONFIG_COMMAND="${NIXOME_SETUP_CONFIG_COMMAND:-nixome-setup-config}"

result_written=0
root_source=""
root_source_physical=""
store_source=""
store_source_physical=""
store_view_image=""
store_view_image_physical=""
store_mode=""
system_source=""
store_device=""
store_view_backend=""
store_filesystem=""
store_blob_device=""
store_blob_map=""
store_blob_map_physical=""
EXPECTED_DOMAIN_ROLE="${NIXOME_NIX_CLOSURE_EXPECTED_ROLE:-nix-closure-smoke}"
reboot_status="not-run"
shutdown_start_status="not-run"
gui_status="skipped"

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"
exec 3>&1
exec >> "$COMMAND_LOG" 2>&1
trap write_unexpected_failure EXIT

json_file_string_or_null() {
  file="$1"

  if [ -r "$file" ]; then
    jq -Rs . < "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

json_file_value_or_null() {
  file="$1"

  if [ -r "$file" ] && jq -e . "$file" >/dev/null 2>&1; then
    jq -c . "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

shell_literal() {
  printf "'"
  printf '%s' "$1" | sed "s/'/'\\\\''/g"
  printf "'"
}

write_result() {
  status="$1"
  reason="$2"

  {
    printf '{'
    printf '"schema":"nixome.nix-closure-domain-smoke.v1"'
    printf ',"status":'
    jq -Rn --arg value "$status" '$value' | tr -d '\n'
    printf ',"reason":'
    jq -Rn --arg value "$reason" '$value' | tr -d '\n'
    printf ',"domain":'
    jq -Rn --arg value "$DOMAIN" '$value' | tr -d '\n'
    printf ',"peer_domain":'
    jq -Rn --arg value "$PEER_DOMAIN" '$value' | tr -d '\n'
    printf ',"staging":'
    jq -Rn --arg value "$STAGING" '$value' | tr -d '\n'
    printf ',"validator":'
    jq -Rn --arg value "$VALIDATOR" '$value' | tr -d '\n'
    printf ',"vault":'
    jq -Rn --arg value "$VAULT" '$value' | tr -d '\n'
    printf ',"apply_message":'
    jq -Rn --arg value "$APPLY_MESSAGE" '$value' | tr -d '\n'
    printf ',"apply_message_gen2":'
    jq -Rn --arg value "$APPLY_MESSAGE_GEN2" '$value' | tr -d '\n'
    printf ',"apply_mode":'
    jq -Rn --arg value "$APPLY_MODE" '$value' | tr -d '\n'
    printf ',"system_source":'
    jq -Rn --arg value "$system_source" '$value' | tr -d '\n'
    printf ',"root_source":'
    jq -Rn --arg value "$root_source" '$value' | tr -d '\n'
    printf ',"root_source_physical":'
    jq -Rn --arg value "$root_source_physical" '$value' | tr -d '\n'
    printf ',"store_source":'
    jq -Rn --arg value "$store_source" '$value' | tr -d '\n'
    printf ',"store_source_physical":'
    jq -Rn --arg value "$store_source_physical" '$value' | tr -d '\n'
    printf ',"store_view_image":'
    jq -Rn --arg value "$store_view_image" '$value' | tr -d '\n'
    printf ',"store_view_image_physical":'
    jq -Rn --arg value "$store_view_image_physical" '$value' | tr -d '\n'
    printf ',"store_mode":'
    jq -Rn --arg value "$store_mode" '$value' | tr -d '\n'
    printf ',"store_device":'
    jq -Rn --arg value "$store_device" '$value' | tr -d '\n'
    printf ',"reboot_status":'
    jq -Rn --arg value "$reboot_status" '$value' | tr -d '\n'
    printf ',"shutdown_start_status":'
    jq -Rn --arg value "$shutdown_start_status" '$value' | tr -d '\n'
    printf ',"gui_status":'
    jq -Rn --arg value "$gui_status" '$value' | tr -d '\n'
    printf ',"logs":{'
    printf '"command":'
    jq -Rn --arg value "$COMMAND_LOG" '$value' | tr -d '\n'
    printf ',"staging_update":'
    jq -Rn --arg value "$STAGING_UPDATE_LOG" '$value' | tr -d '\n'
    printf ',"apply":'
    jq -Rn --arg value "$APPLY_LOG" '$value' | tr -d '\n'
    printf ',"apply_result":'
    jq -Rn --arg value "$APPLY_RESULT_JSON" '$value' | tr -d '\n'
    printf ',"gen2_staging_update":'
    jq -Rn --arg value "$GEN2_STAGING_UPDATE_LOG" '$value' | tr -d '\n'
    printf ',"gen2_apply":'
    jq -Rn --arg value "$GEN2_APPLY_LOG" '$value' | tr -d '\n'
    printf ',"gen2_apply_result":'
    jq -Rn --arg value "$GEN2_APPLY_RESULT_JSON" '$value' | tr -d '\n'
    printf ',"qvm_ls":'
    jq -Rn --arg value "$QVM_LS_LOG" '$value' | tr -d '\n'
    printf ',"qvm_prefs":'
    jq -Rn --arg value "$QVM_PREFS_LOG" '$value' | tr -d '\n'
    printf ',"qvm_features":'
    jq -Rn --arg value "$QVM_FEATURES_LOG" '$value' | tr -d '\n'
    printf ',"qvm_volumes":'
    jq -Rn --arg value "$QVM_VOLUMES_LOG" '$value' | tr -d '\n'
    printf ',"root_volume":'
    jq -Rn --arg value "$ROOT_VOLUME_LOG" '$value' | tr -d '\n'
    printf ',"store_volume":'
    jq -Rn --arg value "$STORE_VOLUME_LOG" '$value' | tr -d '\n'
    printf ',"peer_store_volume":'
    jq -Rn --arg value "$PEER_STORE_VOLUME_LOG" '$value' | tr -d '\n'
    printf ',"kernelopts":'
    jq -Rn --arg value "$KERNEL_OPTS_LOG" '$value' | tr -d '\n'
    printf ',"root_image_check":'
    jq -Rn --arg value "$ROOT_IMAGE_CHECK_LOG" '$value' | tr -d '\n'
    printf ',"store_view_check":'
    jq -Rn --arg value "$STORE_VIEW_CHECK_LOG" '$value' | tr -d '\n'
    printf ',"runtime_service":'
    jq -Rn --arg value "$RUNTIME_SERVICE_LOG" '$value' | tr -d '\n'
    printf ',"peer_runtime_service":'
    jq -Rn --arg value "$PEER_RUNTIME_SERVICE_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe":'
    jq -Rn --arg value "$GUEST_PROBE_LOG" '$value' | tr -d '\n'
    printf ',"peer_guest_probe":'
    jq -Rn --arg value "$PEER_GUEST_PROBE_LOG" '$value' | tr -d '\n'
    printf ',"visibility":'
    jq -Rn --arg value "$DOMAIN_VISIBILITY_LOG" '$value' | tr -d '\n'
    printf ',"peer_visibility":'
    jq -Rn --arg value "$PEER_VISIBILITY_LOG" '$value' | tr -d '\n'
    printf ',"sharing_proof":'
    jq -Rn --arg value "$SHARING_PROOF_LOG" '$value' | tr -d '\n'
    printf ',"gen2_sharing_proof":'
    jq -Rn --arg value "$GEN2_SHARING_PROOF_LOG" '$value' | tr -d '\n'
    printf ',"running_status":'
    jq -Rn --arg value "$RUNNING_STATUS_LOG" '$value' | tr -d '\n'
    printf ',"gc_running":'
    jq -Rn --arg value "$GC_RUNNING_LOG" '$value' | tr -d '\n'
    printf ',"rollback_switch":'
    jq -Rn --arg value "$ROLLBACK_SWITCH_LOG" '$value' | tr -d '\n'
    printf ',"rollback_identity":'
    jq -Rn --arg value "$ROLLBACK_IDENTITY_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe_after_rollback":'
    jq -Rn --arg value "$ROLLBACK_GUEST_PROBE_LOG" '$value' | tr -d '\n'
    printf ',"retention":'
    jq -Rn --arg value "$RETENTION_LOG" '$value' | tr -d '\n'
    printf ',"gc_after_rollback":'
    jq -Rn --arg value "$GC_AFTER_ROLLBACK_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe_after_reboot":'
    jq -Rn --arg value "$GUEST_PROBE_AFTER_REBOOT_LOG" '$value' | tr -d '\n'
    printf ',"guest_probe_after_restart":'
    jq -Rn --arg value "$GUEST_PROBE_AFTER_RESTART_LOG" '$value' | tr -d '\n'
    printf ',"persistence":'
    jq -Rn --arg value "$GUEST_PERSISTENCE_LOG" '$value' | tr -d '\n'
    printf ',"console":'
    jq -Rn --arg value "$CONSOLE_LOG" '$value' | tr -d '\n'
    printf ',"journal":'
    jq -Rn --arg value "$JOURNAL_LOG" '$value' | tr -d '\n'
    printf ',"libvirt_xml":'
    jq -Rn --arg value "$LIBVIRT_XML_LOG" '$value' | tr -d '\n'
    printf ',"peer_libvirt_xml":'
    jq -Rn --arg value "$PEER_LIBVIRT_XML_LOG" '$value' | tr -d '\n'
    printf '}'
    printf ',"apply_result":'
    json_file_value_or_null "$APPLY_RESULT_JSON"
    printf ',"setup_result":'
    json_file_value_or_null "$SETUP_LOG"
    printf ',"nix_domain_builds":'
    json_file_value_or_null "$BUILDS_LOG"
    printf ',"nix_domain_materializations":'
    json_file_value_or_null "$MATERIALIZATIONS_LOG"
    printf ',"sharing_proof":'
    json_file_value_or_null "$SHARING_PROOF_LOG"
    printf ',"gen2_apply_result":'
    json_file_value_or_null "$GEN2_APPLY_RESULT_JSON"
    printf ',"gen2_sharing_proof":'
    json_file_value_or_null "$GEN2_SHARING_PROOF_LOG"
    printf ',"running_status":'
    json_file_value_or_null "$RUNNING_STATUS_LOG"
    printf ',"gc_running":'
    json_file_value_or_null "$GC_RUNNING_LOG"
    printf ',"rollback_switch":'
    json_file_value_or_null "$ROLLBACK_SWITCH_LOG"
    printf ',"gc_after_rollback":'
    json_file_value_or_null "$GC_AFTER_ROLLBACK_LOG"
    printf ',"runtime_service_output":'
    json_file_string_or_null "$RUNTIME_SERVICE_LOG"
    printf ',"guest_probe_output":'
    json_file_string_or_null "$GUEST_PROBE_LOG"
    printf '}'
    printf '\n'
  } > "$RESULT_JSON"
  result_written=1
}

collect_diagnostics() {
  qvm-ls --raw-data --fields NAME,STATE,CLASS,LABEL,TEMPLATE > "$QVM_LS_LOG" 2>&1 || true
  cp /var/lib/qubes/qubes.xml "$QUBES_XML_LOG" 2>/dev/null || true
  if qvm-check "$DOMAIN" >/dev/null 2>&1; then
    qvm-prefs "$DOMAIN" > "$QVM_PREFS_LOG" 2>&1 || true
    qvm-features "$DOMAIN" > "$QVM_FEATURES_LOG" 2>&1 || true
    qvm-volume list "$DOMAIN" > "$QVM_VOLUMES_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:root" > "$ROOT_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:private" > "$PRIVATE_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:volatile" > "$VOLATILE_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$DOMAIN:store" > "$STORE_VOLUME_LOG" 2>&1 || true
    qvm-prefs "$DOMAIN" kernelopts > "$KERNEL_OPTS_LOG" 2>&1 || true
  fi
  if qvm-check "$PEER_DOMAIN" >/dev/null 2>&1; then
    qvm-features "$PEER_DOMAIN" > "$PEER_QVM_FEATURES_LOG" 2>&1 || true
    qvm-volume info "$PEER_DOMAIN:store" > "$PEER_STORE_VOLUME_LOG" 2>&1 || true
    qvm-volume info "$PEER_DOMAIN:store-data" > "$PEER_STORE_DATA_VOLUME_LOG" 2>&1 || true
  fi
  tail -n 1000 "/var/log/xen/console/guest-$DOMAIN.log" > "$CONSOLE_LOG" 2>&1 || true
  journalctl -b --no-pager -n 320 \
    -u qubesd \
    -u libvirtd \
    -u qubes-db-dom0 \
    -u qubes-qrexec-policy-daemon \
    > "$JOURNAL_LOG" 2>&1 || true
  tail -n 320 /var/log/libvirt/libxl/libxl-driver.log > "$LIBXL_LOG" 2>&1 || true
  cat /var/log/nixome/config-apply-topology.json > "$APPLIED_TOPOLOGY_LOG" 2>/dev/null || true
  if [ "$APPLY_MODE" = "config-flow" ]; then
    cat /var/log/nixome/config-apply.json > "$APPLY_RESULT_JSON" 2>/dev/null || true
  fi
  cat /var/log/nixome/config-apply-nix-domain-materializations.json > "$MATERIALIZATIONS_LOG" 2>/dev/null || true
  cat /var/log/nixome/config-apply-nix-domain-builds.json > "$BUILDS_LOG" 2>/dev/null || true
  cat /var/log/nixome/setup-config.json > "$SETUP_LOG" 2>/dev/null || true
  cat /var/lib/nixome/qubes-policy.d/90-nixome.policy > "$POLICY_LOG" 2>/dev/null || true
}

fail() {
  reason="$1"
  collect_diagnostics
  write_result failed "$reason"
  printf 'nix-closure domain smoke failed: %s\n' "$reason" >&2
  cat "$RESULT_JSON" >&3
  exit 1
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    collect_diagnostics
    write_result failed "unexpected command failure; see command_log"
    cat "$RESULT_JSON" >&3
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

run_or_fail() {
  description="$1"
  output="$2"
  shift 2

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if ! "$@" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

run_json_or_fail() {
  description="$1"
  output="$2"
  shift 2
  stdout="$output.stdout"
  stderr="$output.stderr"

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if ! "$@" > "$stdout" 2> "$stderr"; then
    cat "$stderr" >&2 || true
    cat "$stdout" >&2 || true
    fail "$description failed"
  fi

  if ! python3 - "$stdout" "$output" <<'PY'
import json
import sys
from pathlib import Path

source = Path(sys.argv[1])
target = Path(sys.argv[2])
text = source.read_text()
decoder = json.JSONDecoder()

for index, char in enumerate(text):
    if char != "{":
        continue
    try:
        payload, _ = decoder.raw_decode(text[index:])
    except json.JSONDecodeError:
        continue
    if isinstance(payload, dict):
        target.write_text(json.dumps(payload, sort_keys=True) + "\n")
        raise SystemExit(0)

raise SystemExit("command did not emit a JSON object")
PY
  then
    cat "$stderr" >&2 || true
    cat "$stdout" >&2 || true
    fail "$description did not produce valid JSON"
  fi
}

run_qrexec_script_or_fail() {
  vm="$1"
  description="$2"
  output="$3"
  script="$4"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  printf '+'
  printf ' %q' "${qvm_run_args[@]}" "$vm" "bash -s"
  printf ' < %q\n' "$script"

  if ! "${qvm_run_args[@]}" "$vm" "bash -s" < "$script" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

qvm_run_probe() {
  vm="$1"
  output="$2"
  command="$3"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  timeout "$QREXEC_TIMEOUT_SECONDS" "${qvm_run_args[@]}" "$vm" "$command" > "$output" 2>&1
}

qvm_volume_list_has_volume() {
  vm="$1"
  volume="$2"

  qvm-volume list "$vm" |
    awk -v volume="$volume" '
      NR == 1 { next }
      $3 == volume || $1 ~ "/" volume "$" { found = 1 }
      END { exit found ? 0 : 1 }
    '
}

wait_for_qrexec() {
  vm="$1"
  label="$2"
  output="$3"
  deadline=$((SECONDS + WAIT_SECONDS))

  while [ "$SECONDS" -lt "$deadline" ]; do
    if qvm_run_probe "$vm" "$output" "printf qrexec-ready"; then
      grep -q qrexec-ready "$output" && return 0
    fi
    sleep 3
  done

  cat "$output" >&2 || true
  fail "qrexec did not become ready for $label within ${WAIT_SECONDS}s"
}

start_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
  else
    run_or_fail "qvm-start $vm" "$LOG_DIR/nix-closure-domain-$vm-start.log" qvm-start "$vm"
  fi
}

shutdown_vm() {
  vm="$1"
  output="$2"

  if ! qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already halted: %s\n' "$vm" > "$output"
    return 0
  fi

  run_or_fail "qvm-shutdown $vm" "$output" qvm-shutdown --wait "$vm"
}

nixome_store_uses_chroot() {
  [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] &&
    [ -n "${NIXOME_NIX_ROOT:-}" ] &&
    [ "$NIXOME_NIX_ROOT" != "/" ]
}

nixome_physical_store_path() {
  path="$1"

  if nixome_store_uses_chroot && [ "${path#/nix/store/}" != "$path" ]; then
    printf '%s%s\n' "$NIXOME_NIX_ROOT" "$path"
  else
    printf '%s\n' "$path"
  fi
}

extract_kernelopt() {
  key="$1"
  tr ' ' '\n' < "$KERNEL_OPTS_LOG" |
    sed -n "s|^$key=||p" |
    head -n 1
}

feature_value() {
  qvm-features "$DOMAIN" "$1" 2>/dev/null | tr -d '\n'
}

feature_value_for() {
  vm="$1"
  feature="$2"
  qvm-features "$vm" "$feature" 2>/dev/null | tr -d '\n'
}

store_blob_map_physical_for() {
  vm="$1"
  blob_map="$(feature_value_for "$vm" nixome-store-blob-map)"
  [ -n "$blob_map" ] || fail "missing nixome-store-blob-map feature for $vm"
  nixome_physical_store_path "$blob_map"
}

store_view_backend_for() {
  vm="$1"
  backend="$(feature_value_for "$vm" nixome-store-view-backend)"
  [ -n "$backend" ] || fail "missing nixome-store-view-backend feature for $vm"
  printf '%s\n' "$backend"
}

store_generation_for() {
  vm="$1"
  generation="$(feature_value_for "$vm" nixome-store-generation)"
  [ -n "$generation" ] || fail "missing nixome-store-generation feature for $vm"
  printf '%s\n' "$generation"
}

write_sharing_proof() {
  label="$1"
  left_vm="$2"
  right_vm="$3"
  left_map="$4"
  right_map="$5"
  output="$6"
  python3 - "$label" "$left_vm" "$right_vm" "$left_map" "$right_map" "$output" <<'PY'
import json
import os
import sys
from pathlib import Path

label, left_vm, right_vm, left_path, right_path, output_path = sys.argv[1:]

def load(path):
    data = json.loads(Path(path).read_text())
    if data.get("view_backend") != "erofs-objectmap":
        raise SystemExit(f"{path} is not an erofs-objectmap view")
    if data.get("external_data_format") != "posix-tar-index":
        raise SystemExit(f"{path} is not a tar-index EROFS external-data view")
    extents_by_object = {}
    for extent in data.get("extents", []):
        obj = extent.get("object")
        if obj == ".tar-eof":
            continue
        if obj:
            extents_by_object.setdefault(obj, []).append(extent)
    if not extents_by_object:
        raise SystemExit(f"{path} has no object extents")
    return data, extents_by_object

left, left_extents = load(left_path)
right, right_extents = load(right_path)
if "central_blob" in left or "central_blob" in right:
    raise SystemExit("closure views use obsolete single-blob metadata")

common = sorted(set(left_extents) & set(right_extents))
if not common:
    raise SystemExit("closure views do not share any canonical objects")

selected = None
for obj in common:
    a = left_extents[obj]
    b = right_extents[obj]
    if len(a) == len(b) and all(
        x.get("segment_id") == y.get("segment_id")
        and int(x["segment_offset"]) == int(y["segment_offset"])
        and int(x["length"]) == int(y["length"])
        and x.get("object_sha256") == y.get("object_sha256")
        for x, y in zip(a, b)
    ):
        selected = obj
        break
if selected is None:
    raise SystemExit("common objects do not retain stable canonical extents")

left_unique = sorted(set(left_extents) - set(right_extents))
right_unique = sorted(set(right_extents) - set(left_extents))
if not left_unique or not right_unique:
    raise SystemExit("expected each closure to contain at least one unique object")

payload = {
    "schema": "nixome.nix-closure-domain-sharing-proof.v1",
    "status": "passed",
    "label": label,
    "left_domain": left_vm,
    "right_domain": right_vm,
    "left_blob_map": left_path,
    "right_blob_map": right_path,
    "left_segments": left.get("segments", []),
    "right_segments": right.get("segments", []),
    "unique_segment_count": len({item["blob"] for item in left.get("segments", []) + right.get("segments", [])}),
    "segment_physical_size_total": sum(
        os.path.getsize(path)
        for path in {item["blob"] for item in left.get("segments", []) + right.get("segments", [])}
    ),
    "common_object_count": len(common),
    "left_unique_object_count": len(left_unique),
    "right_unique_object_count": len(right_unique),
    "shared_object": selected,
    "shared_extents": left_extents[selected],
    "left_unique_object": left_unique[0],
    "right_unique_object": right_unique[0],
    "left_logical_size": int(left["logical_size"]),
    "right_logical_size": int(right["logical_size"]),
    "same_canonical_segment_offset_and_hash": True,
}
Path(output_path).write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
PY
}

sharing_json_value() {
  file="$1"
  key="$2"
  jq -r "$key" "$file"
}

closure_segment_bytes_total() {
  nixome-closure-store status | jq -r '.segment_bytes_total'
}

set_domain_generation_identity_from_switch() {
  switch_json="$1"
  selected_system="$(jq -r '.system' "$switch_json")"
  selected_root="$(jq -r '.root_source // ""' "$switch_json")"
  selected_kernel="$(jq -r '.kernel_name // ""' "$switch_json")"
  selected_generation="$(jq -r '.generation' "$switch_json")"
  selected_source="$(jq -r '.source // ""' "$switch_json")"
  selected_closure_hash="$(jq -r '.closure_hash' "$switch_json")"
  selected_view_image="$(jq -r '.view_image // .metadata_image // ""' "$switch_json")"
  selected_backend="$(jq -r '.view_backend // "erofs-objectmap"' "$switch_json")"
  selected_filesystem="$(jq -r '.filesystem // "erofs"' "$switch_json")"
  selected_blob_map="$(jq -r '.blob_map // ""' "$switch_json")"
  selected_blob_volume="$(jq -r '.blob_volume // ""' "$switch_json")"
  selected_blob_device="$(jq -r '.blob_device // ""' "$switch_json")"
  [ "$selected_blob_device" != "null" ] || selected_blob_device=""
  [ -n "$selected_blob_device" ] || selected_blob_device="/dev/xvdf"

  {
    printf 'selected_system=%s\n' "$selected_system"
    printf 'selected_root=%s\n' "$selected_root"
    printf 'selected_kernel=%s\n' "$selected_kernel"
    printf 'selected_generation=%s\n' "$selected_generation"
    printf 'selected_source=%s\n' "$selected_source"
    printf 'selected_view_image=%s\n' "$selected_view_image"

    if [ -n "$selected_root" ] && [ "$selected_root" != "null" ]; then
      selected_root_physical="$(nixome_physical_store_path "$selected_root")"
      if [ -d "$selected_root_physical" ]; then
        selected_root_image="$selected_root_physical/root.img"
      else
        selected_root_image="$selected_root_physical"
      fi
      [ -f "$selected_root_image" ] ||
        fail "rollback root image is missing: $selected_root"
      printf '+ qvm-volume import %q %q\n' "$DOMAIN:root" "$selected_root_image"
      qvm-volume import "$DOMAIN:root" "$selected_root_image"
    fi

    if [ -n "$selected_kernel" ] && [ "$selected_kernel" != "null" ]; then
      printf '+ qvm-prefs %q kernel %q\n' "$DOMAIN" "$selected_kernel"
      qvm-prefs "$DOMAIN" kernel "$selected_kernel"
    fi

    selected_kernelopts="console=hvc0 init=/sbin/init nixome.system=$selected_system nixome.storedev=/dev/xvde nixome.storefmt=$selected_filesystem"
    if [ "$selected_filesystem" = "erofs" ]; then
      selected_kernelopts="$selected_kernelopts nixome.storeblobdev=$selected_blob_device"
    fi
    printf '+ qvm-prefs %q kernelopts %q\n' "$DOMAIN" "$selected_kernelopts"
    qvm-prefs "$DOMAIN" kernelopts "$selected_kernelopts"

    qvm-features "$DOMAIN" nixome-system-source "$selected_system"
    qvm-features "$DOMAIN" nixome-root-source "$selected_root"
    qvm-features "$DOMAIN" nixome-store-source "$selected_source"
    qvm-features "$DOMAIN" nixome-store-mode shared-closure
    qvm-features "$DOMAIN" nixome-store-view-backend "$selected_backend"
    qvm-features "$DOMAIN" nixome-store-filesystem "$selected_filesystem"
    qvm-features "$DOMAIN" nixome-store-generation "$selected_generation"
    qvm-features "$DOMAIN" nixome-store-closure-hash "$selected_closure_hash"
    qvm-features "$DOMAIN" nixome-store-view-image "$selected_view_image"
    qvm-features "$DOMAIN" nixome-store-mountpoint /nix/store
    qvm-features "$DOMAIN" nixome-store-device /dev/xvde
    qvm-features "$DOMAIN" nixome-store-read-only True
    if [ "$selected_filesystem" = "erofs" ]; then
      qvm-features "$DOMAIN" nixome-store-blob-device "$selected_blob_device"
      qvm-features "$DOMAIN" nixome-store-blob-volume "$selected_blob_volume"
      qvm-features "$DOMAIN" nixome-store-blob-map "$selected_blob_map"
    fi
  } > "$ROLLBACK_IDENTITY_LOG" 2>&1 ||
    fail "apply selected generation identity after rollback failed"
}

run_visibility_probe() {
  vm="$1"
  own_object="$2"
  other_object="$3"
  shared_object="$4"
  output="$5"
  script="$LOG_DIR/nix-closure-domain-$vm-visibility.sh"

  case "$own_object$other_object$shared_object" in
    *[!A-Za-z0-9+._?=-]*)
      fail "unsafe Nix store object name in visibility probe"
      ;;
  esac

  cat > "$script" <<EOF
set -eu
own_object='$own_object'
other_object='$other_object'
shared_object='$shared_object'
[ -e "/nix/store/\$shared_object" ]
[ -e "/nix/store/\$own_object" ]
if [ -e "/nix/store/\$other_object" ]; then
  echo "unexpected object visible: /nix/store/\$other_object" >&2
  exit 1
fi
findmnt -rn -T /nix/store -o OPTIONS | grep -E '(^|,)ro(,|$)'
printf 'visibility_probe=passed\n'
EOF

  run_qrexec_script_or_fail "$vm" "closure visibility probe in $vm" "$output" "$script"
  grep -Fx "visibility_probe=passed" "$output" >/dev/null ||
    fail "closure visibility probe did not complete for $vm"
}

check_libvirt_filtered_devices() {
  vm="$1"
  output="$2"
  xml_tmp="$output.tmp"

  if virsh -c xen:/// dumpxml "$vm" > "$output" 2>&1; then
    :
  elif virsh -c xen:/// dumpxml "domain-$vm" > "$output" 2>&1; then
    :
  else
    cat "$output" >&2 || true
    fail "read libvirt XML for $vm failed"
  fi
  cp "$output" "$xml_tmp"
  grep -E "<target dev=['\"]xvde['\"]" "$xml_tmp" >/dev/null ||
    fail "libvirt XML for $vm does not attach store metadata at xvde"
  grep -E "<target dev=['\"]xvdf['\"]" "$xml_tmp" >/dev/null ||
    fail "libvirt XML for $vm does not attach store data at xvdf"
  grep -F "/dev/mapper/nixome-" "$xml_tmp" >/dev/null ||
    fail "libvirt XML for $vm does not use a filtered /dev/mapper/nixome-* backing device"
  if grep -F "/var/lib/nixome/closure-store/objects/segments/" "$xml_tmp" >/dev/null; then
    fail "libvirt XML for $vm exposes canonical segment backing storage"
  fi
  rm -f "$xml_tmp"
}

set_test_vm_memory() {
  vm="$1"
  memory="$2"
  maxmem="$3"

  shutdown_vm "$vm" "$LOG_DIR/nix-closure-domain-$vm-memory-shutdown.log"
  run_or_fail "set $vm test memory" "$LOG_DIR/nix-closure-domain-$vm-memory.log" \
    qvm-prefs "$vm" memory "$memory"
  run_or_fail "set $vm test maxmem" "$LOG_DIR/nix-closure-domain-$vm-maxmem.log" \
    qvm-prefs "$vm" maxmem "$maxmem"
}

require_source_domain_artifacts() {
  vm="$1"

  for feature in \
    nixome-system-source \
    nixome-root-source \
    nixome-store-source \
    nixome-role; do
    [ -n "$(feature_value_for "$vm" "$feature")" ] ||
      fail "source domain $vm is missing feature $feature"
  done
  [ -n "$(qvm-prefs "$vm" kernel 2>/dev/null | tr -d '\n')" ] ||
    fail "source domain $vm has no kernel preference"
}

write_direct_topology() {
  output="$1"
  primary_source_vm="$2"
  peer_source_vm="$3"

  require_source_domain_artifacts "$primary_source_vm"
  require_source_domain_artifacts "$peer_source_vm"

  primary_system="$(feature_value_for "$primary_source_vm" nixome-system-source)"
  primary_root="$(feature_value_for "$primary_source_vm" nixome-root-source)"
  primary_store="$(feature_value_for "$primary_source_vm" nixome-store-source)"
  primary_kernel="$(qvm-prefs "$primary_source_vm" kernel 2>/dev/null | tr -d '\n')"
  primary_role="$(feature_value_for "$primary_source_vm" nixome-role)"
  peer_system="$(feature_value_for "$peer_source_vm" nixome-system-source)"
  peer_root="$(feature_value_for "$peer_source_vm" nixome-root-source)"
  peer_store="$(feature_value_for "$peer_source_vm" nixome-store-source)"
  peer_kernel="$(qvm-prefs "$peer_source_vm" kernel 2>/dev/null | tr -d '\n')"
  peer_role="$(feature_value_for "$peer_source_vm" nixome-role)"

  jq -n \
    --arg domain "$DOMAIN" \
    --arg peer "$PEER_DOMAIN" \
    --arg primary_system "$primary_system" \
    --arg primary_root "$primary_root" \
    --arg primary_store "$primary_store" \
    --arg primary_kernel "$primary_kernel" \
    --arg primary_role "$primary_role" \
    --arg peer_system "$peer_system" \
    --arg peer_root "$peer_root" \
    --arg peer_store "$peer_store" \
    --arg peer_kernel "$peer_kernel" \
    --arg peer_role "$peer_role" \
    --arg qube_user "$QUBE_USER" \
    '
      def nix_domain($label; $system; $root; $store; $kernel; $role):
        {
          class: "StandaloneVM",
          label: $label,
          netvm: null,
          template: null,
          prefs: {
            autostart: false,
            default_user: $qube_user,
            include_in_backups: false,
            kernel: $kernel,
            memory: 1024,
            maxmem: 1024,
            qrexec_timeout: 180,
            vcpus: 1,
            virt_mode: "pvh"
          },
          features: {
            "nixome-backend": "nix-closure",
            "nixome-role": $role,
            "nixome-test": "nix-closure-domain",
            os: "Linux",
            qrexec: "1"
          },
          services: {
            "network-manager": false
          },
          backend: {
            type: "nix-closure",
            template: null,
            system: {
              source: $system,
              source_ref: null
            },
            kernel: {
              name: $kernel,
              source: null,
              source_ref: null
            },
            initrd: {
              name: null,
              source: null,
              source_ref: null
            },
            modules: {
              name: null,
              source: null,
              source_ref: null
            },
            root: {
              mode: "qubes-root-image",
              source: $root,
              source_ref: null,
              read_only: false
            },
            store: {
              mode: "shared-closure",
              source: $store,
              source_ref: null,
              read_only: true,
              device: "/dev/xvde",
              blob_device: "/dev/xvdf",
              blobDevice: "/dev/xvdf",
              mountpoint: "/nix/store",
              view_backend: "erofs-objectmap",
              viewBackend: "erofs-objectmap",
              chunk_size: 65536,
              chunkSize: 65536
            },
            state: {
              persistent: null,
              volatile: null
            }
          }
        };

      {
        schema: "nixome.qubes-topology.v1",
        prefs: {},
        templates: {},
        kernels: {},
        qrexec_policy: {
          rules: [
            {
              service: "nixome.NixStoreRuntimeCheck",
              argument: "*",
              source: "dom0",
              target: $domain,
              action: "allow"
            },
            {
              service: "nixome.NixStoreRuntimeCheck",
              argument: "*",
              source: "dom0",
              target: $peer,
              action: "allow"
            },
            {
              service: "nixome.NixStoreRuntimeCheck",
              argument: "*",
              source: "@anyvm",
              target: "@anyvm",
              action: "deny"
            }
          ]
        },
        domains: {
          ($domain): nix_domain("green"; $primary_system; $primary_root; $primary_store; $primary_kernel; $primary_role),
          ($peer): nix_domain("blue"; $peer_system; $peer_root; $peer_store; $peer_kernel; $peer_role)
        }
      }
    ' > "$output"
}

write_direct_apply_result() {
  reason="$1"
  topology="${2:-$DIRECT_TOPOLOGY_LOG}"
  output="${3:-$APPLY_RESULT_JSON}"
  setup_json="$(json_file_value_or_null "$SETUP_LOG")"

  jq -c -n \
    --arg schema "nixome.config-apply.v1" \
    --arg status "passed" \
    --arg reason "$reason" \
    --arg domain "$DOMAIN" \
    --arg peer "$PEER_DOMAIN" \
    --arg topology "$topology" \
    --argjson setup "$setup_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      topology: $topology,
      setup: $setup,
      nix_domain_materializations_result: {
        domains: [
          { domain: $domain },
          { domain: $peer }
        ]
      }
    }' > "$output"
}

check_source_paths() {
  root_source="$(feature_value nixome-root-source)"
  store_source="$(feature_value nixome-store-source)"
  store_mode="$(feature_value nixome-store-mode)"
  store_view_backend="$(feature_value nixome-store-view-backend)"
  store_filesystem="$(feature_value nixome-store-filesystem)"
  store_blob_device="$(feature_value nixome-store-blob-device)"
  store_blob_map="$(feature_value nixome-store-blob-map)"
  [ -n "$store_mode" ] || store_mode="shared-closure"
  [ -n "$store_view_backend" ] || store_view_backend="erofs-objectmap"
  [ -n "$store_filesystem" ] || store_filesystem="erofs"
  store_view_image="$(feature_value nixome-store-view-image)"
  system_source="$(feature_value nixome-system-source)"
  store_device="$(feature_value nixome-store-device)"
  root_source_physical="$(nixome_physical_store_path "$root_source")"
  store_source_physical="$(nixome_physical_store_path "$store_source")"
  store_view_image_physical="$(nixome_physical_store_path "$store_view_image")"
  store_blob_map_physical="$(nixome_physical_store_path "$store_blob_map")"

  [ -n "$root_source" ] || fail "missing nixome-root-source feature"
  [ -n "$store_source" ] || fail "missing nixome-store-source feature"
  [ -n "$store_view_image" ] || fail "missing nixome-store-view-image feature"
  [ -n "$system_source" ] || fail "missing nixome-system-source feature"
  [ -n "$store_device" ] || fail "missing nixome-store-device feature"
  [ -f "$root_source_physical" ] || fail "root image source is missing: $root_source"
  [ "$store_mode" = "shared-closure" ] ||
    fail "unsupported store mode for $DOMAIN: $store_mode"
  [ "$store_view_backend" = "erofs-objectmap" ] ||
    fail "unsupported store view backend for $DOMAIN: $store_view_backend"
  [ "$store_filesystem" = "erofs" ] ||
    fail "shared closure objectmap backend did not report erofs filesystem"
  [ -d "$store_source_physical" ] || fail "store closure source is missing: $store_source"
  [ -f "$store_source_physical/store-paths" ] ||
    fail "store closure source is missing store-paths: $store_source/store-paths"
  [ -f "$store_source_physical/manifest.json" ] ||
    fail "store closure source is missing manifest.json: $store_source/manifest.json"
  [ -f "$store_view_image_physical" ] || fail "store view image path is missing: $store_view_image"
  [ -n "$store_blob_device" ] || fail "missing nixome-store-blob-device feature"
  [ -n "$store_blob_map" ] || fail "missing nixome-store-blob-map feature"
  [ -f "$store_blob_map_physical" ] || fail "store blob map path is missing: $store_blob_map"
}

check_root_image_has_no_store() {
  {
    printf 'root_source=%s\n' "$root_source"
    printf 'root_source_physical=%s\n' "$root_source_physical"
    printf 'bytes=%s\n' "$(stat -c %s "$root_source_physical")"
    debugfs -R 'ls -p /nix/store' "$root_source_physical"
  } > "$ROOT_IMAGE_CHECK_LOG" 2>&1

  entries="$(
    debugfs -R 'ls -p /nix/store' "$root_source_physical" 2>/dev/null |
      awk -F/ 'NF >= 6 && $6 != "." && $6 != ".." { print $6 }'
  )"

  [ -z "$entries" ] ||
    fail "root image contains unexpected /nix/store entries: $entries"
}

check_store_view_contains_system() {
  system_entry="${system_source#/nix/store/}"
  [ "$system_entry" != "$system_source" ] ||
    fail "system source is not in /nix/store: $system_source"

  {
    printf 'store_source=%s\n' "$store_source"
    printf 'store_source_physical=%s\n' "$store_source_physical"
    printf 'store_view_image=%s\n' "$store_view_image"
    printf 'store_view_image_physical=%s\n' "$store_view_image_physical"
    printf 'system_source=%s\n' "$system_source"
    printf 'system_entry=%s\n' "$system_entry"
    printf 'bytes=%s\n' "$(stat -c %s "$store_view_image_physical")"
    if [ -f "$store_source_physical/store-paths" ]; then
      grep -Fx "$system_source" "$store_source_physical/store-paths"
    elif [ -f "$(dirname "$store_source_physical")/store-paths" ]; then
      grep -Fx "$system_source" "$(dirname "$store_source_physical")/store-paths"
    fi
    generation="$(store_generation_for "$DOMAIN")"
    attach_json="$LOG_DIR/nix-closure-domain-store-attach.json"
    extract_dir="$LOG_DIR/nix-closure-domain-store-extract"
    chmod -R u+w "$extract_dir" 2>/dev/null || true
    rm -rf "$extract_dir"
    mkdir -p "$extract_dir"
    nixome-closure-store attach-blob-view \
      --domain "$DOMAIN" \
      --generation "$generation" \
      > "$attach_json"
    filtered_device="$(jq -r '.device' "$attach_json")"
    case "$filtered_device" in
      /dev/mapper/nixome-*) ;;
      *) echo "unexpected filtered device: $filtered_device" >&2; exit 1 ;;
    esac
    while IFS= read -r backing_path; do
      [ -n "$backing_path" ] || continue
      if [ "$(readlink -f "$filtered_device")" = "$(readlink -f "$backing_path")" ]; then
        echo "filtered device resolves to canonical segment backing storage" >&2
        exit 1
      fi
    done < <(jq -r '.backing_paths[]?' "$attach_json")
    set +e
    fsck.erofs --device="$filtered_device" --extract="$extract_dir" "$store_view_image_physical"
    fsck_rc="$?"
    nixome-closure-store detach-blob-view \
      --domain "$DOMAIN" \
      --generation "$generation"
    detach_rc="$?"
    set -e
    [ "$fsck_rc" = "0" ]
    [ "$detach_rc" = "0" ]
    [ -x "$extract_dir/$system_entry/init" ]
  } > "$STORE_VIEW_CHECK_LOG" 2>&1 ||
    fail "store view does not contain declared system init"
}

run_runtime_service_check_for() {
  vm="$1"
  expected_system="$2"
  expected_store_device="$3"
  expected_store_filesystem="$4"
  expected_store_blob_device="$5"
  output="$6"
  error_output="$7"
  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC nixome.NixStoreRuntimeCheck+ dom0")
  printf '+'
  printf ' %q' "${qrexec_args[@]}"
  printf '\n'

  if ! "${qrexec_args[@]}" > "$output" 2> "$error_output"; then
    cat "$error_output" >&2 || true
    fail "nixome.NixStoreRuntimeCheck qrexec service failed for $vm"
  fi

  grep -Fx "system=$expected_system" "$output" >/dev/null ||
    fail "runtime service did not report expected system source for $vm"
  grep -Fx "store_device=$expected_store_device" "$output" >/dev/null ||
    fail "runtime service did not report expected store device for $vm"
  grep -Fx "store_format=$expected_store_filesystem" "$output" >/dev/null ||
    fail "runtime service did not report expected store filesystem for $vm"
  if [ "$expected_store_filesystem" = "erofs" ]; then
    grep -Fx "store_blob_device=$expected_store_blob_device" "$output" >/dev/null ||
      fail "runtime service did not report expected store blob device for $vm"
  fi
  grep -E '^store_options=([^,]+,)*ro(,|$)' "$output" >/dev/null ||
    fail "runtime service did not report read-only store mount for $vm"
}

run_runtime_service_check() {
  run_runtime_service_check_for \
    "$DOMAIN" \
    "$system_source" \
    "$store_device" \
    "$store_filesystem" \
    "$store_blob_device" \
    "$RUNTIME_SERVICE_LOG" \
    "$RUNTIME_SERVICE_ERR"
}

write_guest_probe_script() {
  {
    cat <<'EOF'
set -eu
EOF
    printf 'expected_domain_role=%s\n' "$(shell_literal "$EXPECTED_DOMAIN_ROLE")"
    cat <<'EOF'

system_path="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.system=||p' |
    head -n 1
)"
store_device="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storedev=||p' |
    head -n 1
)"
store_format="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storefmt=||p' |
    head -n 1
)"
store_blob_device="$(
  tr ' ' '\n' < /proc/cmdline |
    sed -n 's|^nixome.storeblobdev=||p' |
    head -n 1
)"
[ -n "$store_format" ] || store_format="erofs"
mount_source="$(findmnt -rn -T /nix/store -o SOURCE)"
mount_options="$(findmnt -rn -T /nix/store -o OPTIONS)"
current_system="$(readlink -f /run/current-system)"
guest_user="$(id -un)"
home_dir="${HOME:-/home/$guest_user}"

printf 'cmdline=%s\n' "$(cat /proc/cmdline)"
printf 'system_path=%s\n' "$system_path"
printf 'store_device=%s\n' "$store_device"
printf 'store_format=%s\n' "$store_format"
printf 'store_blob_device=%s\n' "$store_blob_device"
printf 'mount_source=%s\n' "$mount_source"
printf 'mount_options=%s\n' "$mount_options"
printf 'current_system=%s\n' "$current_system"
printf 'guest_user=%s\n' "$guest_user"
printf 'home_dir=%s\n' "$home_dir"
printf 'system_state=%s\n' "$(systemctl is-system-running 2>/dev/null || true)"
printf 'qubes_name=%s\n' "$(qubesdb-read /name 2>/dev/null || true)"
domain_role="$(cat /etc/nixome/domain-role 2>/dev/null || true)"
applied_message="$(cat /etc/nixome/applied-message 2>/dev/null || true)"
printf 'domain_role=%s\n' "$domain_role"
printf 'applied_message=%s\n' "$applied_message"

[ -n "$system_path" ]
[ -n "$store_device" ]
[ "$current_system" = "$system_path" ]
[ -x "$system_path/init" ]
[ "$mount_source" = "$store_device" ]
[ "$store_format" = "erofs" ]
[ -n "$store_blob_device" ]
[ -b "$store_blob_device" ]
blob_block="${store_blob_device#/dev/}"
[ -r "/sys/class/block/$blob_block/ro" ]
[ "$(cat "/sys/class/block/$blob_block/ro")" = "1" ]
case ",$mount_options," in
  *,ro,*) ;;
  *) echo "/nix/store is not read-only: $mount_options" >&2; exit 1 ;;
esac

printf '\n== findmnt ==\n'
for mount_path in / /nix/store /rw /home /usr/local /run /var "$home_dir"; do
  printf '%s ' "$mount_path"
  findmnt -rn -T "$mount_path" -o TARGET,SOURCE,OPTIONS || true
done
printf '\n== lsblk ==\n'
lsblk -o NAME,RO,SIZE,FSTYPE,MOUNTPOINTS
printf '\n== services ==\n'
systemctl is-active qubes-db.service
systemctl is-active qubes-qrexec-agent.service
systemctl is-active nixome-qubes-rw-init.service

if [ -n "$expected_domain_role" ] && [ "$domain_role" != "$expected_domain_role" ]; then
  echo "unexpected domain role: $domain_role" >&2
  exit 1
fi

[ -d /rw ]
[ -d /rw/home ]
[ -d /usr/local ]
[ -d "$home_dir" ]
[ -w "$home_dir" ]
touch "$home_dir/nixome-closure-smoke-home"
test -d /run/qubes
test -d /var/log/qubes

printf 'runtime_probe=passed\n'
EOF
  } > "$GUEST_PROBE_SCRIPT"
}

run_guest_probe() {
  output="$1"
  run_qrexec_script_or_fail "$DOMAIN" "runtime guest probe in $DOMAIN" "$output" "$GUEST_PROBE_SCRIPT"
  grep -Fx "system_path=$system_source" "$output" >/dev/null ||
    fail "guest probe did not report expected system path"
  grep -Fx "store_device=$store_device" "$output" >/dev/null ||
    fail "guest probe did not report expected store device"
  grep -Fx "mount_source=$store_device" "$output" >/dev/null ||
    fail "guest probe did not mount store from expected device"
  grep -Fx "runtime_probe=passed" "$output" >/dev/null ||
    fail "guest probe did not complete"
}

run_persistence_probe() {
  cat > "$GUEST_PROBE_SCRIPT.persistence" <<'EOF'
set -eu
guest_user="$(id -un)"
home_dir="${HOME:-/home/$guest_user}"
test -d /rw
test -d /rw/home
test -d /usr/local
test -d "$home_dir"
test -w "$home_dir"
test -f "$home_dir/nixome-closure-smoke-home"
findmnt -rn -T /nix/store -o OPTIONS | grep -E '(^|,)ro(,|$)'
printf 'persistence_probe=passed\n'
EOF
  run_qrexec_script_or_fail "$DOMAIN" "persistent state probe in $DOMAIN" "$GUEST_PERSISTENCE_LOG" "$GUEST_PROBE_SCRIPT.persistence"
  grep -Fx "persistence_probe=passed" "$GUEST_PERSISTENCE_LOG" >/dev/null ||
    fail "persistent state probe did not complete"
}

run_reboot_cycle() {
  start_vm "$DOMAIN"
  wait_for_qrexec "$DOMAIN" "$DOMAIN before reboot" "$LOG_DIR/nix-closure-domain-qrexec-before-reboot.log"

  qvm_run_args=(qvm-run --pass-io)
  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  {
    printf '+'
    printf ' %q' "${qvm_run_args[@]}" "$DOMAIN" "sh -lc 'echo reboot-requested; sleep 1; systemctl reboot'"
    printf '\n'
    "${qvm_run_args[@]}" "$DOMAIN" "sh -lc 'echo reboot-requested; sleep 1; systemctl reboot'"
  } > "$REBOOT_LOG" 2>&1 || true

  sleep 8
  if ! qvm-check --running "$DOMAIN" >/dev/null 2>&1; then
    printf 'domain stopped after guest reboot request; starting it again\n' >> "$REBOOT_LOG"
    qvm-start "$DOMAIN" >> "$REBOOT_LOG" 2>&1
  fi

  wait_for_qrexec "$DOMAIN" "$DOMAIN after reboot" "$LOG_DIR/nix-closure-domain-qrexec-after-reboot.log"
  run_runtime_service_check
  run_guest_probe "$GUEST_PROBE_AFTER_REBOOT_LOG"
  reboot_status="passed"
}

run_shutdown_start_cycle() {
  shutdown_vm "$DOMAIN" "$SHUTDOWN_LOG"
  start_vm "$DOMAIN"
  wait_for_qrexec "$DOMAIN" "$DOMAIN after shutdown/start" "$RESTART_LOG"
  run_runtime_service_check
  run_guest_probe "$GUEST_PROBE_AFTER_RESTART_LOG"
  run_persistence_probe
  shutdown_start_status="passed"
}

run_optional_gui_probe() {
  case "$RUN_GUI_PROBE" in
    1|true|yes)
      ;;
    0|false|no)
      gui_status="skipped"
      printf 'GUI probe skipped by NIXOME_NIX_CLOSURE_GUI_PROBE=%s\n' "$RUN_GUI_PROBE" > "$GUI_LOG"
      return 0
      ;;
    *)
      fail "unsupported NIXOME_NIX_CLOSURE_GUI_PROBE value: $RUN_GUI_PROBE"
      ;;
  esac

  if ! command -v nixome-qube-gui-smoke >/dev/null 2>&1; then
    gui_status="skipped"
    printf 'nixome-qube-gui-smoke is not available\n' > "$GUI_LOG"
    return 0
  fi

  if NIXOME_TEST_QUBE="$DOMAIN" nixome-qube-gui-smoke > "$GUI_LOG" 2>&1; then
    gui_status="passed"
  else
    gui_status="failed"
    cat "$GUI_LOG" >&2 || true
    fail "GUI probe failed for $DOMAIN"
  fi
}

for cmd in \
  awk \
  cat \
  date \
  debugfs \
  findmnt \
  grep \
  jq \
  journalctl \
  nixome-closure-store \
  nixome-nix-domain-storage \
  qrexec-client \
  qvm-check \
  qvm-features \
  qvm-ls \
  qvm-prefs \
  qvm-run \
  qvm-shutdown \
  qvm-start \
  qvm-volume \
  readlink \
  sed \
  stat \
  systemctl \
  tail \
  timeout \
  tr \
  virsh; do
  require_command "$cmd"
done
require_command "$SETUP_CONFIG_COMMAND"

case "$DOMAIN" in
  ""|*[!A-Za-z0-9_.-]*)
    fail "invalid Nix closure test domain name: $DOMAIN"
    ;;
esac
case "$PEER_DOMAIN" in
  ""|*[!A-Za-z0-9_.-]*)
    fail "invalid peer Nix closure test domain name: $PEER_DOMAIN"
    ;;
esac
[ "$PEER_DOMAIN" != "$DOMAIN" ] ||
  fail "peer Nix closure test domain must differ from primary domain"

case "$APPLY_MODE" in
  direct|config-flow)
    ;;
  *)
    fail "unsupported NIXOME_NIX_CLOSURE_APPLY_MODE: $APPLY_MODE"
    ;;
esac

if [ "$APPLY_MODE" = "config-flow" ]; then
  require_command nixome-config-apply
fi

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service ||
  fail "qubes-qrexec-policy-daemon.service is not active"

qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VALIDATOR" >/dev/null 2>&1 || fail "missing config validator VM: $VALIDATOR"
qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"

case "$APPLY_MODE" in
  config-flow)
    set_test_vm_memory "$STAGING" "$CONFIG_VM_TEST_MEMORY" "$CONFIG_VM_TEST_MAXMEM"
    set_test_vm_memory "$VALIDATOR" "$VALIDATOR_TEST_MEMORY" "$VALIDATOR_TEST_MAXMEM"
    set_test_vm_memory "$VAULT" "$CONFIG_VM_TEST_MEMORY" "$CONFIG_VM_TEST_MAXMEM"

    start_vm "$STAGING"
    start_vm "$VALIDATOR"
    start_vm "$VAULT"
    wait_for_qrexec "$STAGING" "$STAGING" "$LOG_DIR/nix-closure-domain-$STAGING-qrexec.log"
    wait_for_qrexec "$VALIDATOR" "$VALIDATOR" "$LOG_DIR/nix-closure-domain-$VALIDATOR-qrexec.log"
    wait_for_qrexec "$VAULT" "$VAULT" "$LOG_DIR/nix-closure-domain-$VAULT-qrexec.log"

    run_qrexec_script_or_fail "$STAGING" "initialize $STAGING config repo" "$LOG_DIR/nix-closure-domain-staging-init.log" <(
      printf 'set -eu\nnixome-config-staging-init\n'
    )
    run_qrexec_script_or_fail "$VALIDATOR" "initialize $VALIDATOR config tools" "$LOG_DIR/nix-closure-domain-validator-init.log" <(
      printf 'set -eu\nnixome-config-validator-init\n'
    )

    {
      shutdown_vm "$DOMAIN" "$LOG_DIR/nix-closure-domain-primary-pre-apply-shutdown.log"
      shutdown_vm "$PEER_DOMAIN" "$LOG_DIR/nix-closure-domain-peer-pre-apply-shutdown.log"
    } > "$PRE_APPLY_SHUTDOWN_LOG" 2>&1 || true

    cat > "$STAGING_UPDATE_SCRIPT" <<EOF
set -eu
cd /home/$QUBE_USER/nixome-config
cat > nixome/machine.nix <<'MACHINENIX'
{
  appliedMessage,
  pkgs ? null,
  system,
  templateBuilders ? null,
  ...
}:
let
  mkClosureDomain = role: message: {
    label = "green";
    netvm = null;
    rootImageSizeMiB = 128;
    prefs = {
      memory = 1024;
      maxmem = 2048;
      qrexec_timeout = 180;
      vcpus = 1;
    };
    features = {
      "nixome-role" = role;
      "nixome-test" = "nix-closure-domain";
    };
    services = {
      "network-manager" = false;
    };
    configuration = {
      environment.etc."nixome/domain-role".text = "\${role}\\n";
      environment.etc."nixome/applied-message".text = "\${message}\\n";
    };
  };
in
{
  domains = { };

  nixNative.domains = {
    "$DOMAIN" = mkClosureDomain "nix-closure-smoke" "$APPLY_MESSAGE";
    "$PEER_DOMAIN" = mkClosureDomain "nix-closure-peer" "$APPLY_MESSAGE";
  };

  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [
      {
        service = "nixome.NixStoreRuntimeCheck";
        argument = "*";
        source = "dom0";
        target = "$DOMAIN";
        action = "allow";
      }
      {
        service = "nixome.NixStoreRuntimeCheck";
        argument = "*";
        source = "dom0";
        target = "$PEER_DOMAIN";
        action = "allow";
      }
    ];
  };

  configVms = {
    guests = {
      config-vault = { };
      config-staging = { };
      config-validator = { };
    };
    domains = { };
  };

  dom0.host = {
    apply.mode = "auto";
    overlays = [ ];
    packages = [ ];
    etc = { };
    systemd = { };
    configuration = { };
    modules = [ ];
  };

  dom0.extraModules = [ ];
  system.args = { };
}
MACHINENIX
git add nixome/machine.nix
git -c user.name='Nixome Closure Smoke' -c user.email='nixome-closure-smoke@localhost' \
  commit -m 'add nix closure smoke domain'
git status --short --branch
EOF

    run_qrexec_script_or_fail \
      "$STAGING" \
      "stage nix-closure domain config in $STAGING" \
      "$STAGING_UPDATE_LOG" \
      "$STAGING_UPDATE_SCRIPT"

    run_or_fail "apply nix-closure domain config through $VALIDATOR" "$APPLY_LOG" \
      env \
        NIXOME_LOG_DIR="$LOG_DIR" \
        NIXOME_CONFIG_STAGING_VM="$STAGING" \
        NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
        NIXOME_CONFIG_VAULT_VM="$VAULT" \
        NIXOME_CONFIG_APPLY_NIX_OFFLINE="$CONFIG_APPLY_NIX_OFFLINE" \
      nixome-config-apply --apply --staging "$STAGING" --validator "$VALIDATOR" --vault "$VAULT"
    ;;
  direct)
    for source_vm in "$DIRECT_PRIMARY_SOURCE_VM" "$DIRECT_PEER_SOURCE_VM" "$DIRECT_GEN2_SOURCE_VM"; do
      qvm-check "$source_vm" >/dev/null 2>&1 || fail "missing direct source VM: $source_vm"
      require_source_domain_artifacts "$source_vm"
    done

    EXPECTED_DOMAIN_ROLE=""
    {
      shutdown_vm "$DOMAIN" "$LOG_DIR/nix-closure-domain-primary-pre-apply-shutdown.log"
      shutdown_vm "$PEER_DOMAIN" "$LOG_DIR/nix-closure-domain-peer-pre-apply-shutdown.log"
    } > "$PRE_APPLY_SHUTDOWN_LOG" 2>&1 || true

    write_direct_topology "$DIRECT_TOPOLOGY_LOG" "$DIRECT_PRIMARY_SOURCE_VM" "$DIRECT_PEER_SOURCE_VM"
    run_or_fail "apply direct nix-closure topology" "$APPLY_LOG" \
      env NIXOME_LOG_DIR="$LOG_DIR" \
      "$SETUP_CONFIG_COMMAND" --apply --topology "$DIRECT_TOPOLOGY_LOG"
    cat "$LOG_DIR/setup-config.json" > "$SETUP_LOG" 2>/dev/null || true
    write_direct_apply_result "direct nix-closure topology setup" "$DIRECT_TOPOLOGY_LOG" "$APPLY_RESULT_JSON"
    ;;
esac

collect_diagnostics

jq -e --arg domain "$DOMAIN" --arg peer "$PEER_DOMAIN" '
  .schema == "nixome.config-apply.v1" and
  .status == "passed" and
  .setup.status == "passed" and
  any(.nix_domain_materializations_result.domains[]; .domain == $domain) and
  any(.nix_domain_materializations_result.domains[]; .domain == $peer)
' "$APPLY_RESULT_JSON" >/dev/null ||
  fail "config apply result did not contain successful nix-closure materializations"

qvm-check "$DOMAIN" >/dev/null 2>&1 || fail "Qubes did not create domain: $DOMAIN"
qvm-ls --raw-data --fields NAME,CLASS | grep -Fx "$DOMAIN|StandaloneVM" >/dev/null ||
  fail "domain is not a StandaloneVM"
qvm-check "$PEER_DOMAIN" >/dev/null 2>&1 || fail "Qubes did not create domain: $PEER_DOMAIN"
qvm-ls --raw-data --fields NAME,CLASS | grep -Fx "$PEER_DOMAIN|StandaloneVM" >/dev/null ||
  fail "peer domain is not a StandaloneVM"
qvm-volume info "$DOMAIN:store" >/dev/null 2>&1 ||
  fail "Qubes store volume was not created"
qvm_volume_list_has_volume "$DOMAIN" store ||
  fail "Qubes volume list does not include the store volume"
qvm-volume info "$PEER_DOMAIN:store" >/dev/null 2>&1 ||
  fail "Qubes peer store volume was not created"
qvm_volume_list_has_volume "$PEER_DOMAIN" store ||
  fail "Qubes volume list does not include the peer store volume"

grep -F "nixome.NixStoreRuntimeCheck * dom0 $DOMAIN allow" "$POLICY_LOG" >/dev/null ||
  fail "generated qrexec policy does not allow nixome.NixStoreRuntimeCheck"
grep -F "nixome.NixStoreRuntimeCheck * dom0 $PEER_DOMAIN allow" "$POLICY_LOG" >/dev/null ||
  fail "generated qrexec policy does not allow nixome.NixStoreRuntimeCheck for peer"
grep -Fx "nixome.NixStoreRuntimeCheck * @anyvm @anyvm deny" "$POLICY_LOG" >/dev/null ||
  fail "generated qrexec policy does not deny nixome.NixStoreRuntimeCheck catch-all"

check_source_paths
[ "$store_view_backend" = "erofs-objectmap" ] ||
  fail "primary domain is not using erofs-objectmap: $store_view_backend"
qvm-prefs "$DOMAIN" kernelopts > "$KERNEL_OPTS_LOG" 2>&1
[ "$(extract_kernelopt init)" = "/sbin/init" ] ||
  fail "kernelopts do not route boot through /sbin/init"
[ "$(extract_kernelopt nixome.system)" = "$system_source" ] ||
  fail "kernelopts nixome.system does not match declared system"
[ "$(extract_kernelopt nixome.storedev)" = "$store_device" ] ||
  fail "kernelopts nixome.storedev does not match declared store device"
[ "$(extract_kernelopt nixome.storefmt)" = "$store_filesystem" ] ||
  fail "kernelopts nixome.storefmt does not match declared store filesystem"
if [ "$store_filesystem" = "erofs" ]; then
  [ "$(extract_kernelopt nixome.storeblobdev)" = "$store_blob_device" ] ||
    fail "kernelopts nixome.storeblobdev does not match declared blob device"
  qvm-volume info "$DOMAIN:store-data" >/dev/null 2>&1 ||
    fail "Qubes store-data volume was not created"
  qvm_volume_list_has_volume "$DOMAIN" store-data ||
    fail "Qubes volume list does not include the store-data volume"
fi

peer_store_view_backend="$(store_view_backend_for "$PEER_DOMAIN")"
[ "$peer_store_view_backend" = "erofs-objectmap" ] ||
  fail "peer domain is not using erofs-objectmap: $peer_store_view_backend"
peer_store_blob_map_physical="$(store_blob_map_physical_for "$PEER_DOMAIN")"
[ -f "$peer_store_blob_map_physical" ] ||
  fail "peer store blob map path is missing"
qvm-volume info "$PEER_DOMAIN:store-data" >/dev/null 2>&1 ||
  fail "Qubes peer store-data volume was not created"
qvm_volume_list_has_volume "$PEER_DOMAIN" store-data ||
  fail "Qubes volume list does not include the peer store-data volume"
write_sharing_proof \
  initial \
  "$DOMAIN" \
  "$PEER_DOMAIN" \
  "$store_blob_map_physical" \
  "$peer_store_blob_map_physical" \
  "$SHARING_PROOF_LOG"

check_root_image_has_no_store
check_store_view_contains_system

write_guest_probe_script
start_vm "$DOMAIN"
start_vm "$PEER_DOMAIN"
wait_for_qrexec "$DOMAIN" "$DOMAIN" "$LOG_DIR/nix-closure-domain-qrexec.log"
wait_for_qrexec "$PEER_DOMAIN" "$PEER_DOMAIN" "$LOG_DIR/nix-closure-domain-peer-qrexec.log"
check_libvirt_filtered_devices "$DOMAIN" "$LIBVIRT_XML_LOG"
check_libvirt_filtered_devices "$PEER_DOMAIN" "$PEER_LIBVIRT_XML_LOG"
run_runtime_service_check
peer_system_source="$(feature_value_for "$PEER_DOMAIN" nixome-system-source)"
peer_store_device="$(feature_value_for "$PEER_DOMAIN" nixome-store-device)"
peer_store_filesystem="$(feature_value_for "$PEER_DOMAIN" nixome-store-filesystem)"
peer_store_blob_device="$(feature_value_for "$PEER_DOMAIN" nixome-store-blob-device)"
run_runtime_service_check_for \
  "$PEER_DOMAIN" \
  "$peer_system_source" \
  "$peer_store_device" \
  "$peer_store_filesystem" \
  "$peer_store_blob_device" \
  "$PEER_RUNTIME_SERVICE_LOG" \
  "$PEER_RUNTIME_SERVICE_ERR"
run_guest_probe "$GUEST_PROBE_LOG"
shared_object="$(sharing_json_value "$SHARING_PROOF_LOG" '.shared_object')"
domain_unique_object="$(sharing_json_value "$SHARING_PROOF_LOG" '.left_unique_object')"
peer_unique_object="$(sharing_json_value "$SHARING_PROOF_LOG" '.right_unique_object')"
run_visibility_probe "$DOMAIN" "$domain_unique_object" "$peer_unique_object" "$shared_object" "$DOMAIN_VISIBILITY_LOG"
run_visibility_probe "$PEER_DOMAIN" "$peer_unique_object" "$domain_unique_object" "$shared_object" "$PEER_VISIBILITY_LOG"
run_persistence_probe

initial_generation="$(store_generation_for "$DOMAIN")"
pre_gen2_segment_bytes="$(closure_segment_bytes_total)"
shutdown_vm "$DOMAIN" "$LOG_DIR/nix-closure-domain-gen2-pre-shutdown.log"

case "$APPLY_MODE" in
  config-flow)
    cat > "$GEN2_STAGING_UPDATE_SCRIPT" <<EOF
set -eu
cd /home/$QUBE_USER/nixome-config
sed -i \
  's|"$DOMAIN" = mkClosureDomain "nix-closure-smoke" "$APPLY_MESSAGE";|"$DOMAIN" = mkClosureDomain "nix-closure-smoke" "$APPLY_MESSAGE_GEN2";|' \
  nixome/machine.nix
grep -F '"$DOMAIN" = mkClosureDomain "nix-closure-smoke" "$APPLY_MESSAGE_GEN2";' nixome/machine.nix
git add nixome/machine.nix
git -c user.name='Nixome Closure Smoke' -c user.email='nixome-closure-smoke@localhost' \
  commit -m 'update nix closure smoke domain generation'
git status --short --branch
EOF

    run_qrexec_script_or_fail \
      "$STAGING" \
      "stage second nix-closure domain generation in $STAGING" \
      "$GEN2_STAGING_UPDATE_LOG" \
      "$GEN2_STAGING_UPDATE_SCRIPT"

    run_or_fail "apply second nix-closure domain generation through $VALIDATOR" "$GEN2_APPLY_LOG" \
      env \
        NIXOME_LOG_DIR="$LOG_DIR" \
        NIXOME_CONFIG_STAGING_VM="$STAGING" \
        NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
        NIXOME_CONFIG_VAULT_VM="$VAULT" \
        NIXOME_CONFIG_APPLY_NIX_OFFLINE="$CONFIG_APPLY_NIX_OFFLINE" \
      nixome-config-apply --apply --staging "$STAGING" --validator "$VALIDATOR" --vault "$VAULT"
    cp "$LOG_DIR/config-apply.json" "$GEN2_APPLY_RESULT_JSON" 2>/dev/null || true
    ;;
  direct)
    EXPECTED_DOMAIN_ROLE=""
    write_direct_topology "$DIRECT_GEN2_TOPOLOGY_LOG" "$DIRECT_GEN2_SOURCE_VM" "$DIRECT_PEER_SOURCE_VM"
    run_or_fail "apply direct second nix-closure generation" "$GEN2_APPLY_LOG" \
      env NIXOME_LOG_DIR="$LOG_DIR" \
      "$SETUP_CONFIG_COMMAND" --apply --topology "$DIRECT_GEN2_TOPOLOGY_LOG"
    cat "$LOG_DIR/setup-config.json" > "$SETUP_LOG" 2>/dev/null || true
    write_direct_apply_result \
      "direct second nix-closure topology setup" \
      "$DIRECT_GEN2_TOPOLOGY_LOG" \
      "$GEN2_APPLY_RESULT_JSON"
    ;;
esac

check_source_paths
new_generation="$(store_generation_for "$DOMAIN")"
[ "$new_generation" != "$initial_generation" ] ||
  fail "second generation did not switch the primary domain"
new_store_blob_map_physical="$store_blob_map_physical"
write_sharing_proof \
  gen2 \
  "$DOMAIN" \
  "$PEER_DOMAIN" \
  "$new_store_blob_map_physical" \
  "$peer_store_blob_map_physical" \
  "$GEN2_SHARING_PROOF_LOG"
post_gen2_segment_bytes="$(closure_segment_bytes_total)"
gen2_bytes_added="$((post_gen2_segment_bytes - pre_gen2_segment_bytes))"
gen2_logical_size="$(sharing_json_value "$GEN2_SHARING_PROOF_LOG" '.left_logical_size')"
[ "$gen2_bytes_added" -lt "$gen2_logical_size" ] ||
  fail "second generation rewrote too much canonical object data"
gen2_tmp="$GEN2_SHARING_PROOF_LOG.tmp"
jq \
  --argjson initial_generation "$initial_generation" \
  --argjson new_generation "$new_generation" \
  --argjson segment_bytes_before "$pre_gen2_segment_bytes" \
  --argjson segment_bytes_after "$post_gen2_segment_bytes" \
  --argjson physical_bytes_added "$gen2_bytes_added" \
  '. + {
    initial_generation: $initial_generation,
    new_generation: $new_generation,
    segment_bytes_before: $segment_bytes_before,
    segment_bytes_after: $segment_bytes_after,
    physical_bytes_added_for_new_generation: $physical_bytes_added,
    existing_object_reuse_verified: ($physical_bytes_added < .left_logical_size)
  }' \
  "$GEN2_SHARING_PROOF_LOG" > "$gen2_tmp"
mv "$gen2_tmp" "$GEN2_SHARING_PROOF_LOG"

start_vm "$DOMAIN"
wait_for_qrexec "$DOMAIN" "$DOMAIN after second generation" "$LOG_DIR/nix-closure-domain-qrexec-gen2.log"
check_libvirt_filtered_devices "$DOMAIN" "$LIBVIRT_XML_LOG"
run_runtime_service_check
write_guest_probe_script
run_guest_probe "$GUEST_PROBE_LOG"
if [ "$APPLY_MODE" = "config-flow" ]; then
  grep -Fx "applied_message=$APPLY_MESSAGE_GEN2" "$GUEST_PROBE_LOG" >/dev/null ||
    fail "guest did not boot the second generation"
fi
wait_for_qrexec "$PEER_DOMAIN" "$PEER_DOMAIN after primary generation switch" "$LOG_DIR/nix-closure-domain-peer-qrexec-after-gen2.log"
run_runtime_service_check_for \
  "$PEER_DOMAIN" \
  "$peer_system_source" \
  "$peer_store_device" \
  "$peer_store_filesystem" \
  "$peer_store_blob_device" \
  "$PEER_RUNTIME_SERVICE_LOG" \
  "$PEER_RUNTIME_SERVICE_ERR"

run_or_fail "read closure-store status while second generation is running" "$RUNNING_STATUS_LOG" \
  nixome-closure-store status
jq -e --arg domain "$DOMAIN" --argjson generation "$new_generation" '
  any(.running[]; .domain == $domain and .generation == $generation and .volume == "store") and
  any(.running[]; .domain == $domain and .generation == $generation and .volume == "store-data")
' "$RUNNING_STATUS_LOG" >/dev/null ||
  fail "closure-store running refs do not include second generation"

run_or_fail "run closure-store GC while second generation is running" "$GC_RUNNING_LOG" \
  nixome-closure-store gc --keep-generations 0
jq -e --arg domain "$DOMAIN" --argjson generation "$new_generation" '
  any(.running_generations[]; .domain == $domain and .generation == $generation) and
  any(.retained_generations[]; .domain == $domain and .generation == $generation) and
  all(.removed_generations[]; .domain != $domain or .generation != $generation) and
  (.stale_running_refs | length) == 0
' "$GC_RUNNING_LOG" >/dev/null ||
  fail "closure-store GC did not protect the running second generation"

shutdown_vm "$DOMAIN" "$LOG_DIR/nix-closure-domain-rollback-pre-shutdown.log"
run_json_or_fail "switch primary domain back to retained generation" "$ROLLBACK_SWITCH_LOG" \
  nixome-nix-domain-storage switch-store "$DOMAIN" "$initial_generation"
set_domain_generation_identity_from_switch "$ROLLBACK_SWITCH_LOG"
run_or_fail "mark second generation collectible after rollback" "$RETENTION_LOG" \
  nixome-closure-store set-retention \
    --domain "$DOMAIN" \
    --generation "$new_generation" \
    --retain false
run_or_fail "run closure-store GC after rollback" "$GC_AFTER_ROLLBACK_LOG" \
  nixome-closure-store gc --keep-generations 0
jq -e --arg domain "$DOMAIN" --argjson generation "$new_generation" --arg shared "$shared_object" '
  any(.removed_generations[]; .domain == $domain and .generation == $generation) and
  (.removed_object_blobs | index($shared)) == null
' "$GC_AFTER_ROLLBACK_LOG" >/dev/null ||
  fail "closure-store GC did not remove the unreferenced second generation while preserving shared objects"

check_source_paths
[ "$(store_generation_for "$DOMAIN")" = "$initial_generation" ] ||
  fail "primary domain did not select the initial generation after rollback"
start_vm "$DOMAIN"
wait_for_qrexec "$DOMAIN" "$DOMAIN after rollback" "$LOG_DIR/nix-closure-domain-qrexec-after-rollback.log"
check_libvirt_filtered_devices "$DOMAIN" "$LIBVIRT_XML_LOG"
run_runtime_service_check
write_guest_probe_script
run_guest_probe "$ROLLBACK_GUEST_PROBE_LOG"
if [ "$APPLY_MODE" = "config-flow" ]; then
  grep -Fx "applied_message=$APPLY_MESSAGE" "$ROLLBACK_GUEST_PROBE_LOG" >/dev/null ||
    fail "guest did not boot the rolled-back generation"
fi

case "$RUN_REBOOT_PROBE" in
  1|true|yes)
    run_reboot_cycle
    ;;
  0|false|no)
    reboot_status="skipped"
    ;;
  *)
    fail "unsupported NIXOME_NIX_CLOSURE_REBOOT_PROBE value: $RUN_REBOOT_PROBE"
    ;;
esac
run_shutdown_start_cycle
run_optional_gui_probe

collect_diagnostics
write_result passed "nix-closure domains booted from object-stable shared read-only Nix store views"
cat "$RESULT_JSON" >&3
