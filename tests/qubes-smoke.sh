#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -n "${NIXOME_LOG_DIR:-}" ]; then
  LOG_DIR="$NIXOME_LOG_DIR"
elif [ -w "$ROOT" ]; then
  LOG_DIR="$ROOT/.nixome-logs"
else
  LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/nixome"
fi
RESULT_JSON="$LOG_DIR/qubes-smoke.json"
QUBE="${NIXOME_TEST_QUBE:-test-nixome-smoke}"
TEMPLATE="${NIXOME_TEST_TEMPLATE:-}"
CLEANUP="${NIXOME_TEST_CLEANUP:-1}"

mkdir -p "$LOG_DIR"

json_escape() {
  printf '%s' "$1" | sed \
    -e 's/\\/\\\\/g' \
    -e 's/"/\\"/g' \
    -e ':a;N;$!ba;s/\n/\\n/g'
}

json_string() {
  printf '"'
  json_escape "$1"
  printf '"'
}

write_result() {
  status="$1"
  reason="$2"
  stdout_file="${3:-}"

  {
    printf '{'
    printf '"schema":"nixome.qubes-smoke.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"qube":'
    json_string "$QUBE"
    printf ',"template":'
    json_string "$TEMPLATE"
    printf ',"reason":'
    json_string "$reason"
    printf ',"stdout":'
    if [ -n "$stdout_file" ] && [ -r "$stdout_file" ]; then
      json_string "$(cat "$stdout_file")"
    else
      printf 'null'
    fi
    printf '}\n'
  } > "$RESULT_JSON"
}

fail() {
  reason="$1"
  rc="${2:-1}"
  write_result failed "$reason"
  printf 'qubes smoke failed: %s\n' "$reason" >&2
  exit "$rc"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

if ! [ -r /proc/xen/capabilities ] || ! grep -qw control_d /proc/xen/capabilities; then
  fail "host is not a Xen control domain"
fi

for cmd in systemctl qvm-check qvm-create qvm-ls qvm-run qvm-start qubes-prefs; do
  require_command "$cmd"
done

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

if [ -z "$TEMPLATE" ]; then
  TEMPLATE="$(qubes-prefs default_template 2>/dev/null || true)"
fi

if [ -z "$TEMPLATE" ]; then
  fail "no template selected; set NIXOME_TEST_TEMPLATE or qubes-prefs default_template"
fi

created=0
if ! qvm-ls --raw-data --fields NAME --no-spinner | grep -Fx "$QUBE" >/dev/null 2>&1; then
  qvm-create --class AppVM --template "$TEMPLATE" --label red "$QUBE"
  created=1
fi

if ! qvm-check --running "$QUBE" >/dev/null 2>&1; then
  qvm-start "$QUBE"
fi

uname_log="$LOG_DIR/qubes-uname.txt"

qvm_run_args=(qvm-run --pass-io)
if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
  qvm_run_args+=(--no-gui)
fi

"${qvm_run_args[@]}" "$QUBE" "uname -a" > "$uname_log"

if ! grep -q 'Linux' "$uname_log"; then
  fail "qrexec uname output did not contain Linux"
fi

write_result passed "qrexec uname succeeded" "$uname_log"

if [ "$created" = "1" ] && [ "$CLEANUP" = "1" ]; then
  if qvm-check --running "$QUBE" >/dev/null 2>&1; then
    qvm-shutdown --wait --timeout=60 "$QUBE" >/dev/null 2>&1 || qvm-kill "$QUBE" >/dev/null 2>&1 || true
  fi
  qvm-remove --force "$QUBE" >/dev/null 2>&1 || true
fi

cat "$RESULT_JSON"
