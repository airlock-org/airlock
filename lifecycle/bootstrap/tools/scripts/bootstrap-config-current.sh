#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-current-bootstrap.json"
COMMAND_LOG="$LOG_DIR/config-current-bootstrap.commands.log"
EXPORT_STDERR="$LOG_DIR/config-current-bootstrap-export.stderr.log"
IMPORT_STDOUT="$LOG_DIR/config-current-bootstrap-import.stdout.log"
IMPORT_STDERR="$LOG_DIR/config-current-bootstrap-import.stderr.log"
CURRENT_TAR="$LOG_DIR/config-current-bootstrap-current.tar"
MANIFEST_JSON="$LOG_DIR/config-current-bootstrap-manifest.json"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"
QREXEC_RUN_DIR="${NIXOME_QREXEC_RUN_DIR:-/var/run/qubes}"
QREXEC_READY_WAIT_SECONDS="${NIXOME_CONFIG_QREXEC_READY_WAIT_SECONDS:-120}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-sys-config-vault}"
STAGING="${NIXOME_CONFIG_STAGING_VM:-sys-config-staging}"
EXPORT_SERVICE="${NIXOME_CONFIG_CURRENT_EXPORT_SERVICE:-nixome.Config.ExportCurrent}"
IMPORT_SERVICE="${NIXOME_CONFIG_CURRENT_IMPORT_SERVICE:-nixome.Config.ImportCurrent}"

tmpdir=""

cleanup() {
  if [ -n "$tmpdir" ]; then
    rm -rf "$tmpdir"
  fi
}
trap cleanup EXIT INT TERM

json_file_or_null() {
  local path="$1"

  if [ -s "$path" ] && jq -e . "$path" >/dev/null 2>&1; then
    cat "$path"
  else
    printf null
  fi
}

write_result() {
  local status="$1"
  local reason="$2"
  local candidate_commit="${3:-}"
  local archive_bytes="${4:-0}"
  local import_json

  import_json="$(json_file_or_null "$IMPORT_STDOUT")"
  jq -c -n \
    --arg schema "nixome.config-current-bootstrap.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg vault "$VAULT" \
    --arg staging "$STAGING" \
    --arg export_service "$EXPORT_SERVICE" \
    --arg import_service "$IMPORT_SERVICE" \
    --arg candidate_commit "$candidate_commit" \
    --arg manifest "$MANIFEST_JSON" \
    --arg current_archive "$CURRENT_TAR" \
    --arg command_log "$COMMAND_LOG" \
    --arg export_stderr "$EXPORT_STDERR" \
    --arg import_stdout "$IMPORT_STDOUT" \
    --arg import_stderr "$IMPORT_STDERR" \
    --argjson archive_bytes "$archive_bytes" \
    --argjson import "$import_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      vault: $vault,
      staging: $staging,
      export_service: $export_service,
      import_service: $import_service,
      candidate_commit: $candidate_commit,
      manifest: $manifest,
      current_archive: $current_archive,
      archive_bytes: $archive_bytes,
      command_log: $command_log,
      export_stderr: $export_stderr,
      import_stdout: $import_stdout,
      import_stderr: $import_stderr,
      import: $import
    }' | tee "$RESULT_JSON"
}

fail() {
  local reason="$1"

  write_result failed "$reason" "" 0 >/dev/null || true
  printf 'config current bootstrap failed: %s\n' "$reason" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"
}

log_command() {
  {
    printf '+'
    printf ' %q' "$@"
    printf '\n'
  } >> "$COMMAND_LOG"
}

run_logged() {
  log_command "$@"
  "$@" >> "$COMMAND_LOG" 2>&1
}

start_vm() {
  local vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm" >> "$COMMAND_LOG"
    return 0
  fi

  run_logged qvm-start "$vm" || fail "qvm-start failed: $vm"
}

wait_for_qrexec_ready() {
  local vm="$1"
  local stderr="$2"
  local waited=0
  local socket

  if [ ! -d "$QREXEC_RUN_DIR" ]; then
    return 0
  fi

  socket="$QREXEC_RUN_DIR/qrexec.$vm"
  while [ "$waited" -lt "$QREXEC_READY_WAIT_SECONDS" ]; do
    if [ -S "$socket" ]; then
      return 0
    fi
    sleep 1
    waited=$((waited + 1))
  done

  printf 'qrexec endpoint did not become ready for %s: %s\n' "$vm" "$socket" >> "$stderr"
  return 1
}

run_qrexec_service_to_file() {
  local vm="$1"
  local output="$2"
  local stderr="$3"
  local service="$4"
  local attempt=1
  local rc=0
  local qrexec_args

  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC $service+ dom0")
  while [ "$attempt" -le 3 ]; do
    : > "$stderr"
    wait_for_qrexec_ready "$vm" "$stderr" || return 1
    log_command "${qrexec_args[@]}"
    if "${qrexec_args[@]}" > "$output" 2> "$stderr"; then
      return 0
    fi
    rc="$?"
    if ! grep -Eq 'connect .*/qrexec\.[^:]+: (No such file or directory|Connection refused)' "$stderr"; then
      return "$rc"
    fi
    printf 'qrexec endpoint for %s disappeared during %s; retrying\n' "$vm" "$service" >> "$stderr"
    attempt=$((attempt + 1))
    sleep 2
  done

  return "$rc"
}

run_qrexec_service_with_stdin() {
  local vm="$1"
  local input="$2"
  local output="$3"
  local stderr="$4"
  local service="$5"
  local attempt=1
  local rc=0
  local qrexec_args

  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC $service+ dom0")
  while [ "$attempt" -le 3 ]; do
    : > "$stderr"
    wait_for_qrexec_ready "$vm" "$stderr" || return 1
    log_command "${qrexec_args[@]}" "<" "$input"
    if "${qrexec_args[@]}" < "$input" > "$output" 2> "$stderr"; then
      return 0
    fi
    rc="$?"
    if ! grep -Eq 'connect .*/qrexec\.[^:]+: (No such file or directory|Connection refused)' "$stderr"; then
      return "$rc"
    fi
    printf 'qrexec endpoint for %s disappeared during %s; retrying\n' "$vm" "$service" >> "$stderr"
    attempt=$((attempt + 1))
    sleep 2
  done

  return "$rc"
}

verify_current_archive() {
  tar -tf "$CURRENT_TAR" |
    awk '
      $0 != "config.bundle" && $0 != "manifest.json" {
        printf "unexpected current config tar member: %s\n", $0 > "/dev/stderr"
        exit 1
      }
    ' || fail "vault current archive contains unexpected paths"

  tar -O -xf "$CURRENT_TAR" manifest.json > "$MANIFEST_JSON"
  jq -e '.schema == "nixome.config-candidate.v1" and (.commit | type == "string")' "$MANIFEST_JSON" >/dev/null ||
    fail "vault current manifest is invalid"
}

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"
: > "$EXPORT_STDERR"
: > "$IMPORT_STDOUT"
: > "$IMPORT_STDERR"
tmpdir="$(mktemp -d)"

for cmd in awk cat date grep jq mkdir mktemp qrexec-client qvm-check qvm-start rm sleep tar tee tr wc; do
  require_command "$cmd"
done

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"
qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"

start_vm "$VAULT"
start_vm "$STAGING"

run_qrexec_service_to_file "$VAULT" "$CURRENT_TAR" "$EXPORT_STDERR" "$EXPORT_SERVICE" ||
  fail "exporting current config from $VAULT failed"
verify_current_archive

candidate_commit="$(jq -r '.commit' "$MANIFEST_JSON")"
archive_bytes="$(wc -c < "$CURRENT_TAR" | tr -d ' ')"

run_qrexec_service_with_stdin "$STAGING" "$CURRENT_TAR" "$IMPORT_STDOUT" "$IMPORT_STDERR" "$IMPORT_SERVICE" ||
  fail "importing current config into $STAGING failed"
jq -e '.schema == "nixome.config-staging-import-current.v1" and .status == "passed"' "$IMPORT_STDOUT" >/dev/null ||
  fail "staging import did not emit a passing result"

write_result passed "sys-config-vault current exported and sys-config-staging initialized" "$candidate_commit" "$archive_bytes"
