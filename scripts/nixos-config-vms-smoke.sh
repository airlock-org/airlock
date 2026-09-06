#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-vms-smoke.json"
COMMAND_LOG="$LOG_DIR/config-vms-smoke.commands.log"
QVM_LS_LOG="$LOG_DIR/config-vms-qvm-ls.log"
MOD_UNAME_LOG="$LOG_DIR/config-vms-config-mod-uname.log"
VAULT_UNAME_LOG="$LOG_DIR/config-vms-config-vault-uname.log"
MOD_USER_LOG="$LOG_DIR/config-vms-config-mod-user.log"
VAULT_USER_LOG="$LOG_DIR/config-vms-config-vault-user.log"
MOD_TEMPLATE_LOG="$LOG_DIR/config-vms-config-mod-template.log"
VAULT_TEMPLATE_LOG="$LOG_DIR/config-vms-config-vault-template.log"
MOD_APPMENUS_LOG="$LOG_DIR/config-vms-config-mod-appmenus.log"
VAULT_APPMENUS_LOG="$LOG_DIR/config-vms-config-vault-appmenus.log"
MOD_INIT_LOG="$LOG_DIR/config-vms-mod-init.log"
MOD_VALIDATION_LOG="$LOG_DIR/config-vms-mod-validation.log"
MOD_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-mod-repo-status.log"
VAULT_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-vault-repo-status.log"
ARCHIVE_LOG="$LOG_DIR/config-vms-archive.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
JOURNAL_LOG="$LOG_DIR/config-vms-journal.log"
LIBXL_LOG="$LOG_DIR/config-vms-libxl.log"
QVM_PREFS_LOG="$LOG_DIR/config-vms-qvm-prefs.log"

MOD="${NIXOME_CONFIG_MOD_VM:-config-mod}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
DOM0_USER="${NIXOME_DOM0_USER:-${SUDO_USER:-nixome}}"
MOD_CONSOLE_LOG="$LOG_DIR/config-vms-$MOD-console.log"
VAULT_CONSOLE_LOG="$LOG_DIR/config-vms-$VAULT-console.log"
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
    printf ',"modifier":'
    json_string "$MOD"
    printf ',"vault":'
    json_string "$VAULT"
    printf ',"modifier_uname":'
    json_file_string_or_null "$MOD_UNAME_LOG"
    printf ',"vault_uname":'
    json_file_string_or_null "$VAULT_UNAME_LOG"
    printf ',"modifier_user":'
    json_file_string_or_null "$MOD_USER_LOG"
    printf ',"vault_user":'
    json_file_string_or_null "$VAULT_USER_LOG"
    printf ',"modifier_template":'
    json_file_string_or_null "$MOD_TEMPLATE_LOG"
    printf ',"vault_template":'
    json_file_string_or_null "$VAULT_TEMPLATE_LOG"
    printf ',"modifier_appmenus_log":'
    json_file_string_or_null "$MOD_APPMENUS_LOG"
    printf ',"vault_appmenus_log":'
    json_file_string_or_null "$VAULT_APPMENUS_LOG"
    printf ',"modifier_init_log":'
    json_file_string_or_null "$MOD_INIT_LOG"
    printf ',"modifier_validation":'
    json_file_value_or_null "$MOD_VALIDATION_LOG"
    printf ',"modifier_repo_status":'
    json_file_string_or_null "$MOD_REPO_STATUS_LOG"
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
    printf ',"modifier_console_log":'
    json_string "$MOD_CONSOLE_LOG"
    printf ',"vault_console_log":'
    json_string "$VAULT_CONSOLE_LOG"
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
    for vm in "$VAULT" "$MOD"; do
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
  tail -n 500 "/var/log/xen/console/guest-$MOD.log" > "$MOD_CONSOLE_LOG" 2>&1 || true
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
qvm-check "$MOD" >/dev/null 2>&1 || fail "missing config modifier VM: $MOD"

run_or_fail "read template for $VAULT" "$VAULT_TEMPLATE_LOG" qvm-prefs "$VAULT" template
run_or_fail "read template for $MOD" "$MOD_TEMPLATE_LOG" qvm-prefs "$MOD" template
VAULT_TEMPLATE="$(tr -d '\n' < "$VAULT_TEMPLATE_LOG")"
MOD_TEMPLATE="$(tr -d '\n' < "$MOD_TEMPLATE_LOG")"
[ -n "$VAULT_TEMPLATE" ] || fail "missing template preference for $VAULT"
[ -n "$MOD_TEMPLATE" ] || fail "missing template preference for $MOD"

start_vm "$VAULT"
start_vm "$MOD"
start_vm "$VAULT_TEMPLATE"
start_vm "$MOD_TEMPLATE"

run_qrexec_or_fail "$VAULT" "qrexec uname in $VAULT" "$VAULT_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$MOD" "qrexec uname in $MOD" "$MOD_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VAULT" "qrexec user in $VAULT" "$VAULT_USER_LOG" "id -un"
run_qrexec_or_fail "$MOD" "qrexec user in $MOD" "$MOD_USER_LOG" "id -un"

grep -q 'Linux' "$VAULT_UNAME_LOG" || fail "vault uname output did not contain Linux"
grep -q 'Linux' "$MOD_UNAME_LOG" || fail "modifier uname output did not contain Linux"
grep -qx "$QUBE_USER" "$VAULT_USER_LOG" || fail "vault qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$MOD_USER_LOG" || fail "modifier qrexec user was not $QUBE_USER"

run_appmenus_sync_or_fail "$VAULT_TEMPLATE" "$VAULT_APPMENUS_LOG"
run_appmenus_sync_or_fail "$MOD_TEMPLATE" "$MOD_APPMENUS_LOG"

run_qrexec_or_fail "$VAULT" "config vault repo status" "$VAULT_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault && git status --short --branch"
run_qrexec_or_fail "$MOD" "config mod repo status" "$MOD_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config && git status --short --branch"

grep -q '^## main$' "$VAULT_REPO_STATUS_LOG" || fail "config vault repository is not initialized cleanly"
grep -q '^## main$' "$MOD_REPO_STATUS_LOG" || fail "config mod repository is not initialized cleanly"

run_qrexec_or_fail "$MOD" "config mod init" "$MOD_INIT_LOG" "nixome-config-mod-init"
run_qrexec_or_fail "$MOD" "config mod validation in $MOD" "$MOD_VALIDATION_LOG" "nixome-config-mod-validate"

jq -e '
  .schema == "nixome.config-validation.v1" and
  .status == "passed" and
  (.commit | type == "string" and length == 40)
' "$MOD_VALIDATION_LOG" >/dev/null || fail "config mod validation JSON did not pass"

run_or_fail "archive config candidate from $MOD into $VAULT" "$ARCHIVE_LOG" \
  env NIXOME_LOG_DIR="$LOG_DIR" NIXOME_CONFIG_MOD_VM="$MOD" NIXOME_CONFIG_VAULT_VM="$VAULT" \
  nixome-config-archive-candidate --mod "$MOD" --vault "$VAULT"

jq -e '
  .schema == "nixome.config-archive.v1" and
  .status == "passed" and
  .candidate_commit == .validation_commit and
  .candidate_commit == .vault_archive.candidate_commit
' "$ARCHIVE_JSON" >/dev/null || fail "config archive JSON did not pass"

collect_diagnostics
write_result passed "NixOS config VMs booted, config-mod validated a candidate, and dom0 archived it in config-vault"
cat "$RESULT_JSON"
