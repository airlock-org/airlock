#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
SOURCE_ROOT="${NIXOME_CONFIG_INPUTS_SOURCE:-/etc/nixome/config-inputs}"
SOURCE_MANIFEST="${NIXOME_CONFIG_INPUTS_MANIFEST:-$SOURCE_ROOT/manifest.json}"
SERVICE="${NIXOME_CONFIG_INPUTS_IMPORT_SERVICE:-nixome.Config.InputsImport}"
STAGING="${NIXOME_CONFIG_STAGING_VM:-sys-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-sys-config-validator}"
TIMEOUT="${NIXOME_CONFIG_INPUTS_QREXEC_TIMEOUT:-600}"
RESULT_JSON="$LOG_DIR/config-inputs-import.json"
COMMANDS_LOG="$LOG_DIR/config-inputs-import.commands.log"
STDERR_LOG="$LOG_DIR/config-inputs-import.stderr.log"

mkdir -p "$LOG_DIR"
: > "$COMMANDS_LOG"
: > "$STDERR_LOG"

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

fail() {
  message="$1"
  {
    printf '{"schema":"nixome.config-inputs-import.v1","status":"failed","reason":'
    json_string "$message"
    printf ',"source_root":'
    json_string "$SOURCE_ROOT"
    printf '}\n'
  } | tee "$RESULT_JSON"
  printf '%s\n' "$message" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "missing command: $1"
}

run_logged() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  {
    printf '+'
    printf ' %q' "$@"
    printf '\n'
  } >> "$COMMANDS_LOG"
  "$@"
}

start_vm() {
  vm="$1"

  qvm-check "$vm" >/dev/null 2>&1 || fail "missing config VM: $vm"
  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
    return 0
  fi

  run_logged qvm-start "$vm"
}

for cmd in awk cat cut date grep gzip jq mkdir mktemp nix-store qrexec-client qvm-check qvm-start readlink rm sed sort tar tee tr wc; do
  require_command "$cmd"
done

store_root_of() {
  path="$1"
  case "$path" in
    /nix/store/*)
      printf '%s\n' "$path" | sed 's|^\(/nix/store/[^/]*\).*|\1|'
      ;;
    *)
      fail "config input is not a Nix store path: $path"
      ;;
  esac
}

config_input_path_from_manifest() {
  name="$1"

  jq -er --arg name "$name" '
    if (.schema? == "nixome.config-inputs.v1") and ((.inputs? | type) == "object") then
      .inputs[$name] // empty
    else
      (
        if $name == "nixpkgs" then
          .nixpkgs
        elif $name == "nixpkgs-libvirt_10_5" then
          .nixpkgsLibvirt105
        elif $name == "nixos-qubes" then
          .nixosQubes
        elif $name == "nixome-support" then
          .nixomeSupport
        else
          empty
        end
      ) // empty
    end
  ' "$SOURCE_MANIFEST"
}

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/nixome-config-inputs.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT
archive="$tmpdir/config-inputs.tar"
inputs_manifest="$tmpdir/manifest.json"
inputs_store_paths="$tmpdir/store-paths"
inputs_store_export="$tmpdir/store-export.nar.gz"

for name in nixpkgs nixpkgs-libvirt_10_5 nixos-qubes nixome-support; do
  [ -d "$SOURCE_ROOT/$name" ] || fail "missing config input directory: $SOURCE_ROOT/$name"
  if [ -f "$SOURCE_MANIFEST" ]; then
    resolved="$(config_input_path_from_manifest "$name")" ||
      fail "config input manifest is missing path for: $name"
    [ -n "$resolved" ] || fail "config input manifest is missing path for: $name"
  else
    resolved="$(readlink -f "$SOURCE_ROOT/$name")"
  fi
  store_root="$(store_root_of "$resolved")"
  refs="$(nix-store -q --references "$store_root" | grep -Fvx "$store_root" || true)"
  [ -z "$refs" ] || fail "config input has Nix store references and would pull runtime baggage: $name -> $refs"
  printf '%s\t%s\t%s\n' "$name" "$resolved" "$store_root" >> "$tmpdir/config-inputs.tsv"
done

cut -f3 "$tmpdir/config-inputs.tsv" | sort -u > "$inputs_store_paths"
config_input_path_count="$(wc -l < "$inputs_store_paths" | tr -d ' ')"
jq -Rn '
  reduce inputs as $line (
    {schema: "nixome.config-inputs.v1", inputs: {}};
    ($line | split("\t")) as $fields
    | .inputs[$fields[0]] = $fields[1]
  )
' < "$tmpdir/config-inputs.tsv" > "$inputs_manifest"

{
  printf '+ %q --export' nix-store
  while IFS= read -r store_path; do
    printf ' %q' "$store_path"
  done < "$inputs_store_paths"
  printf ' | %q -1 > %q\n' gzip "$inputs_store_export"
} | tee -a "$COMMANDS_LOG"
# shellcheck disable=SC2046
nix-store --export $(cat "$inputs_store_paths") |
  gzip -1 > "$inputs_store_export"

run_logged tar -C "$tmpdir" -cf "$archive" \
  manifest.json \
  store-paths \
  store-export.nar.gz

archive_bytes="$(wc -c < "$archive" | tr -d ' ')"
store_export_bytes="$(wc -c < "$inputs_store_export" | tr -d ' ')"

import_vm() {
  vm="$1"
  stdout="$LOG_DIR/config-inputs-import-$vm.stdout.log"
  stderr="$LOG_DIR/config-inputs-import-$vm.stderr.log"

  start_vm "$vm"

  qrexec_args=(qrexec-client -d "$vm" -T -w "$TIMEOUT" "DEFAULT:QUBESRPC $SERVICE+ dom0")
  {
    printf '+'
    printf ' %q' "${qrexec_args[@]}"
    printf ' < %q\n' "$archive"
  } | tee -a "$COMMANDS_LOG"

  if ! "${qrexec_args[@]}" < "$archive" > "$stdout" 2> "$stderr"; then
    cat "$stderr" >> "$STDERR_LOG" || true
    cat "$stderr" >&2 || true
    fail "config input import failed for $vm"
  fi
}

import_vm "$STAGING"
import_vm "$VALIDATOR"

{
  printf '{"schema":"nixome.config-inputs-import.v1","status":"passed"'
  printf ',"source_root":'
  json_string "$SOURCE_ROOT"
  printf ',"service":'
  json_string "$SERVICE"
  printf ',"staging":'
  json_string "$STAGING"
  printf ',"validator":'
  json_string "$VALIDATOR"
  printf ',"archive_bytes":%s' "$archive_bytes"
  printf ',"store_export_bytes":%s' "$store_export_bytes"
  printf ',"store_path_count":%s' "$config_input_path_count"
  printf ',"timestamp":'
  json_string "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf '}\n'
} | tee "$RESULT_JSON"
