#!/usr/bin/env bash
set -euo pipefail

TOPOLOGY_PATH="${NIXOME_QUBES_TOPOLOGY:-/etc/nixome/qubes-topology.json}"
LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
QVM_START="${NIXOME_QVM_START:-qvm-start}"
RESULT_JSON="$LOG_DIR/start-network-providers.json"
COMMAND_LOG="$LOG_DIR/start-network-providers.commands.log"

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

provider_file="$tmpdir/providers"
started_file="$tmpdir/started"
failed_file="$tmpdir/failed"
: > "$provider_file"
: > "$started_file"
: > "$failed_file"

jq -r '
  (.domains // {}) | to_entries[]
  | select((((.value.prefs // {}).provides_network) // false) == true)
  | select((((.value.prefs // {}).autostart) // false) == true)
  | select(
      (.value.netvm == null)
      or (.value.netvm == "")
      or ((.value.netvm | tostring | ascii_downcase) == "none")
    )
  | .key
' "$TOPOLOGY_PATH" > "$provider_file"

status="passed"
{
  printf 'topology: %s\n' "$TOPOLOGY_PATH"
  while IFS= read -r vm; do
    [ -n "$vm" ] || continue
    printf '+ qvm-start --skip-if-running -- %s\n' "$vm"
    if "$QVM_START" --skip-if-running -- "$vm"; then
      printf '%s\n' "$vm" >> "$started_file"
    else
      rc=$?
      printf 'qvm-start failed for %s with exit %s\n' "$vm" "$rc" >&2
      printf '%s\n' "$vm" >> "$failed_file"
      status="failed"
    fi
  done < "$provider_file"
} >> "$COMMAND_LOG" 2>&1

file_array() {
  if [ -s "$1" ]; then
    jq -R -s 'split("\n") | map(select(length > 0))' "$1"
  else
    printf '[]'
  fi
}

providers_json="$(file_array "$provider_file")"
started_json="$(file_array "$started_file")"
failed_json="$(file_array "$failed_file")"
jq -n \
  --arg schema "nixome.start-network-providers.v1" \
  --arg status "$status" \
  --arg topology "$TOPOLOGY_PATH" \
  --arg command_log "$COMMAND_LOG" \
  --argjson providers "$providers_json" \
  --argjson started "$started_json" \
  --argjson failed "$failed_json" \
  '{
    schema: $schema,
    status: $status,
    topology: $topology,
    command_log: $command_log,
    providers: $providers,
    started_or_running: $started,
    failed: $failed
  }' > "$RESULT_JSON"
cat "$RESULT_JSON"
[ "$status" = "passed" ]
