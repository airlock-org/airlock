#!/usr/bin/env bash
set -euo pipefail

TOPOLOGY_PATH="${AIRLOCK_QUBES_TOPOLOGY:-/etc/airlock/qubes-topology.json}"
LOG_DIR="${AIRLOCK_LOG_DIR:-/var/log/airlock}"
QVM_START="${AIRLOCK_QVM_START:-qvm-start}"
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
candidate_file="$tmpdir/provider-candidates"
: > "$provider_file"
: > "$started_file"
: > "$failed_file"

jq -r '
  (.domains // {}) | to_entries[]
  | select((((.value.prefs // {}).provides_network) // false) == true)
  | select((((.value.prefs // {}).autostart) // false) == true)
  | .key
' "$TOPOLOGY_PATH" > "$candidate_file"

array_contains() {
  local needle="$1"
  shift
  local item

  for item in "$@"; do
    [ "$item" != "$needle" ] || return 0
  done

  return 1
}

provider_local_netvm_dependency() {
  local domain="$1"

  jq -r '
    (.domains[$name].netvm // null) as $netvm
    | if $netvm == null then
        ""
      else
        ($netvm | tostring) as $target
        | if $target == "" or ($target | ascii_downcase) == "none" then
            ""
          else
            (.domains[$target] // null) as $target_domain
            | if $target_domain != null
              and ((($target_domain.prefs // {}).provides_network // false) == true)
              and ((($target_domain.prefs // {}).autostart // false) == true) then
                $target
              else
                ""
              end
          end
      end
  ' --arg name "$domain" "$TOPOLOGY_PATH"
}

ordered_provider_names() {
  local domain
  local dependency
  local progress
  local -a ordered=()
  local -a pending=()
  local -a next_pending=()

  mapfile -t pending < "$candidate_file"

  while [ "${#pending[@]}" -gt 0 ]; do
    progress=0
    next_pending=()

    for domain in "${pending[@]}"; do
      dependency="$(provider_local_netvm_dependency "$domain")"
      if [ -z "$dependency" ] || array_contains "$dependency" "${ordered[@]}"; then
        ordered+=( "$domain" )
        progress=1
      else
        next_pending+=( "$domain" )
      fi
    done

    if [ "$progress" = "0" ]; then
      local IFS=", "
      printf 'declared autostart network-provider NetVM dependency cycle: %s\n' "${pending[*]}" >&2
      return 1
    fi

    pending=( "${next_pending[@]}" )
  done

  if [ "${#ordered[@]}" -gt 0 ]; then
    printf '%s\n' "${ordered[@]}"
  fi
}

ordered_provider_names > "$provider_file"

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
  --arg schema "airlock.start-network-providers.v1" \
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
