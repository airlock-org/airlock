#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-vms-smoke.json"
COMMAND_LOG="$LOG_DIR/config-vms-smoke.commands.log"
QVM_LS_LOG="$LOG_DIR/config-vms-qvm-ls.log"
AGENT_UNAME_LOG="$LOG_DIR/config-vms-sys-agent-uname.log"
VAULT_UNAME_LOG="$LOG_DIR/config-vms-sys-config-vault-uname.log"
VAULT_INIT_LOG="$LOG_DIR/config-vms-vault-init.log"
AGENT_FETCH_LOG="$LOG_DIR/config-vms-agent-fetch.log"
JOURNAL_LOG="$LOG_DIR/config-vms-journal.log"
LIBXL_LOG="$LOG_DIR/config-vms-libxl.log"
QVM_PREFS_LOG="$LOG_DIR/config-vms-qvm-prefs.log"

AGENT="${NIXOME_CONFIG_AGENT_VM:-sys-agent}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-sys-config-vault}"
SNAPSHOT_PATH="${NIXOME_CONFIG_AGENT_SNAPSHOT:-/home/user/nixome-config-readonly}"
AGENT_CONSOLE_LOG="$LOG_DIR/config-vms-$AGENT-console.log"
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
    printf ',"agent":'
    json_string "$AGENT"
    printf ',"vault":'
    json_string "$VAULT"
    printf ',"snapshot_path":'
    json_string "$SNAPSHOT_PATH"
    printf ',"agent_uname":'
    json_file_string_or_null "$AGENT_UNAME_LOG"
    printf ',"vault_uname":'
    json_file_string_or_null "$VAULT_UNAME_LOG"
    printf ',"vault_init_log":'
    json_file_string_or_null "$VAULT_INIT_LOG"
    printf ',"agent_fetch_log":'
    json_file_string_or_null "$AGENT_FETCH_LOG"
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
    printf ',"agent_console_log":'
    json_string "$AGENT_CONSOLE_LOG"
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

single_quote() {
  printf "'"
  printf '%s' "$1" | sed "s/'/'\\\\''/g"
  printf "'"
}

collect_diagnostics() {
  qvm-ls > "$QVM_LS_LOG" 2>&1 || true
  {
    for vm in "$VAULT" "$AGENT"; do
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
  tail -n 500 "/var/log/xen/console/guest-$AGENT.log" > "$AGENT_CONSOLE_LOG" 2>&1 || true
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
  qvm-check \
  qvm-ls \
  qvm-run \
  qvm-start \
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
qvm-check "$AGENT" >/dev/null 2>&1 || fail "missing config agent VM: $AGENT"

start_vm "$VAULT"
start_vm "$AGENT"

run_qrexec_or_fail "$VAULT" "qrexec uname in $VAULT" "$VAULT_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$AGENT" "qrexec uname in $AGENT" "$AGENT_UNAME_LOG" "uname -a"

grep -q 'Linux' "$VAULT_UNAME_LOG" || fail "vault uname output did not contain Linux"
grep -q 'Linux' "$AGENT_UNAME_LOG" || fail "agent uname output did not contain Linux"

run_qrexec_or_fail "$VAULT" "config vault init" "$VAULT_INIT_LOG" "nixome-config-vault-init"

agent_fetch_command="
set -eu
dest=$(single_quote "$SNAPSHOT_PATH")
vault=$(single_quote "$VAULT")
rm -rf \"\$dest\"
nixome-agent-fetch-config \"\$vault\" \"\$dest\"
test -d \"\$dest/.git\"
if touch \"\$dest/.nixome-write-test\" 2>/dev/null; then
  echo \"snapshot is writable: \$dest\" >&2
  exit 1
fi
git -C \"\$dest\" log --oneline -1
"

run_qrexec_or_fail "$AGENT" "config snapshot fetch in $AGENT" "$AGENT_FETCH_LOG" "$agent_fetch_command"

collect_diagnostics
write_result passed "NixOS config VMs booted, qrexec worked, and sys-agent fetched a read-only config snapshot"
cat "$RESULT_JSON"
