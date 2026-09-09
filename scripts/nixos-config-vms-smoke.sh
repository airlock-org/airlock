#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-vms-smoke.json"
COMMAND_LOG="$LOG_DIR/config-vms-smoke.commands.log"
QVM_LS_LOG="$LOG_DIR/config-vms-qvm-ls.log"
STAGING_UNAME_LOG="$LOG_DIR/config-vms-config-staging-uname.log"
VAULT_UNAME_LOG="$LOG_DIR/config-vms-config-vault-uname.log"
VALIDATOR_UNAME_LOG="$LOG_DIR/config-vms-config-validator-uname.log"
CREATED_QUBE_UNAME_LOG="$LOG_DIR/config-vms-created-qube-uname.log"
STAGING_USER_LOG="$LOG_DIR/config-vms-config-staging-user.log"
VAULT_USER_LOG="$LOG_DIR/config-vms-config-vault-user.log"
VALIDATOR_USER_LOG="$LOG_DIR/config-vms-config-validator-user.log"
CREATED_QUBE_USER_LOG="$LOG_DIR/config-vms-created-qube-user.log"
STAGING_BACKEND_LOG="$LOG_DIR/config-vms-config-staging-backend.log"
VAULT_BACKEND_LOG="$LOG_DIR/config-vms-config-vault-backend.log"
VALIDATOR_BACKEND_LOG="$LOG_DIR/config-vms-config-validator-backend.log"
CREATED_QUBE_BACKEND_LOG="$LOG_DIR/config-vms-created-qube-backend.log"
STAGING_STORE_VOLUME_LOG="$LOG_DIR/config-vms-config-staging-store-volume.log"
VAULT_STORE_VOLUME_LOG="$LOG_DIR/config-vms-config-vault-store-volume.log"
VALIDATOR_STORE_VOLUME_LOG="$LOG_DIR/config-vms-config-validator-store-volume.log"
CREATED_QUBE_STORE_VOLUME_LOG="$LOG_DIR/config-vms-created-qube-store-volume.log"
STAGING_STORE_MOUNT_LOG="$LOG_DIR/config-vms-config-staging-store-mount.log"
VAULT_STORE_MOUNT_LOG="$LOG_DIR/config-vms-config-vault-store-mount.log"
VALIDATOR_STORE_MOUNT_LOG="$LOG_DIR/config-vms-config-validator-store-mount.log"
CREATED_QUBE_STORE_MOUNT_LOG="$LOG_DIR/config-vms-created-qube-store-mount.log"
STAGING_PERSISTENCE_LOG="$LOG_DIR/config-vms-config-staging-persistence.log"
VAULT_PERSISTENCE_LOG="$LOG_DIR/config-vms-config-vault-persistence.log"
VALIDATOR_PERSISTENCE_LOG="$LOG_DIR/config-vms-config-validator-persistence.log"
CREATED_QUBE_PERSISTENCE_LOG="$LOG_DIR/config-vms-created-qube-persistence.log"
STAGING_APPMENUS_LOG="$LOG_DIR/config-vms-config-staging-appmenus.log"
VAULT_APPMENUS_LOG="$LOG_DIR/config-vms-config-vault-appmenus.log"
VALIDATOR_APPMENUS_LOG="$LOG_DIR/config-vms-config-validator-appmenus.log"
SEED_INPUTS_IMPORT_LOG="$LOG_DIR/config-vms-seed-inputs-import.log"
STAGING_INIT_LOG="$LOG_DIR/config-vms-config-staging-init.log"
VALIDATOR_INIT_LOG="$LOG_DIR/config-vms-config-validator-init.log"
STAGING_UPDATE_SCRIPT="$LOG_DIR/config-vms-config-staging-update.sh"
STAGING_UPDATE_LOG="$LOG_DIR/config-vms-config-staging-update.log"
STAGING_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-staging-repo-status.log"
VAULT_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-vault-repo-status.log"
DOM0_SYSTEM_JSON_LOG="$LOG_DIR/config-vms-dom0-system.json"
DOM0_SYSTEM_PLAN_LOG="$LOG_DIR/config-vms-dom0-system-plan.json"
DOM0_SYSTEM_PROFILE_JSON_LOG="$LOG_DIR/config-vms-dom0-profile.json"
DOM0_SYSTEM_APPLY_LOG="$LOG_DIR/config-vms-dom0-system-apply.json"
DOM0_SYSTEM_PROFILE_LINK_LOG="$LOG_DIR/config-vms-dom0-system-profile-link.log"
DOM0_HOST_CONFIG_MESSAGE_LOG="$LOG_DIR/config-vms-dom0-host-config-message.log"
DOM0_EXTRA_MODULE_MESSAGE_LOG="$LOG_DIR/config-vms-dom0-extra-module-message.log"
DOM0_HOST_CONFIGURATION_MESSAGE_LOG="$LOG_DIR/config-vms-dom0-host-configuration-message.log"
DOM0_EXTRA_MODULE_RESTARTED_LOG="$LOG_DIR/config-vms-dom0-extra-module-restarted.log"
DOM0_SYSTEMD_ACTIONS_LOG="$LOG_DIR/config-vms-dom0-systemd-actions.json"
DOM0_HOST_SYSTEM_PROFILE_LINK_LOG="$LOG_DIR/config-vms-dom0-host-system-profile-link.log"
DOM0_CURRENT_SYSTEM_LINK_LOG="$LOG_DIR/config-vms-dom0-current-system-link.log"
CONFIG_STATUS_LOG="$LOG_DIR/config-vms-config-status.json"
ARCHIVE_LOG="$LOG_DIR/config-vms-archive.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
APPLY_LOG="$LOG_DIR/config-vms-apply.log"
APPLY_JSON="$LOG_DIR/config-apply.json"
APPLIED_TOPOLOGY_LOG="$LOG_DIR/config-vms-applied-topology.json"
QREXEC_POLICY_FILE_LOG="$LOG_DIR/config-vms-qrexec-policy.log"
QREXEC_POLICY_PROBE_LOG="$LOG_DIR/config-vms-qrexec-policy-probe.log"
APPLIED_FEATURE_LOG="$LOG_DIR/config-vms-applied-feature.log"
CREATED_QUBE_FEATURE_LOG="$LOG_DIR/config-vms-created-qube-feature.log"
VAULT_ARCHIVE_TREE_LOG="$LOG_DIR/config-vms-config-vault-archive-tree.log"
VAULT_DEPLOYMENT_TREE_LOG="$LOG_DIR/config-vms-config-vault-deployment-tree.log"
JOURNAL_LOG="$LOG_DIR/config-vms-journal.log"
LIBXL_LOG="$LOG_DIR/config-vms-libxl.log"
QVM_PREFS_LOG="$LOG_DIR/config-vms-qvm-prefs.log"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
DOM0_USER="${NIXOME_DOM0_USER:-${SUDO_USER:-nixome}}"
APPLY_MESSAGE="${NIXOME_CONFIG_APPLY_MESSAGE:-smoke-applied-$(date -u +%Y%m%dT%H%M%SZ)}"
CREATED_QUBE="${NIXOME_CONFIG_CREATED_QUBE:-nixome-work-from-config}"
DOM0_SYSTEM_SWITCH_ACTION="${NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION:-switch}"
case "$DOM0_SYSTEM_SWITCH_ACTION" in
  switch)
    DOM0_SYSTEM_REASON="${NIXOME_CONFIG_DOM0_SYSTEM_REASON:-staged dom0 NixOS system generation}"
    EXPECTED_SYSTEMD_ACTIONS_STATUS="passed"
    ;;
  boot)
    DOM0_SYSTEM_REASON="${NIXOME_CONFIG_DOM0_SYSTEM_REASON:-install next dom0 boot generation}"
    EXPECTED_SYSTEMD_ACTIONS_STATUS="skipped"
    ;;
  *)
    printf 'NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION must be switch or boot\n' >&2
    exit 1
    ;;
esac
STAGING_CONSOLE_LOG="$LOG_DIR/config-vms-$STAGING-console.log"
VAULT_CONSOLE_LOG="$LOG_DIR/config-vms-$VAULT-console.log"
VALIDATOR_CONSOLE_LOG="$LOG_DIR/config-vms-$VALIDATOR-console.log"
CREATED_QUBE_CONSOLE_LOG="$LOG_DIR/config-vms-$CREATED_QUBE-console.log"
result_written=0

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec >> "$COMMAND_LOG" 2>&1
trap write_unexpected_failure EXIT

json_string() {
  printf '%s' "$1" | jq -Rs . | tr -d '\n'
}

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

write_result() {
  status="$1"
  reason="$2"

  {
    printf '{'
    printf '"schema":"nixome.config-vms-smoke.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"reason":'
    json_string "$reason"
    printf ',"staging":'
    json_string "$STAGING"
    printf ',"vault":'
    json_string "$VAULT"
    printf ',"validator":'
    json_string "$VALIDATOR"
    printf ',"staging_uname":'
    json_file_string_or_null "$STAGING_UNAME_LOG"
    printf ',"vault_uname":'
    json_file_string_or_null "$VAULT_UNAME_LOG"
    printf ',"validator_uname":'
    json_file_string_or_null "$VALIDATOR_UNAME_LOG"
    printf ',"created_qube":'
    json_string "$CREATED_QUBE"
    printf ',"created_qube_uname":'
    json_file_string_or_null "$CREATED_QUBE_UNAME_LOG"
    printf ',"staging_user":'
    json_file_string_or_null "$STAGING_USER_LOG"
    printf ',"vault_user":'
    json_file_string_or_null "$VAULT_USER_LOG"
    printf ',"validator_user":'
    json_file_string_or_null "$VALIDATOR_USER_LOG"
    printf ',"created_qube_user":'
    json_file_string_or_null "$CREATED_QUBE_USER_LOG"
    printf ',"staging_backend":'
    json_file_string_or_null "$STAGING_BACKEND_LOG"
    printf ',"vault_backend":'
    json_file_string_or_null "$VAULT_BACKEND_LOG"
    printf ',"validator_backend":'
    json_file_string_or_null "$VALIDATOR_BACKEND_LOG"
    printf ',"created_qube_backend":'
    json_file_string_or_null "$CREATED_QUBE_BACKEND_LOG"
    printf ',"staging_store_volume":'
    json_file_string_or_null "$STAGING_STORE_VOLUME_LOG"
    printf ',"vault_store_volume":'
    json_file_string_or_null "$VAULT_STORE_VOLUME_LOG"
    printf ',"validator_store_volume":'
    json_file_string_or_null "$VALIDATOR_STORE_VOLUME_LOG"
    printf ',"created_qube_store_volume":'
    json_file_string_or_null "$CREATED_QUBE_STORE_VOLUME_LOG"
    printf ',"staging_store_mount":'
    json_file_string_or_null "$STAGING_STORE_MOUNT_LOG"
    printf ',"vault_store_mount":'
    json_file_string_or_null "$VAULT_STORE_MOUNT_LOG"
    printf ',"validator_store_mount":'
    json_file_string_or_null "$VALIDATOR_STORE_MOUNT_LOG"
    printf ',"created_qube_store_mount":'
    json_file_string_or_null "$CREATED_QUBE_STORE_MOUNT_LOG"
    printf ',"staging_persistence":'
    json_file_string_or_null "$STAGING_PERSISTENCE_LOG"
    printf ',"vault_persistence":'
    json_file_string_or_null "$VAULT_PERSISTENCE_LOG"
    printf ',"validator_persistence":'
    json_file_string_or_null "$VALIDATOR_PERSISTENCE_LOG"
    printf ',"created_qube_persistence":'
    json_file_string_or_null "$CREATED_QUBE_PERSISTENCE_LOG"
    printf ',"dom0_system_switch_action":'
    json_string "$DOM0_SYSTEM_SWITCH_ACTION"
    printf ',"dom0_system_reason":'
    json_string "$DOM0_SYSTEM_REASON"
    printf ',"staging_appmenus_log":'
    json_file_string_or_null "$STAGING_APPMENUS_LOG"
    printf ',"vault_appmenus_log":'
    json_file_string_or_null "$VAULT_APPMENUS_LOG"
    printf ',"validator_appmenus_log":'
    json_file_string_or_null "$VALIDATOR_APPMENUS_LOG"
    printf ',"staging_init_log":'
    json_file_string_or_null "$STAGING_INIT_LOG"
    printf ',"validator_init_log":'
    json_file_string_or_null "$VALIDATOR_INIT_LOG"
    printf ',"staging_update_log":'
    json_file_string_or_null "$STAGING_UPDATE_LOG"
    printf ',"staging_update_script":'
    json_string "$STAGING_UPDATE_SCRIPT"
    printf ',"staging_repo_status":'
    json_file_string_or_null "$STAGING_REPO_STATUS_LOG"
    printf ',"vault_repo_status":'
    json_file_string_or_null "$VAULT_REPO_STATUS_LOG"
    printf ',"dom0_system":'
    json_file_value_or_null "$DOM0_SYSTEM_JSON_LOG"
    printf ',"dom0_system_plan":'
    json_file_value_or_null "$DOM0_SYSTEM_PLAN_LOG"
    printf ',"dom0_system_profile":'
    json_file_value_or_null "$DOM0_SYSTEM_PROFILE_JSON_LOG"
    printf ',"dom0_system_apply":'
    json_file_value_or_null "$DOM0_SYSTEM_APPLY_LOG"
    printf ',"dom0_system_profile_link":'
    json_file_string_or_null "$DOM0_SYSTEM_PROFILE_LINK_LOG"
    printf ',"dom0_host_config_message":'
    json_file_string_or_null "$DOM0_HOST_CONFIG_MESSAGE_LOG"
    printf ',"dom0_extra_module_message":'
    json_file_string_or_null "$DOM0_EXTRA_MODULE_MESSAGE_LOG"
    printf ',"dom0_host_configuration_message":'
    json_file_string_or_null "$DOM0_HOST_CONFIGURATION_MESSAGE_LOG"
    printf ',"dom0_extra_module_restarted":'
    json_file_string_or_null "$DOM0_EXTRA_MODULE_RESTARTED_LOG"
    printf ',"dom0_systemd_actions":'
    json_file_value_or_null "$DOM0_SYSTEMD_ACTIONS_LOG"
    printf ',"dom0_host_system_profile_link":'
    json_file_string_or_null "$DOM0_HOST_SYSTEM_PROFILE_LINK_LOG"
    printf ',"dom0_current_system_link":'
    json_file_string_or_null "$DOM0_CURRENT_SYSTEM_LINK_LOG"
    printf ',"config_status":'
    json_file_value_or_null "$CONFIG_STATUS_LOG"
    printf ',"archive":'
    json_file_value_or_null "$ARCHIVE_JSON"
    printf ',"archive_log":'
    json_file_string_or_null "$ARCHIVE_LOG"
    printf ',"apply":'
    json_file_value_or_null "$APPLY_JSON"
    printf ',"apply_log":'
    json_file_string_or_null "$APPLY_LOG"
    printf ',"applied_topology":'
    json_file_value_or_null "$APPLIED_TOPOLOGY_LOG"
    printf ',"qrexec_policy_file":'
    json_file_string_or_null "$QREXEC_POLICY_FILE_LOG"
    printf ',"qrexec_policy_probe":'
    json_file_string_or_null "$QREXEC_POLICY_PROBE_LOG"
    printf ',"applied_feature":'
    json_file_string_or_null "$APPLIED_FEATURE_LOG"
    printf ',"created_qube_feature":'
    json_file_string_or_null "$CREATED_QUBE_FEATURE_LOG"
    printf ',"vault_archive_tree":'
    json_file_string_or_null "$VAULT_ARCHIVE_TREE_LOG"
    printf ',"vault_deployment_tree":'
    json_file_string_or_null "$VAULT_DEPLOYMENT_TREE_LOG"
    printf ',"command_log":'
    json_string "$COMMAND_LOG"
    printf ',"qvm_ls_log":'
    json_string "$QVM_LS_LOG"
    printf ',"journal_log":'
    json_string "$JOURNAL_LOG"
    printf ',"libxl_log":'
    json_string "$LIBXL_LOG"
    printf ',"qvm_prefs_log":'
    json_string "$QVM_PREFS_LOG"
    printf ',"staging_console_log":'
    json_string "$STAGING_CONSOLE_LOG"
    printf ',"vault_console_log":'
    json_string "$VAULT_CONSOLE_LOG"
    printf ',"validator_console_log":'
    json_string "$VALIDATOR_CONSOLE_LOG"
    printf ',"created_qube_console_log":'
    json_string "$CREATED_QUBE_CONSOLE_LOG"
    printf '}\n'
  } > "$RESULT_JSON"
  result_written=1
}

fail() {
  reason="$1"
  collect_diagnostics
  write_result failed "$reason"
  printf 'config VMs smoke failed: %s\n' "$reason" >&2
  exit 1
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    collect_diagnostics
    write_result failed "unexpected command failure; see command_log"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

collect_diagnostics() {
  qvm-ls > "$QVM_LS_LOG" 2>&1 || true
  {
    diagnostic_vms=("$VAULT" "$STAGING" "$VALIDATOR" "$CREATED_QUBE")
    for vm in "${diagnostic_vms[@]}"; do
      printf '== %s qvm-prefs ==\n' "$vm"
      if qvm-check "$vm" >/dev/null 2>&1; then
        qvm-prefs "$vm" || true
      else
        printf 'missing domain\n'
      fi
      printf '\n== %s qvm-features ==\n' "$vm"
      if qvm-check "$vm" >/dev/null 2>&1; then
        qvm-features "$vm" || true
      else
        printf 'missing domain\n'
      fi
      printf '\n'
    done
  } > "$QVM_PREFS_LOG" 2>&1 || true
  journalctl -b --no-pager -n 260 \
    -u qubesd \
    -u libvirtd \
    -u qubes-db-dom0 \
    -u qubes-qrexec-policy-daemon \
    > "$JOURNAL_LOG" 2>&1 || true
  tail -n 260 /var/log/libvirt/libxl/libxl-driver.log > "$LIBXL_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$VAULT.log" > "$VAULT_CONSOLE_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$STAGING.log" > "$STAGING_CONSOLE_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$VALIDATOR.log" > "$VALIDATOR_CONSOLE_LOG" 2>&1 || true
  tail -n 500 \
    "/var/log/xen/console/guest-$CREATED_QUBE.log" \
    > "$CREATED_QUBE_CONSOLE_LOG" 2>&1 || true
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

run_qrexec_or_fail() {
  vm="$1"
  description="$2"
  output="$3"
  command="$4"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  run_or_fail "$description" "$output" "${qvm_run_args[@]}" "$vm" "$command"
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

run_appmenus_sync_or_fail() {
  template="$1"
  output="$2"

  if [ "$(id -u)" = 0 ]; then
    run_or_fail "sync appmenus for $template" "$output" \
      runuser -u "$DOM0_USER" -- qvm-sync-appmenus "$template"
  else
    run_or_fail "sync appmenus for $template" "$output" \
      qvm-sync-appmenus "$template"
  fi
}

start_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
    return 0
  fi

  run_or_fail "qvm-start $vm" "$LOG_DIR/config-vms-$vm-start.log" qvm-start "$vm"
}

restart_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    run_or_fail "qvm-shutdown $vm" "$LOG_DIR/config-vms-$vm-shutdown.log" \
      qvm-shutdown --wait "$vm"
  fi

  start_vm "$vm"
}

assert_runtime_nix_store() {
  vm="$1"
  output="$2"
  script="$LOG_DIR/config-vms-$vm-store-check.sh"

  cat > "$script" <<'EOF'
set -eu

mount_line="$(
  while IFS=" " read -r source mountpoint _fstype options _rest; do
    if [ "$mountpoint" = "/nix/store" ]; then
      printf "%s %s\n" "$source" "$options"
      exit 0
    fi
  done < /proc/mounts
  exit 1
)"
printf "%s\n" "$mount_line"

case "$mount_line" in
  /dev/xvde\ *ro,*|/dev/xvde\ *,ro,*|/dev/xvde\ *,ro|/dev/xvde\ ro,*|/dev/xvde\ ro)
    ;;
  *)
    printf "unexpected /nix/store mount: %s\n" "$mount_line" >&2
    exit 1
    ;;
esac

if touch /nix/store/.nixome-write-test 2>/dev/null; then
  rm -f /nix/store/.nixome-write-test 2>/dev/null || true
  printf "/nix/store accepted a write\n" >&2
  exit 1
fi

test -e /run/current-system
readlink /run/current-system
EOF

  run_qrexec_script_or_fail "$vm" "runtime /nix/store check in $vm" "$output" "$script"
}

check_persistent_home() {
  vm="$1"
  output="$2"
  marker="/home/$QUBE_USER/.nixome-persistence-$vm"

  run_qrexec_or_fail "$vm" "write persistent marker in $vm" \
    "$LOG_DIR/config-vms-$vm-persistence-write.log" \
    "printf '%s\n' '$APPLY_MESSAGE' > '$marker' && sync"
  restart_vm "$vm"
  assert_runtime_nix_store "$vm" "$LOG_DIR/config-vms-$vm-store-mount-after-restart.log"
  run_qrexec_or_fail "$vm" "read persistent marker in $vm" "$output" \
    "cat '$marker'"
  grep -qx "$APPLY_MESSAGE" "$output" ||
    fail "$vm persistent home state did not survive restart"
}

assert_nix_closure_domain() {
  vm="$1"
  backend_log="$2"
  store_volume_log="$3"

  run_or_fail "read nix-closure backend state for $vm" "$backend_log" bash -c '
    set -euo pipefail
    vm="$1"
    printf "class="
    qvm-ls --raw-data --fields NAME,CLASS --no-spinner |
      awk -F"|" -v name="$vm" "\$1 == name { print \$2; found = 1 } END { exit(found ? 0 : 1) }"
    printf "backend="
    qvm-features "$vm" nixome-backend
    printf "store_device="
    qvm-features "$vm" nixome-store-device
    printf "store_read_only="
    qvm-features "$vm" nixome-store-read-only
    printf "netvm="
    qvm-prefs "$vm" netvm
  ' bash "$vm"

  grep -qx 'class=StandaloneVM' "$backend_log" ||
    fail "$vm is not a StandaloneVM"
  grep -qx 'backend=nix-closure' "$backend_log" ||
    fail "$vm is not using the nix-closure backend"
  grep -qx 'store_device=/dev/xvde' "$backend_log" ||
    fail "$vm does not declare /dev/xvde as its Nix store device"
  grep -qx 'store_read_only=True' "$backend_log" ||
    fail "$vm does not declare a read-only Nix store"
  grep -Eq '^netvm=(|none|None)$' "$backend_log" ||
    fail "$vm unexpectedly has a NetVM"

  run_or_fail "read store volume for $vm" "$store_volume_log" qvm-volume info "$vm:store"
  grep -Eq '(^|[[:space:]])rw[[:space:]]+False($|[[:space:]])' "$store_volume_log" ||
    fail "$vm store volume is not read-only"
}

for cmd in \
  awk \
  bash \
  grep \
  jq \
  journalctl \
  cat \
  date \
  nixome-config-seed-inputs-import \
  nixome-config-apply \
  nixome-config-status \
  qrexec-client \
  readlink \
  qvm-check \
  qvm-features \
  qvm-ls \
  qvm-prefs \
  qvm-run \
  qvm-start \
  qvm-shutdown \
  qvm-sync-appmenus \
  qvm-volume \
  runuser \
  systemctl \
  tail \
  tr; do
  require_command "$cmd"
done

DOM0_SYSTEM_SWITCH_ACTION_NIX="$(json_string "$DOM0_SYSTEM_SWITCH_ACTION")"
DOM0_SYSTEM_REASON_NIX="$(json_string "$DOM0_SYSTEM_REASON")"

if ! [ -r /proc/xen/capabilities ] || ! grep -qw control_d /proc/xen/capabilities; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service ||
  fail "qubes-qrexec-policy-daemon.service is not active"

case "$CREATED_QUBE" in
  ""|*[!A-Za-z0-9_.-]*)
    fail "invalid configured test qube name: $CREATED_QUBE"
    ;;
esac

case "$APPLY_MESSAGE" in
  ""|*[!A-Za-z0-9_.:-]*)
    fail "invalid configured apply message: $APPLY_MESSAGE"
    ;;
esac

qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"
qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VALIDATOR" >/dev/null 2>&1 || fail "missing config validator VM: $VALIDATOR"

assert_nix_closure_domain "$VAULT" "$VAULT_BACKEND_LOG" "$VAULT_STORE_VOLUME_LOG"
assert_nix_closure_domain "$STAGING" "$STAGING_BACKEND_LOG" "$STAGING_STORE_VOLUME_LOG"
assert_nix_closure_domain "$VALIDATOR" "$VALIDATOR_BACKEND_LOG" "$VALIDATOR_STORE_VOLUME_LOG"

start_vm "$VAULT"
start_vm "$STAGING"
start_vm "$VALIDATOR"

run_qrexec_or_fail "$VAULT" "qrexec uname in $VAULT" "$VAULT_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$STAGING" "qrexec uname in $STAGING" "$STAGING_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VALIDATOR" "qrexec uname in $VALIDATOR" "$VALIDATOR_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VAULT" "qrexec user in $VAULT" "$VAULT_USER_LOG" "id -un"
run_qrexec_or_fail "$STAGING" "qrexec user in $STAGING" "$STAGING_USER_LOG" "id -un"
run_qrexec_or_fail "$VALIDATOR" "qrexec user in $VALIDATOR" "$VALIDATOR_USER_LOG" "id -un"
assert_runtime_nix_store "$VAULT" "$VAULT_STORE_MOUNT_LOG"
assert_runtime_nix_store "$STAGING" "$STAGING_STORE_MOUNT_LOG"
assert_runtime_nix_store "$VALIDATOR" "$VALIDATOR_STORE_MOUNT_LOG"

grep -q 'Linux' "$VAULT_UNAME_LOG" || fail "vault uname output did not contain Linux"
grep -q 'Linux' "$STAGING_UNAME_LOG" || fail "staging uname output did not contain Linux"
grep -q 'Linux' "$VALIDATOR_UNAME_LOG" || fail "validator uname output did not contain Linux"
grep -qx "$QUBE_USER" "$VAULT_USER_LOG" || fail "vault qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$STAGING_USER_LOG" || fail "staging qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$VALIDATOR_USER_LOG" || fail "validator qrexec user was not $QUBE_USER"

run_appmenus_sync_or_fail "$VAULT" "$VAULT_APPMENUS_LOG"
run_appmenus_sync_or_fail "$STAGING" "$STAGING_APPMENUS_LOG"
run_appmenus_sync_or_fail "$VALIDATOR" "$VALIDATOR_APPMENUS_LOG"

check_persistent_home "$VAULT" "$VAULT_PERSISTENCE_LOG"
check_persistent_home "$STAGING" "$STAGING_PERSISTENCE_LOG"
check_persistent_home "$VALIDATOR" "$VALIDATOR_PERSISTENCE_LOG"

if [ "${NIXOME_CONFIG_VMS_IMPORT_SEED_INPUTS:-1}" = "1" ]; then
  run_or_fail "import config seed inputs" "$SEED_INPUTS_IMPORT_LOG" \
    env \
      NIXOME_LOG_DIR="$LOG_DIR" \
      NIXOME_CONFIG_STAGING_VM="$STAGING" \
      NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
      nixome-config-seed-inputs-import
else
  printf 'config seed inputs import skipped by NIXOME_CONFIG_VMS_IMPORT_SEED_INPUTS=0\n' > "$SEED_INPUTS_IMPORT_LOG"
fi

run_qrexec_or_fail "$VAULT" "config vault repo status" "$VAULT_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault && git status --short --branch"
run_qrexec_or_fail "$STAGING" "config staging repo status" "$STAGING_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config && git status --short --branch"

grep -q '^## main$' "$VAULT_REPO_STATUS_LOG" ||
  fail "config vault repository is not initialized cleanly"
grep -q '^## main$' "$STAGING_REPO_STATUS_LOG" ||
  fail "config staging repository is not initialized cleanly"

run_qrexec_or_fail "$STAGING" "config staging init" "$STAGING_INIT_LOG" "nixome-config-staging-init"
run_qrexec_or_fail \
  "$VALIDATOR" \
  "config validator init" \
  "$VALIDATOR_INIT_LOG" \
  "nixome-config-validator-init"

if [ -z "${NIXOME_CONFIG_VMS_SEED:-}" ] || [ ! -r "$NIXOME_CONFIG_VMS_SEED" ]; then
  fail "missing config VM seed helper for smoke update"
fi
if [ -z "${NIXOME_DOM0_HOST_SEED:-}" ] || [ ! -r "$NIXOME_DOM0_HOST_SEED" ]; then
  fail "missing dom0 host seed helper for smoke update"
fi
if [ -z "${NIXOME_FLAKE_SEED:-}" ] || [ ! -r "$NIXOME_FLAKE_SEED" ]; then
  fail "missing flake seed helper for smoke update"
fi
if [ -z "${NIXOME_LOCAL_SEED:-}" ] || [ ! -r "$NIXOME_LOCAL_SEED" ]; then
  fail "missing local config compatibility seed helper for smoke update"
fi
if [ -z "${NIXOME_MACHINE_SEED:-}" ] || [ ! -r "$NIXOME_MACHINE_SEED" ]; then
  fail "missing machine config seed helper for smoke update"
fi
if [ -z "${NIXOME_SYSTEM_SEED:-}" ] || [ ! -r "$NIXOME_SYSTEM_SEED" ]; then
  fail "missing system seed helper for smoke update"
fi
if [ -z "${NIXOME_SYSTEM_SCHEMA_SEED:-}" ] || [ ! -r "$NIXOME_SYSTEM_SCHEMA_SEED" ]; then
  fail "missing system schema seed helper for smoke update"
fi
if [ -z "${NIXOME_TOPOLOGY_SCHEMA_SEED:-}" ] || [ ! -r "$NIXOME_TOPOLOGY_SCHEMA_SEED" ]; then
  fail "missing topology schema seed helper for smoke update"
fi
CONFIG_VMS_SEED_TEXT="$(cat "$NIXOME_CONFIG_VMS_SEED")"
DOM0_HOST_SEED_TEXT="$(cat "$NIXOME_DOM0_HOST_SEED")"
FLAKE_SEED_TEXT="$(cat "$NIXOME_FLAKE_SEED")"
LOCAL_SEED_TEXT="$(cat "$NIXOME_LOCAL_SEED")"
SYSTEM_SEED_TEXT="$(cat "$NIXOME_SYSTEM_SEED")"
SYSTEM_SCHEMA_SEED_TEXT="$(cat "$NIXOME_SYSTEM_SCHEMA_SEED")"
TOPOLOGY_SCHEMA_SEED_TEXT="$(cat "$NIXOME_TOPOLOGY_SCHEMA_SEED")"

cat > "$STAGING_UPDATE_SCRIPT" <<EOF
set -eu
cd /home/$QUBE_USER/nixome-config
extract_input_url() {
  name="\$1"
  awk -v input="\$name" '
    \$0 ~ "^[[:space:]]*" input "[[:space:]]*=" {
      in_input = 1
      next
    }
    in_input && /^[[:space:]]*url[[:space:]]*=/ {
      sub(/^[[:space:]]*url[[:space:]]*=[[:space:]]*"/, "")
      sub(/";[[:space:]]*$/, "")
      print
      exit
    }
    in_input && /^[[:space:]]*};[[:space:]]*$/ {
      in_input = 0
    }
  ' flake.nix
}
nixpkgs_url="\$(sed -n 's/^[[:space:]]*\\(inputs\\.\\)\\{0,1\\}nixpkgs\\.url = "\\(.*\\)";$/\\2/p' flake.nix | head -n 1)"
nixome_support_url="\$(extract_input_url nixome-support)"
nixos_qubes_url="\$(extract_input_url nixos-qubes)"
if [ -n "\$nixome_support_url" ]; then
  nixome_support_path="\${nixome_support_url#path:}"
else
  nixome_support_path="\$(sed -n 's/^      nixomeSupport = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
fi
if [ -n "\$nixos_qubes_url" ]; then
  nixos_qubes_path="\${nixos_qubes_url#path:}"
else
  nixos_qubes_path="\$(sed -n 's/^      nixosQubesSource = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
fi
nixpkgs_libvirt_10_5_path="\$(sed -n 's/^      nixpkgsLibvirt105Source = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
if [ -z "\$nixpkgs_libvirt_10_5_path" ] && [ -e /rw/home/user/nixome-config-seed-inputs/nixpkgs-libvirt_10_5 ]; then
  nixpkgs_libvirt_10_5_path="\$(readlink -f /rw/home/user/nixome-config-seed-inputs/nixpkgs-libvirt_10_5)"
fi
[ -n "\$nixpkgs_url" ]
[ -n "\$nixpkgs_libvirt_10_5_path" ]
[ -n "\$nixome_support_path" ]
[ -n "\$nixos_qubes_path" ]
mkdir -p dom0-profile/bin dom0-profile/share/nixome nixome
rm -f dom0-profile/bin/nixome-dom0-profile-activate dom0-profile/share/nixome/profile.json
cat > flake.nix <<'FLAKENIXEOF'
$FLAKE_SEED_TEXT
FLAKENIXEOF
cat > nixome/config-vms.nix <<'CONFIGVMSNIXEOF'
$CONFIG_VMS_SEED_TEXT
CONFIGVMSNIXEOF
cat > nixome/dom0-host.nix <<'DOM0HOSTNIXEOF'
$DOM0_HOST_SEED_TEXT
DOM0HOSTNIXEOF
cat > nixome/local.nix <<'LOCALNIXEOF'
${LOCAL_SEED_TEXT}
LOCALNIXEOF
cat > nixome/machine.nix <<'MACHINENIXEOF'
{
  appliedMessage,
  pkgs ? null,
  system,
  ...
}:
{
  nixNative.domains."$CREATED_QUBE" = {
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
      "nixome-created-from-config" = "1";
      "nixome-created-from-config-message" = "$APPLY_MESSAGE";
      "nixome-role" = "created-from-config";
    };
    services = {
      "network-manager" = false;
    };
    configuration = {
      environment.etc."nixome/domain-role".text = "created-from-config\n";
      environment.etc."nixome/applied-message".text = "$APPLY_MESSAGE\n";
    };
  };

  dom0.host = {
    packages = with pkgs; [
      hello
    ];
    etc."nixome/local-dom0-extra-module".text = "$APPLY_MESSAGE-extra\n";
    systemd.services.nixome-local-extra-module = {
      description = "Nixome local extra module fixture";
      serviceConfig.Type = "oneshot";
      serviceConfig.RemainAfterExit = true;
      script = ''
        \${pkgs.coreutils}/bin/install -d -m 0755 /var/lib/nixome/system
        printf '%s\n' "$APPLY_MESSAGE" > /var/lib/nixome/system/local-extra-module-restarted
      '';
    };
    configuration = {
      environment.etc."nixome/local-dom0-host-configuration".text = "$APPLY_MESSAGE-configuration\n";
    };
  };

  system.args = {
    dom0SystemSwitchAction = $DOM0_SYSTEM_SWITCH_ACTION_NIX;
    dom0SystemReason = $DOM0_SYSTEM_REASON_NIX;
    extraSystemdServices =
      if $DOM0_SYSTEM_SWITCH_ACTION_NIX == "switch" then {
        "nixome-local-extra-module" = {
          action = "restart";
          reason = "restart local extra module fixture";
        };
      } else { };
  };
}
MACHINENIXEOF
cat > nixome/topology.nix <<'TOPOLOGYNIXEOF'
$TOPOLOGY_SCHEMA_SEED_TEXT
TOPOLOGYNIXEOF
cat > nixome/system-schema.nix <<'SYSTEMSCHEMANIXEOF'
$SYSTEM_SCHEMA_SEED_TEXT
SYSTEMSCHEMANIXEOF
cat > nixome/system.nix <<'SYSTEMNIXEOF'
$SYSTEM_SEED_TEXT
SYSTEMNIXEOF
cat > dom0-profile/share/nixome/profile.json <<PROFILEJSONEOF
{"schema":"nixome.dom0-profile.v1","message":"$APPLY_MESSAGE","built_by":"config-staging"}
PROFILEJSONEOF
cat > dom0-profile/bin/nixome-dom0-profile-activate <<'PROFILEACTIVATEEOF'
#!/bin/sh
set -eu

profile_root="\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd)"
state_dir="\${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
etc_dir="\${NIXOME_SYSTEM_ETC_DIR:-/etc/nixome}"

install -d -m 0755 "\$state_dir" "\$etc_dir"
cp "\$profile_root/share/nixome/profile.json" "\$state_dir/current-profile.json"
cp "\$profile_root/share/nixome/profile.json" "\$etc_dir/system-profile.json"

if [ -n "\${NIXOME_SYSTEM_JSON:-}" ]; then
  cp "\$NIXOME_SYSTEM_JSON" "\$state_dir/current-system.json"
fi
if [ -n "\${NIXOME_SYSTEM_PLAN_JSON:-}" ]; then
  cp "\$NIXOME_SYSTEM_PLAN_JSON" "\$state_dir/current-plan.json"
fi
if [ -n "\${NIXOME_SYSTEM_PROFILE:-}" ]; then
  printf '%s\n' "\$NIXOME_SYSTEM_PROFILE" > "\$state_dir/current-profile-link"
fi
PROFILEACTIVATEEOF
chmod 0755 dom0-profile/bin/nixome-dom0-profile-activate
sed -i \
  -e "s|__NIXOME_NIXPKGS_URL__|\$nixpkgs_url|g" \
  -e "s|__NIXPKGS_LIBVIRT_10_5_PATH__|\$nixpkgs_libvirt_10_5_path|g" \
  -e "s|__NIXOME_NIXOS_QUBES_PATH__|\$nixos_qubes_path|g" \
  -e "s|__NIXOME_SUPPORT_PATH__|\$nixome_support_path|g" \
  flake.nix
sed -i 's|appliedMessage = "initial";|appliedMessage = "$APPLY_MESSAGE";|' flake.nix
nix --extra-experimental-features nix-command --extra-experimental-features flakes flake lock --impure --offline
grep -F 'appliedMessage = "$APPLY_MESSAGE";' flake.nix
grep -F 'configVms = import ./nixome/config-vms.nix;' flake.nix
grep -F 'systemConfig = import ./nixome/system.nix;' flake.nix
grep -F 'rawLocalConfig = import ./nixome/machine.nix {' flake.nix
grep -F 'nixome-dom0-profile = dom0Profile;' flake.nix
grep -F 'nixosConfigurations = nixNativeConfigurations // {' flake.nix
grep -F 'nixome-dom0 = nixomeDom0System;' flake.nix
grep -F 'nixNative.domains."$CREATED_QUBE" = {' nixome/machine.nix
grep -F 'dom0SystemAttr ? "nixosConfigurations.nixome-dom0.config.system.build.toplevel"' nixome/system.nix
git add dom0-profile/bin/nixome-dom0-profile-activate dom0-profile/share/nixome/profile.json flake.lock flake.nix nixome/config-vms.nix nixome/dom0-host.nix nixome/local.nix nixome/machine.nix nixome/prebuilt-template-sources.nix nixome/system.nix nixome/system-schema.nix nixome/topology.nix
git \
  -c user.name='Nixome Config Smoke' \
  -c user.email='nixome-config-smoke@localhost' \
  commit -m 'test config changes whole machine'
git status --short --branch
EOF
run_qrexec_script_or_fail \
  "$STAGING" \
  "modify config candidate in $STAGING" \
  "$STAGING_UPDATE_LOG" \
  "$STAGING_UPDATE_SCRIPT"

run_or_fail "apply config candidate from $STAGING through $VALIDATOR" "$APPLY_LOG" \
  env \
    NIXOME_LOG_DIR="$LOG_DIR" \
    NIXOME_CONFIG_STAGING_VM="$STAGING" \
    NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    NIXOME_CONFIG_VAULT_VM="$VAULT" \
    NIXOME_CONFIG_APPLY_NIX_OFFLINE="${NIXOME_CONFIG_APPLY_NIX_OFFLINE:-0}" \
  nixome-config-apply --apply --staging "$STAGING" --validator "$VALIDATOR" --vault "$VAULT"

run_or_fail \
  "read applied topology" \
  "$APPLIED_TOPOLOGY_LOG" \
  cat /var/log/nixome/config-apply-topology.json
jq -e \
  --arg staging "$STAGING" \
  --arg validator "$VALIDATOR" \
  --arg vault "$VAULT" \
  --arg apply_message "$APPLY_MESSAGE" \
  --arg created "$CREATED_QUBE" \
  '
    def closure_domain($name):
      .domains[$name].class == "StandaloneVM" and
      .domains[$name].template == null and
      .domains[$name].backend.type == "nix-closure" and
      .domains[$name].backend.template == null and
      (.domains[$name].backend.system.source | test("^/nix/store/")) and
      .domains[$name].backend.system.source_ref == null and
      (.domains[$name].backend.root.source | test("^/nix/store/.*/root.img$")) and
      .domains[$name].backend.root.source_ref == null and
      .domains[$name].backend.store.mode == "shared-closure" and
      (.domains[$name].backend.store.source | test("^/nix/store/")) and
      .domains[$name].backend.store.source_ref == null and
      .domains[$name].backend.store.view_backend == "erofs-objectmap" and
      .domains[$name].backend.store.blob_device == "/dev/xvdf" and
      .domains[$name].backend.store.mountpoint == "/nix/store" and
      .domains[$name].backend.store.device == "/dev/xvde" and
      .domains[$name].backend.store.read_only == true;

    .schema == "nixome.qubes-topology.v1" and
    .prefs.default_kernel == "" and
    .templates == {} and
    .kernels == {} and
    closure_domain($vault) and
    .domains[$vault].label == "black" and
    .domains[$vault].netvm == null and
    .domains[$vault].prefs.include_in_backups == true and
    .domains[$vault].features."nixome-config-authority" == "1" and
    .domains[$vault].features."nixome-role" == "config-vault" and
    .domains[$vault].services."network-manager" == false and
    closure_domain($staging) and
    .domains[$staging].label == "gray" and
    .domains[$staging].netvm == null and
    .domains[$staging].prefs.include_in_backups == true and
    .domains[$staging].features."nixome-config-staging" == "1" and
    .domains[$staging].features."nixome-config-applied-message" == $apply_message and
    closure_domain($validator) and
    .domains[$validator].label == "blue" and
    .domains[$validator].netvm == null and
    .domains[$validator].prefs.include_in_backups == true and
    .domains[$validator].features."nixome-config-validator" == "1" and
    .domains[$validator].features."nixome-role" == "config-validator" and
    closure_domain($created) and
    .domains[$created].features."nixome-created-from-config" == "1" and
    .domains[$created].features."nixome-created-from-config-message" == $apply_message
  ' "$APPLIED_TOPOLOGY_LOG" >/dev/null ||
  fail "applied topology does not define self-hosted config VMs"

run_or_fail \
  "read generated Nixome qrexec policy" \
  "$QREXEC_POLICY_FILE_LOG" \
  cat /var/lib/nixome/qubes-policy.d/90-nixome.policy
grep -Fx "nixome.PolicyProbe * dom0 config-vault allow" "$QREXEC_POLICY_FILE_LOG" >/dev/null ||
  fail "generated qrexec policy is missing PolicyProbe allow rule"
grep -Fx "nixome.PolicyProbe * @anyvm @anyvm deny" "$QREXEC_POLICY_FILE_LOG" >/dev/null ||
  fail "generated qrexec policy is missing PolicyProbe deny rule"

run_or_fail \
  "run qrexec probe allowed by generated policy" \
  "$QREXEC_POLICY_PROBE_LOG" \
  qrexec-client -d "$VAULT" -T -w 120 "DEFAULT:QUBESRPC nixome.PolicyProbe+ dom0"
grep -Fx "nixome-policy-probe role=config-vault user=$QUBE_USER" "$QREXEC_POLICY_PROBE_LOG" >/dev/null ||
  fail "generated qrexec policy probe output did not match expected value"

run_or_fail \
  "read applied dom0 system" \
  "$DOM0_SYSTEM_JSON_LOG" \
  cat /var/lib/nixome/system/current-system.json
run_or_fail \
  "read applied dom0 system plan" \
  "$DOM0_SYSTEM_PLAN_LOG" \
  cat /var/lib/nixome/system/current-plan.json
run_or_fail \
  "read applied dom0 profile" \
  "$DOM0_SYSTEM_PROFILE_JSON_LOG" \
  cat /var/lib/nixome/system/current-profile.json
run_or_fail \
  "read system apply result" \
  "$DOM0_SYSTEM_APPLY_LOG" \
  cat /var/lib/nixome/system/last-apply.json
run_or_fail \
  "read system profile link" \
  "$DOM0_SYSTEM_PROFILE_LINK_LOG" \
  readlink /nix/var/nix/profiles/nixome-system
if [ "$DOM0_SYSTEM_SWITCH_ACTION" = "switch" ]; then
  run_or_fail \
    "read host config message" \
    "$DOM0_HOST_CONFIG_MESSAGE_LOG" \
    cat /etc/nixome/inside-config-applied-message
  run_or_fail \
    "read local dom0 extra module message" \
    "$DOM0_EXTRA_MODULE_MESSAGE_LOG" \
    cat /etc/nixome/local-dom0-extra-module
  run_or_fail \
    "read local dom0 host configuration message" \
    "$DOM0_HOST_CONFIGURATION_MESSAGE_LOG" \
    cat /etc/nixome/local-dom0-host-configuration
  run_or_fail \
    "read local dom0 extra module restart marker" \
    "$DOM0_EXTRA_MODULE_RESTARTED_LOG" \
    cat /var/lib/nixome/system/local-extra-module-restarted
fi
run_or_fail \
  "read systemd live action result" \
  "$DOM0_SYSTEMD_ACTIONS_LOG" \
  cat /var/lib/nixome/system/last-systemd-actions.json
run_or_fail \
  "read host system profile link" \
  "$DOM0_HOST_SYSTEM_PROFILE_LINK_LOG" \
  readlink -f /nix/var/nix/profiles/system
run_or_fail \
  "read current system link" \
  "$DOM0_CURRENT_SYSTEM_LINK_LOG" \
  readlink /run/current-system

jq -e \
  --arg apply_message "$APPLY_MESSAGE" \
  --arg switch_action "$DOM0_SYSTEM_SWITCH_ACTION" \
  --arg reason "$DOM0_SYSTEM_REASON" \
  '
  .schema == "nixome.system.v1" and
  .dom0.profile.message == $apply_message and
  .dom0.nixos.system.enable == true and
  (.dom0.nixos.system.source | test("^/nix/store/")) and
  .dom0.nixos.system.source_ref == null and
  .dom0.nixos.system.switch_action == $switch_action and
  any(.plan.live_actions[]; .type == "dom0-profile" and .message == $apply_message) and
  any(.plan.live_actions[]; .type == "dom0-nixos-system" and .action == $switch_action) and
  (
    if $switch_action == "switch" then
      .dom0.systemd.services."nixome-local-extra-module".action == "restart" and
      .plan.reboot_actions == [] and
      .plan.reboot_required == false and
      .plan.reboot_reasons == [] and
      any(.plan.live_actions[]; .type == "systemd-service" and .name == "nixome-local-extra-module" and .action == "restart")
    else
      .dom0.systemd.services == {} and
      .plan.reboot_required == true and
      (.plan.reboot_actions | length) == 1 and
      .plan.reboot_actions[0].type == "dom0-nixos-system" and
      .plan.reboot_actions[0].switch_action == "boot" and
      .plan.reboot_actions[0].reason == $reason and
      .plan.reboot_reasons == [$reason]
    end
  )
' "$DOM0_SYSTEM_JSON_LOG" >/dev/null ||
  fail "applied dom0 system JSON did not match expected value"

jq -e \
  --arg apply_message "$APPLY_MESSAGE" \
  --arg switch_action "$DOM0_SYSTEM_SWITCH_ACTION" \
  --arg reason "$DOM0_SYSTEM_REASON" \
  '
  any(.live_actions[]; .type == "dom0-profile" and .message == $apply_message) and
  any(.live_actions[]; .type == "dom0-nixos-system" and .action == $switch_action) and
  (
    if $switch_action == "switch" then
      any(.live_actions[]; .type == "systemd-service" and .name == "nixome-local-extra-module" and .action == "restart") and
      .reboot_actions == [] and
      .reboot_required == false and
      .reboot_reasons == []
    else
      .reboot_required == true and
      (.reboot_actions | length) == 1 and
      .reboot_actions[0].type == "dom0-nixos-system" and
      .reboot_actions[0].switch_action == "boot" and
      .reboot_actions[0].reason == $reason and
      .reboot_reasons == [$reason]
    end
  )
' "$DOM0_SYSTEM_PLAN_LOG" >/dev/null ||
  fail "applied dom0 system plan did not match expected value"

jq -e --arg apply_message "$APPLY_MESSAGE" '
  .schema == "nixome.dom0-profile.v1" and
  .message == $apply_message and
  .built_by == "config-staging"
' "$DOM0_SYSTEM_PROFILE_JSON_LOG" >/dev/null ||
  fail "applied dom0 profile JSON did not match expected value"

jq -e \
  --arg switch_action "$DOM0_SYSTEM_SWITCH_ACTION" \
  --arg reason "$DOM0_SYSTEM_REASON" \
  '
  .schema == "nixome.system-apply.v1" and
  .status == "passed" and
  .profile_status == "passed" and
  .host_system_status == "passed" and
  (.host_system_source | test("^/nix/store/")) and
  .host_system_switch_action == $switch_action and
  (
    if $switch_action == "switch" then
      .systemd_status == "passed" and
      .systemd_actions_result.status == "passed" and
      any(.systemd_actions_result.actions[]; .name == "nixome-local-extra-module" and .action == "restart" and .status == "passed") and
      .reboot_actions == [] and
      .reboot_required == false and
      .reboot_reasons == []
    else
      .systemd_status == "skipped" and
      .systemd_actions_result.status == "skipped" and
      .systemd_actions_result.actions == [] and
      .reboot_required == true and
      (.reboot_actions | length) == 1 and
      .reboot_actions[0].type == "dom0-nixos-system" and
      .reboot_actions[0].switch_action == "boot" and
      .reboot_reasons == [$reason]
    end
  )
' "$DOM0_SYSTEM_APPLY_LOG" >/dev/null ||
  fail "dom0 system apply JSON did not pass"

if [ "$DOM0_SYSTEM_SWITCH_ACTION" = "switch" ]; then
  grep -qx "$APPLY_MESSAGE" "$DOM0_HOST_CONFIG_MESSAGE_LOG" ||
    fail "inside-config host marker did not match expected value"
  grep -qx "$APPLY_MESSAGE-extra" "$DOM0_EXTRA_MODULE_MESSAGE_LOG" ||
    fail "inside-config local dom0 extra module marker did not match expected value"
  grep -qx "$APPLY_MESSAGE-configuration" "$DOM0_HOST_CONFIGURATION_MESSAGE_LOG" ||
    fail "inside-config local dom0 host configuration marker did not match expected value"
  grep -qx "$APPLY_MESSAGE" "$DOM0_EXTRA_MODULE_RESTARTED_LOG" ||
    fail "inside-config local dom0 extra module restart marker did not match expected value"
fi
host_system_source="$(jq -r '.host_system_source' "$DOM0_SYSTEM_APPLY_LOG")"
grep -qx "$host_system_source" "$DOM0_HOST_SYSTEM_PROFILE_LINK_LOG" ||
  fail "host system profile does not point at applied source"
if [ "$DOM0_SYSTEM_SWITCH_ACTION" = "switch" ]; then
  grep -qx "$host_system_source" "$DOM0_CURRENT_SYSTEM_LINK_LOG" ||
    fail "current system link does not point at applied source"
fi

run_or_fail \
  "read config status" \
  "$CONFIG_STATUS_LOG" \
  nixome-config-status
jq -e \
  --arg staging "$STAGING" \
  --arg vault "$VAULT" \
  --arg validator "$VALIDATOR" \
  --arg created "$CREATED_QUBE" \
  --arg host_system_source "$host_system_source" \
  --arg switch_action "$DOM0_SYSTEM_SWITCH_ACTION" \
  --arg reason "$DOM0_SYSTEM_REASON" \
  '
    def pending_vm_restart($domain; $type):
      any(.pending_vm_restarts.actions[]?; .domain == $domain and .type == $type);

    .schema == "nixome.config-status.v1" and
    .status == "applied" and
    (.topology.domain_names | index($staging)) and
    (.topology.domain_names | index($vault)) and
    (.topology.domain_names | index($validator)) and
    (.topology.domain_names | index($created)) and
    .qvm.available == true and
    .qvm.status == "ok" and
    any(.qvm.managed_domains[]; .name == $staging) and
    any(.qvm.managed_domains[]; .name == $vault) and
    any(.qvm.managed_domains[]; .name == $validator) and
    .qrexec_policy.exists == true and
    .deployment.current.status == "passed" and
    .deployment.last_attempt.status == "passed" and
    .deployment.current.candidate_commit == .apply.candidate_commit and
    .deployment.current_apply.status == "passed" and
    .apply.status == "passed" and
    .system.last_apply.status == "passed" and
    .system.host_system_profile_resolved_target == $host_system_source and
    .pending_vm_restarts.required == true and
    (.pending_vm_restarts.domains | index($staging)) and
    pending_vm_restart($staging; "domain-feature") and
    (
      if $switch_action == "switch" then
        .pending_reboot.required == false and
        .pending_reboot.actions == [] and
        .pending_reboot.reasons == [] and
        .system.pending_reboot.required == false and
        .system.run_current_system_target == $host_system_source and
        .system.run_current_system_resolved_target == $host_system_source
      else
        .pending_reboot.required == true and
        (.pending_reboot.actions | length) == 1 and
        .pending_reboot.actions[0].type == "dom0-nixos-system" and
        .pending_reboot.actions[0].switch_action == "boot" and
        .pending_reboot.reasons == [$reason] and
        .system.pending_reboot.required == true and
        (.system.pending_reboot.actions | length) == 1 and
        .system.pending_reboot.reasons == [$reason]
      end
    )
  ' "$CONFIG_STATUS_LOG" >/dev/null ||
  fail "config status JSON did not match applied state"

qvm-features "$STAGING" nixome-config-applied-message > "$APPLIED_FEATURE_LOG" 2>&1 ||
  fail "applied config feature is missing"
grep -qx "$APPLY_MESSAGE" "$APPLIED_FEATURE_LOG" ||
  fail "applied config feature did not match expected value"

qvm-check "$CREATED_QUBE" >/dev/null 2>&1 || fail "config-created qube is missing: $CREATED_QUBE"
assert_nix_closure_domain "$CREATED_QUBE" "$CREATED_QUBE_BACKEND_LOG" "$CREATED_QUBE_STORE_VOLUME_LOG"
qvm-features \
  "$CREATED_QUBE" \
  nixome-created-from-config-message \
  > "$CREATED_QUBE_FEATURE_LOG" 2>&1 ||
  fail "config-created qube feature is missing"
grep -qx "$APPLY_MESSAGE" "$CREATED_QUBE_FEATURE_LOG" ||
  fail "config-created qube feature did not match expected value"

start_vm "$CREATED_QUBE"
run_qrexec_or_fail \
  "$CREATED_QUBE" \
  "qrexec uname in $CREATED_QUBE" \
  "$CREATED_QUBE_UNAME_LOG" \
  "uname -a"
run_qrexec_or_fail "$CREATED_QUBE" "qrexec user in $CREATED_QUBE" "$CREATED_QUBE_USER_LOG" "id -un"
assert_runtime_nix_store "$CREATED_QUBE" "$CREATED_QUBE_STORE_MOUNT_LOG"
grep -q 'Linux' "$CREATED_QUBE_UNAME_LOG" || fail "created qube uname output did not contain Linux"
grep -qx "$QUBE_USER" "$CREATED_QUBE_USER_LOG" || fail "created qube qrexec user was not $QUBE_USER"
check_persistent_home "$CREATED_QUBE" "$CREATED_QUBE_PERSISTENCE_LOG"

run_qrexec_or_fail \
  "$VAULT" \
  "config vault archive tree" \
  "$VAULT_ARCHIVE_TREE_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault &&
    cat latest-candidate &&
    find candidates -maxdepth 3 -type f | sort"

run_qrexec_or_fail \
  "$VAULT" \
  "config vault deployment tree" \
  "$VAULT_DEPLOYMENT_TREE_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault &&
    latest=\$(cat latest-deployment) &&
    attempt=\$(cat latest-deployment-attempt) &&
    test \"\$latest\" = \"\$attempt\" &&
    test -d \"deployments/\$latest\" &&
    jq -e '.schema == \"nixome.config-deployment.v1\" and
      .status == \"passed\"' \"deployments/\$latest/deployment.json\" &&
    jq -e '.schema == \"nixome.config-apply.v1\" and
      .status == \"passed\"' \"deployments/\$latest/apply.json\" &&
    jq -e '.schema == \"nixome.setup-config.v1\" and
      .status == \"passed\"' \"deployments/\$latest/setup.json\" &&
    jq -e '.schema == \"nixome.qubes-topology.v1\"' \"deployments/\$latest/topology.json\" &&
    jq -e '.schema == \"nixome.systemd-actions.v1\" and
      .status == \"$EXPECTED_SYSTEMD_ACTIONS_STATUS\"' \"deployments/\$latest/systemd-actions.json\" &&
    jq -e '.systemd_actions.status == \"$EXPECTED_SYSTEMD_ACTIONS_STATUS\"' \"deployments/\$latest/deployment.json\" &&
    find \"deployments/\$latest\" -maxdepth 2 -type f | sort"

jq -e --arg staging "$STAGING" --arg validator "$VALIDATOR" --arg vault "$VAULT" '
  .schema == "nixome.config-archive.v1" and
  .status == "passed" and
  .staging == $staging and
  .validator == $validator and
  .vault == $vault and
  .candidate_commit == .validation_commit and
  .candidate_commit == .vault_archive.candidate_commit and
  .validation.schema == "nixome.config-validation.v1" and
  .validation.status == "passed" and
  any(.validation.checks[]; .name == "nix-eval-nixome-topology" and .status == "passed") and
  any(.validation.checks[]; .name == "nixome-topology-json" and .status == "passed") and
  any(.validation.checks[]; .name == "nix-eval-nixome-system" and .status == "passed") and
  any(.validation.checks[]; .name == "nixome-system-json" and .status == "passed") and
  any(.validation.checks[]; .name == "nix-system-source-refs-dry-run" and .status == "passed")
' "$ARCHIVE_JSON" >/dev/null || fail "config archive JSON did not pass"

jq -e \
  --arg staging "$STAGING" \
  --arg validator "$VALIDATOR" \
  --arg vault "$VAULT" \
  --arg switch_action "$DOM0_SYSTEM_SWITCH_ACTION" \
  --arg reason "$DOM0_SYSTEM_REASON" \
  '
  def setup_restart_action($domain; $type):
    any(.setup.restart_actions[]?; .domain == $domain and .type == $type);

  .schema == "nixome.config-apply.v1" and
  .status == "passed" and
  .staging == $staging and
  .validator == $validator and
  .vault == $vault and
  .archive.status == "passed" and
  .deployment.schema == "nixome.config-deployment.v1" and
  .deployment.status == "passed" and
  .deployment_archive.schema == "nixome.config-vault-deployment.v1" and
  .deployment_archive.status == "passed" and
  .system_apply_result.schema == "nixome.system-apply.v1" and
  .system_apply_result.status == "passed" and
  .system_apply_result.profile_status == "passed" and
  .system_apply_result.host_system_status == "passed" and
  .deployment.system_apply.schema == "nixome.system-apply.v1" and
  .deployment.system_apply.status == "passed" and
  .deployment.system_apply.host_system_status == "passed" and
  (
    if $switch_action == "switch" then
      .system_apply_result.systemd_status == "passed" and
      .systemd_actions_result.status == "passed" and
      .system_apply_result.reboot_actions == [] and
      .system_apply_result.reboot_required == false and
      .deployment.system_apply.reboot_actions == [] and
      .deployment.system_apply.reboot_required == false and
      .deployment.systemd_actions.status == "passed"
    else
      .system_apply_result.systemd_status == "skipped" and
      .systemd_actions_result.status == "skipped" and
      .system_apply_result.reboot_required == true and
      (.system_apply_result.reboot_actions | length) == 1 and
      .system_apply_result.reboot_actions[0].type == "dom0-nixos-system" and
      .system_apply_result.reboot_reasons == [$reason] and
      .deployment.system_apply.reboot_required == true and
      (.deployment.system_apply.reboot_actions | length) == 1 and
      .deployment.system_apply.reboot_reasons == [$reason] and
      .deployment.systemd_actions.status == "skipped"
    end
  ) and
  .setup.status == "passed" and
  .setup.domains_changed >= 1 and
  .setup.restart_required == true and
  setup_restart_action($staging; "domain-feature")
' "$APPLY_JSON" >/dev/null || fail "config apply JSON did not pass"

collect_diagnostics
write_result passed "staged Nix created a new qrexec-capable nix-closure VM"
cat "$RESULT_JSON"
