#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
mkdir -p "$LOG_DIR"

SSH_PORT="${NIXOME_VM_SSH_PORT:-2222}"
SSH_USER="${NIXOME_VM_SSH_USER:-nixome}"
SSH_PASSWORD="${NIXOME_VM_SSH_PASSWORD:-nixome}"
SSH_WAIT_SECONDS="${NIXOME_VM_SSH_WAIT_SECONDS:-240}"
VM_TIMEOUT="${NIXOME_VM_RUN_TIMEOUT:-14400}"
DISPLAY_MODE="${NIXOME_VM_DISPLAY:-gtk}"
KEEP_VM="${NIXOME_KEEP_VM:-0}"

BOOTSTRAP_QUBES="${NIXOME_BOOTSTRAP_QUBES:-1}"
RUN_QREXEC_SMOKE="${NIXOME_RUN_QUBES_SMOKE:-1}"
RUN_GUI_SMOKE="${NIXOME_RUN_GUI_SMOKE:-1}"

TEMPLATE="${NIXOME_QUBES_TEMPLATE:-fedora-43-xfce}"
QUBE="${NIXOME_TEST_QUBE:-test-nixome-smoke}"

QEMU_LOG="$LOG_DIR/vm-gui-smoke-qemu.log"
OBSERVE_JSON="$LOG_DIR/vm-gui-observe.json"
SERVICES_LOG="$LOG_DIR/vm-gui-services.log"
FAILED_UNITS_LOG="$LOG_DIR/vm-gui-failed-units.log"
JOURNAL_LOG="$LOG_DIR/vm-gui-journal.log"
BOOTSTRAP_JSON="$LOG_DIR/vm-gui-bootstrap.json"
BOOTSTRAP_STDERR="$LOG_DIR/vm-gui-bootstrap.stderr.log"
QREXEC_JSON="$LOG_DIR/vm-gui-qrexec-smoke.json"
QREXEC_STDERR="$LOG_DIR/vm-gui-qrexec-smoke.stderr.log"
GUI_JSON="$LOG_DIR/vm-gui-window-smoke.json"
GUI_STDERR="$LOG_DIR/vm-gui-window-smoke.stderr.log"
QVM_LS_LOG="$LOG_DIR/vm-gui-qvm-ls.log"

vm_pid=""

cleanup() {
  if [ "$KEEP_VM" = "1" ]; then
    return
  fi

  if [ -n "$vm_pid" ] && kill -0 "$vm_pid" >/dev/null 2>&1; then
    kill "$vm_pid" >/dev/null 2>&1 || true
    wait "$vm_pid" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT INT TERM

shell_quote() {
  printf '%q' "$1"
}

remote_env_prefix() {
  local prefix="env"
  local name

  for name in \
    NIXOME_QUBES_TEMPLATE \
    NIXOME_QUBES_TEMPLATE_URL \
    NIXOME_QUBES_KERNEL_NAME \
    NIXOME_QUBES_KERNEL_VERSION \
    NIXOME_QUBES_KERNEL_FEDORA \
    NIXOME_QUBES_KERNEL_URL \
    NIXOME_QUBES_KERNEL_MODULES_URL \
    NIXOME_QUBES_GPG_KEY_URL \
    NIXOME_QUBES_GPG_KEY \
    NIXOME_QUBES_VERIFY_RPMS \
    NIXOME_QUBES_DEFAULT_DVM \
    NIXOME_QUBES_UPDATEVM; do
    if [ -n "${!name+x}" ]; then
      prefix="$prefix $name=$(shell_quote "${!name}")"
    fi
  done

  printf '%s' "$prefix"
}

ssh_command() {
  nix shell nixpkgs#sshpass -c sshpass -p "$SSH_PASSWORD" \
    ssh \
      -o StrictHostKeyChecking=no \
      -o UserKnownHostsFile=/dev/null \
      -o ConnectTimeout=5 \
      -p "$SSH_PORT" \
      "$SSH_USER@127.0.0.1" \
      "$@"
}

remote() {
  ssh_command "$1"
}

collect_diagnostics() {
  remote 'systemctl --failed --no-legend || true' > "$FAILED_UNITS_LOG" 2>/dev/null || true
  remote 'journalctl -b --no-pager -n 260 -u qubesd -u libvirtd -u qubes-db-dom0 -u qubes-qrexec-policy-daemon -u nixome-qubes-store-init || true' > "$JOURNAL_LOG" 2>/dev/null || true
  remote 'qvm-ls || true' > "$QVM_LS_LOG" 2>/dev/null || true
  remote 'sudo -n cat /var/log/nixome/qubes-bootstrap.commands.log 2>/dev/null || true' > "$LOG_DIR/vm-gui-bootstrap.commands.log" 2>/dev/null || true
}

printf 'starting nested GUI VM smoke run\n'
printf 'logs: %s\n' "$LOG_DIR"

timeout -k 20s "$VM_TIMEOUT" env \
  NIXOME_VM_IMAGE_ATTR=nixome-vm-gui-image \
  NIXOME_VM_DISPLAY="$DISPLAY_MODE" \
  NIXOME_VM_SSH_PORT="$SSH_PORT" \
  "$ROOT/scripts/run-nested-vm.sh" > "$QEMU_LOG" 2>&1 &
vm_pid="$!"

deadline=$((SECONDS + SSH_WAIT_SECONDS))
while [ "$SECONDS" -lt "$deadline" ]; do
  if remote 'echo ssh-ready' >/dev/null 2>&1; then
    printf 'ssh: ready\n'
    break
  fi

  if ! kill -0 "$vm_pid" >/dev/null 2>&1; then
    tail -n 120 "$QEMU_LOG" >&2 || true
    exit 1
  fi

  sleep 3
done

if ! remote 'echo ssh-ready' >/dev/null 2>&1; then
  tail -n 120 "$QEMU_LOG" >&2 || true
  exit 1
fi

if ! remote 'nixome-observe-dom0' > "$OBSERVE_JSON"; then
  collect_diagnostics
  exit 1
fi

if ! remote 'systemctl is-active libvirtd qubesd qubes-db-dom0 qubes-qrexec-policy-daemon' > "$SERVICES_LOG"; then
  collect_diagnostics
  cat "$SERVICES_LOG" >&2 || true
  exit 1
fi

if [ "$BOOTSTRAP_QUBES" = "1" ]; then
  printf 'bootstrap: installing/checking Qubes VM kernel and template\n'
  bootstrap_env="$(remote_env_prefix)"
  if ! remote "sudo -n $bootstrap_env NIXOME_LOG_DIR=/var/log/nixome nixome-qube-bootstrap" > "$BOOTSTRAP_JSON" 2> "$BOOTSTRAP_STDERR"; then
    collect_diagnostics
    cat "$BOOTSTRAP_STDERR" >&2 || true
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-bootstrap.v1","status":"skipped","reason":"NIXOME_BOOTSTRAP_QUBES=0"}\n' > "$BOOTSTRAP_JSON"
fi

if [ "$RUN_QREXEC_SMOKE" = "1" ]; then
  printf 'qrexec: running uname smoke test\n'
  if ! remote "env NIXOME_LOG_DIR=/tmp/nixome-logs NIXOME_TEST_TEMPLATE=$(shell_quote "$TEMPLATE") NIXOME_TEST_QUBE=$(shell_quote "$QUBE") NIXOME_TEST_CLEANUP=0 nixome-qubes-smoke" > "$QREXEC_JSON" 2> "$QREXEC_STDERR"; then
    collect_diagnostics
    cat "$QREXEC_STDERR" >&2 || true
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-smoke.v1","status":"skipped","reason":"NIXOME_RUN_QUBES_SMOKE=0"}\n' > "$QREXEC_JSON"
fi

if [ "$RUN_GUI_SMOKE" = "1" ]; then
  printf 'gui: running AppVM window smoke test\n'
  if ! remote "env NIXOME_LOG_DIR=/tmp/nixome-logs NIXOME_TEST_QUBE=$(shell_quote "$QUBE") NIXOME_GUI_DISPLAY=:0 nixome-qube-gui-smoke" > "$GUI_JSON" 2> "$GUI_STDERR"; then
    collect_diagnostics
    cat "$GUI_STDERR" >&2 || true
    exit 1
  fi
else
  printf '{"schema":"nixome.qubes-gui-smoke.v1","status":"skipped","reason":"NIXOME_RUN_GUI_SMOKE=0"}\n' > "$GUI_JSON"
fi

remote 'qvm-ls || true' > "$QVM_LS_LOG" 2>/dev/null || true

printf 'nested GUI smoke completed\n'
printf 'observe: %s\n' "$OBSERVE_JSON"
printf 'bootstrap: %s\n' "$BOOTSTRAP_JSON"
printf 'qrexec: %s\n' "$QREXEC_JSON"
printf 'gui: %s\n' "$GUI_JSON"
