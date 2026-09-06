#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-vms-smoke.json"
COMMAND_LOG="$LOG_DIR/config-vms-smoke.commands.log"
QVM_LS_LOG="$LOG_DIR/config-vms-qvm-ls.log"
STAGING_UNAME_LOG="$LOG_DIR/config-vms-config-staging-uname.log"
VAULT_UNAME_LOG="$LOG_DIR/config-vms-config-vault-uname.log"
VERIFY_UNAME_LOG="$LOG_DIR/config-vms-config-verify-uname.log"
STAGING_USER_LOG="$LOG_DIR/config-vms-config-staging-user.log"
VAULT_USER_LOG="$LOG_DIR/config-vms-config-vault-user.log"
VERIFY_USER_LOG="$LOG_DIR/config-vms-config-verify-user.log"
STAGING_TEMPLATE_LOG="$LOG_DIR/config-vms-config-staging-template.log"
VAULT_TEMPLATE_LOG="$LOG_DIR/config-vms-config-vault-template.log"
VERIFY_TEMPLATE_LOG="$LOG_DIR/config-vms-config-verify-template.log"
STAGING_APPMENUS_LOG="$LOG_DIR/config-vms-config-staging-appmenus.log"
VAULT_APPMENUS_LOG="$LOG_DIR/config-vms-config-vault-appmenus.log"
VERIFY_APPMENUS_LOG="$LOG_DIR/config-vms-config-verify-appmenus.log"
STAGING_INIT_LOG="$LOG_DIR/config-vms-config-staging-init.log"
VERIFY_INIT_LOG="$LOG_DIR/config-vms-config-verify-init.log"
STAGING_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-staging-repo-status.log"
VAULT_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-vault-repo-status.log"
ARCHIVE_LOG="$LOG_DIR/config-vms-archive.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
JOURNAL_LOG="$LOG_DIR/config-vms-journal.log"
LIBXL_LOG="$LOG_DIR/config-vms-libxl.log"
QVM_PREFS_LOG="$LOG_DIR/config-vms-qvm-prefs.log"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
VERIFY="${NIXOME_CONFIG_VERIFY_VM:-config-verify}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
DOM0_USER="${NIXOME_DOM0_USER:-${SUDO_USER:-nixome}}"
STAGING_CONSOLE_LOG="$LOG_DIR/config-vms-$STAGING-console.log"
VAULT_CONSOLE_LOG="$LOG_DIR/config-vms-$VAULT-console.log"
VERIFY_CONSOLE_LOG="$LOG_DIR/config-vms-$VERIFY-console.log"
result_written=0

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec > >(tee -a "$COMMAND_LOG") 2>&1
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
    printf ',"verifier":'
    json_string "$VERIFY"
    printf ',"staging_uname":'
    json_file_string_or_null "$STAGING_UNAME_LOG"
    printf ',"vault_uname":'
    json_file_string_or_null "$VAULT_UNAME_LOG"
    printf ',"verifier_uname":'
    json_file_string_or_null "$VERIFY_UNAME_LOG"
    printf ',"staging_user":'
    json_file_string_or_null "$STAGING_USER_LOG"
    printf ',"vault_user":'
    json_file_string_or_null "$VAULT_USER_LOG"
    printf ',"verifier_user":'
    json_file_string_or_null "$VERIFY_USER_LOG"
    printf ',"staging_template":'
    json_file_string_or_null "$STAGING_TEMPLATE_LOG"
    printf ',"vault_template":'
    json_file_string_or_null "$VAULT_TEMPLATE_LOG"
    printf ',"verifier_template":'
    json_file_string_or_null "$VERIFY_TEMPLATE_LOG"
    printf ',"staging_appmenus_log":'
    json_file_string_or_null "$STAGING_APPMENUS_LOG"
    printf ',"vault_appmenus_log":'
    json_file_string_or_null "$VAULT_APPMENUS_LOG"
    printf ',"verifier_appmenus_log":'
    json_file_string_or_null "$VERIFY_APPMENUS_LOG"
    printf ',"staging_init_log":'
    json_file_string_or_null "$STAGING_INIT_LOG"
    printf ',"verifier_init_log":'
    json_file_string_or_null "$VERIFY_INIT_LOG"
    printf ',"staging_repo_status":'
    json_file_string_or_null "$STAGING_REPO_STATUS_LOG"
    printf ',"vault_repo_status":'
    json_file_string_or_null "$VAULT_REPO_STATUS_LOG"
    printf ',"archive":'
    json_file_value_or_null "$ARCHIVE_JSON"
    printf ',"archive_log":'
    json_file_string_or_null "$ARCHIVE_LOG"
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
    printf ',"verifier_console_log":'
    json_string "$VERIFY_CONSOLE_LOG"
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
    for vm in "$VAULT" "$STAGING" "$VERIFY"; do
      printf '== %s qvm-prefs ==\n' "$vm"
      qvm-prefs "$vm" || true
      printf '\n== %s qvm-features ==\n' "$vm"
      qvm-features "$vm" || true
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
  tail -n 500 "/var/log/xen/console/guest-$VERIFY.log" > "$VERIFY_CONSOLE_LOG" 2>&1 || true
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

for cmd in \
  grep \
  jq \
  journalctl \
  nixome-config-archive-candidate \
  qvm-check \
  qvm-ls \
  qvm-run \
  qvm-start \
  qvm-sync-appmenus \
  runuser \
  systemctl \
  tail \
  tr; do
  require_command "$cmd"
done

if ! [ -r /proc/xen/capabilities ] || ! grep -qw control_d /proc/xen/capabilities; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"
qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VERIFY" >/dev/null 2>&1 || fail "missing config verify VM: $VERIFY"

run_or_fail "read template for $VAULT" "$VAULT_TEMPLATE_LOG" qvm-prefs "$VAULT" template
run_or_fail "read template for $STAGING" "$STAGING_TEMPLATE_LOG" qvm-prefs "$STAGING" template
run_or_fail "read template for $VERIFY" "$VERIFY_TEMPLATE_LOG" qvm-prefs "$VERIFY" template
VAULT_TEMPLATE="$(tr -d '\n' < "$VAULT_TEMPLATE_LOG")"
STAGING_TEMPLATE="$(tr -d '\n' < "$STAGING_TEMPLATE_LOG")"
VERIFY_TEMPLATE="$(tr -d '\n' < "$VERIFY_TEMPLATE_LOG")"
[ -n "$VAULT_TEMPLATE" ] || fail "missing template preference for $VAULT"
[ -n "$STAGING_TEMPLATE" ] || fail "missing template preference for $STAGING"
[ -n "$VERIFY_TEMPLATE" ] || fail "missing template preference for $VERIFY"

start_vm "$VAULT"
start_vm "$STAGING"
start_vm "$VERIFY"
start_vm "$VAULT_TEMPLATE"
start_vm "$STAGING_TEMPLATE"
start_vm "$VERIFY_TEMPLATE"

run_qrexec_or_fail "$VAULT" "qrexec uname in $VAULT" "$VAULT_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$STAGING" "qrexec uname in $STAGING" "$STAGING_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VERIFY" "qrexec uname in $VERIFY" "$VERIFY_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VAULT" "qrexec user in $VAULT" "$VAULT_USER_LOG" "id -un"
run_qrexec_or_fail "$STAGING" "qrexec user in $STAGING" "$STAGING_USER_LOG" "id -un"
run_qrexec_or_fail "$VERIFY" "qrexec user in $VERIFY" "$VERIFY_USER_LOG" "id -un"

grep -q 'Linux' "$VAULT_UNAME_LOG" || fail "vault uname output did not contain Linux"
grep -q 'Linux' "$STAGING_UNAME_LOG" || fail "staging uname output did not contain Linux"
grep -q 'Linux' "$VERIFY_UNAME_LOG" || fail "verifier uname output did not contain Linux"
grep -qx "$QUBE_USER" "$VAULT_USER_LOG" || fail "vault qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$STAGING_USER_LOG" || fail "staging qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$VERIFY_USER_LOG" || fail "verifier qrexec user was not $QUBE_USER"

run_appmenus_sync_or_fail "$VAULT_TEMPLATE" "$VAULT_APPMENUS_LOG"
run_appmenus_sync_or_fail "$STAGING_TEMPLATE" "$STAGING_APPMENUS_LOG"
run_appmenus_sync_or_fail "$VERIFY_TEMPLATE" "$VERIFY_APPMENUS_LOG"

run_qrexec_or_fail "$VAULT" "config vault repo status" "$VAULT_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault && git status --short --branch"
run_qrexec_or_fail "$STAGING" "config staging repo status" "$STAGING_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config && git status --short --branch"

grep -q '^## main$' "$VAULT_REPO_STATUS_LOG" || fail "config vault repository is not initialized cleanly"
grep -q '^## main$' "$STAGING_REPO_STATUS_LOG" || fail "config staging repository is not initialized cleanly"

run_qrexec_or_fail "$STAGING" "config staging init" "$STAGING_INIT_LOG" "nixome-config-staging-init"
run_qrexec_or_fail "$VERIFY" "config verify init" "$VERIFY_INIT_LOG" "nixome-config-verify-init"

run_or_fail "archive config candidate from $STAGING into $VAULT" "$ARCHIVE_LOG" \
  env NIXOME_LOG_DIR="$LOG_DIR" NIXOME_CONFIG_STAGING_VM="$STAGING" NIXOME_CONFIG_VERIFY_VM="$VERIFY" NIXOME_CONFIG_VAULT_VM="$VAULT" \
  nixome-config-archive-candidate --staging "$STAGING" --verify "$VERIFY" --vault "$VAULT"

jq -e --arg staging "$STAGING" --arg verifier "$VERIFY" --arg vault "$VAULT" '
  .schema == "nixome.config-archive.v1" and
  .status == "passed" and
  .staging == $staging and
  .verifier == $verifier and
  .vault == $vault and
  .candidate_commit == .validation_commit and
  .candidate_commit == .vault_archive.candidate_commit and
  .validation.schema == "nixome.config-validation.v1" and
  .validation.status == "passed"
' "$ARCHIVE_JSON" >/dev/null || fail "config archive JSON did not pass"

collect_diagnostics
write_result passed "NixOS config VMs booted, config-staging exported a candidate, config-verify validated it, and dom0 archived it in config-vault"
cat "$RESULT_JSON"
