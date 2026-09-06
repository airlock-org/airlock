#!/usr/bin/env bash
set -euo pipefail

SCRIPT_SOURCE="${BASH_SOURCE[0]:-$0}"
ROOT="$(cd "$(dirname "$SCRIPT_SOURCE")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
if [ -n "${NIXOME_LOG_DIR:-}" ]; then
  LOG_DIR="$NIXOME_LOG_DIR"
elif [ -w "$ROOT" ]; then
  LOG_DIR="$ROOT/.nixome-logs"
else
  LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/nixome"
fi

RESULT_JSON="$LOG_DIR/qubes-gui-smoke.json"
LAUNCH_LOG="$LOG_DIR/qubes-gui-launch.log"
QUBE="${NIXOME_TEST_QUBE:-test-nixome-smoke}"
TITLE="${NIXOME_GUI_SMOKE_TITLE:-nixome-gui-smoke-$(date -u +%Y%m%dT%H%M%SZ)}"
DISPLAY_VALUE="${NIXOME_GUI_DISPLAY:-${DISPLAY:-:0}}"
XAUTHORITY_VALUE="${NIXOME_GUI_XAUTHORITY:-${XAUTHORITY:-$HOME/.Xauthority}}"
TIMEOUT_SECONDS="${NIXOME_GUI_SMOKE_TIMEOUT:-60}"
REMOTE_SLEEP="${NIXOME_GUI_SMOKE_REMOTE_SLEEP:-90}"

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
  window_ids="${3:-}"

  {
    printf '{'
    printf '"schema":"nixome.qubes-gui-smoke.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"qube":'
    json_string "$QUBE"
    printf ',"title":'
    json_string "$TITLE"
    printf ',"display":'
    json_string "$DISPLAY_VALUE"
    printf ',"reason":'
    json_string "$reason"
    printf ',"launch_log":'
    json_string "$LAUNCH_LOG"
    printf ',"window_ids":'
    if [ -n "$window_ids" ]; then
      json_string "$window_ids"
    else
      printf 'null'
    fi
    printf '}\n'
  } > "$RESULT_JSON"
}

fail() {
  reason="$1"
  write_result failed "$reason"
  printf 'qubes GUI smoke failed: %s\n' "$reason" >&2
  exit 1
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

case "$TIMEOUT_SECONDS" in
  "" | *[!0-9]*) fail "NIXOME_GUI_SMOKE_TIMEOUT must be an integer" ;;
esac

case "$REMOTE_SLEEP" in
  "" | *[!0-9]*) fail "NIXOME_GUI_SMOKE_REMOTE_SLEEP must be an integer" ;;
esac

for cmd in qvm-check qvm-run qvm-start qvm-start-gui systemctl xdotool; do
  require_command "$cmd"
done

systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

if ! qvm-check --running "$QUBE" >/dev/null 2>&1; then
  qvm-start "$QUBE"
fi

qvm-start-gui "$QUBE" >/dev/null 2>&1 || true

remote_inner="command -v xterm >/dev/null || exit 127; nohup xterm -title $(single_quote "$TITLE") -e sh -c $(single_quote "sleep $REMOTE_SLEEP") >/tmp/nixome-gui-smoke.log 2>&1 &"
remote_cmd="sh -lc $(single_quote "$remote_inner")"
if ! qvm-run "$QUBE" "$remote_cmd" > "$LAUNCH_LOG" 2>&1; then
  fail "failed to launch xterm through qvm-run"
fi

deadline=$((SECONDS + TIMEOUT_SECONDS))
while [ "$SECONDS" -lt "$deadline" ]; do
  if window_ids="$(
    DISPLAY="$DISPLAY_VALUE" XAUTHORITY="$XAUTHORITY_VALUE" \
      xdotool search --name "$TITLE" 2>/dev/null
  )" && [ -n "$window_ids" ]; then
    write_result passed "GUI window appeared" "$window_ids"
    cat "$RESULT_JSON"
    exit 0
  fi
  sleep 2
done

fail "no dom0 X window appeared for $TITLE within ${TIMEOUT_SECONDS}s"
