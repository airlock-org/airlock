#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-archive.json"
COMMAND_LOG="$LOG_DIR/config-archive.commands.log"
STAGED_CANDIDATE_TAR="$LOG_DIR/config-candidate-staged.tar"
VERIFIED_CANDIDATE_TAR="$LOG_DIR/config-candidate-verified.tar"
EXPORT_STDERR="$LOG_DIR/config-candidate-export.stderr.log"
VERIFY_STDERR="$LOG_DIR/config-candidate-verify.stderr.log"
VALIDATION_JSON="$LOG_DIR/config-candidate-validation.json"
MANIFEST_JSON="$LOG_DIR/config-candidate-manifest.json"
BUNDLE_VERIFY_LOG="$LOG_DIR/config-candidate-bundle-verify.log"
VAULT_ARCHIVE_LOG="$LOG_DIR/config-vault-archive.log"
VAULT_ARCHIVE_STDERR="$LOG_DIR/config-vault-archive.stderr.log"
VAULT_ARCHIVE_JSON="$LOG_DIR/config-vault-archive.json"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VERIFY="${NIXOME_CONFIG_VERIFY_VM:-config-verify}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
CANDIDATE_EXPORT_SERVICE="${NIXOME_CONFIG_CANDIDATE_EXPORT_SERVICE:-nixome.Config.CandidateExport}"
CANDIDATE_VERIFY_SERVICE="${NIXOME_CONFIG_CANDIDATE_VERIFY_SERVICE:-nixome.Config.VerifyCandidate}"
ARCHIVE_SERVICE="${NIXOME_CONFIG_ARCHIVE_SERVICE:-nixome.Config.ArchiveCandidate}"
result_written=0
candidate_commit=""
validation_commit=""
vault_commit=""

usage() {
  cat >&2 <<'EOF'
Usage: nixome-config-archive-candidate [--staging VM] [--verify VM] [--vault VM]

Fetch a config candidate from config-staging, validate it in config-verify, and
archive the verified candidate into config-vault before any deployment attempt.
EOF
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --staging)
      shift
      [ "$#" -gt 0 ] || usage
      STAGING="$1"
      ;;
    --verify)
      shift
      [ "$#" -gt 0 ] || usage
      VERIFY="$1"
      ;;
    --vault)
      shift
      [ "$#" -gt 0 ] || usage
      VAULT="$1"
      ;;
    --help|-h)
      usage
      ;;
    *)
      usage
      ;;
  esac
  shift
done

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec > >(tee -a "$COMMAND_LOG") 2>&1
trap write_unexpected_failure EXIT

json_file_or_null() {
  file="$1"

  if [ -r "$file" ] && jq -e . "$file" >/dev/null 2>&1; then
    jq -c . "$file"
  else
    printf 'null'
  fi
}

write_result() {
  status="$1"
  reason="$2"
  validation_json="$(json_file_or_null "$VALIDATION_JSON")"
  vault_archive_json="$(json_file_or_null "$VAULT_ARCHIVE_JSON")"

  jq -c -n \
    --arg schema "nixome.config-archive.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg staging "$STAGING" \
    --arg verifier "$VERIFY" \
    --arg vault "$VAULT" \
    --arg candidate_export_service "$CANDIDATE_EXPORT_SERVICE" \
    --arg candidate_verify_service "$CANDIDATE_VERIFY_SERVICE" \
    --arg archive_service "$ARCHIVE_SERVICE" \
    --arg candidate_commit "$candidate_commit" \
    --arg validation_commit "$validation_commit" \
    --arg vault_commit "$vault_commit" \
    --arg staged_candidate_tar "$STAGED_CANDIDATE_TAR" \
    --arg verified_candidate_tar "$VERIFIED_CANDIDATE_TAR" \
    --arg command_log "$COMMAND_LOG" \
    --arg export_stderr "$EXPORT_STDERR" \
    --arg verify_stderr "$VERIFY_STDERR" \
    --arg bundle_verify_log "$BUNDLE_VERIFY_LOG" \
    --arg vault_archive_log "$VAULT_ARCHIVE_LOG" \
    --arg vault_archive_stderr "$VAULT_ARCHIVE_STDERR" \
    --argjson validation "$validation_json" \
    --argjson vault_archive "$vault_archive_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      staging: $staging,
      verifier: $verifier,
      vault: $vault,
      candidate_export_service: $candidate_export_service,
      candidate_verify_service: $candidate_verify_service,
      archive_service: $archive_service,
      candidate_commit: $candidate_commit,
      validation_commit: $validation_commit,
      vault_commit: $vault_commit,
      staged_candidate_tar: $staged_candidate_tar,
      verified_candidate_tar: $verified_candidate_tar,
      command_log: $command_log,
      export_stderr: $export_stderr,
      verify_stderr: $verify_stderr,
      bundle_verify_log: $bundle_verify_log,
      vault_archive_stderr: $vault_archive_stderr,
      vault_archive_log: $vault_archive_log,
      validation: $validation,
      vault_archive: $vault_archive
    }' > "$RESULT_JSON"
  result_written=1
}

fail() {
  reason="$1"
  write_result failed "$reason"
  printf 'config archive failed: %s\n' "$reason" >&2
  exit 1
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    write_result failed "unexpected command failure; see command_log"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

run_or_fail() {
  description="$1"
  shift

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  "$@" || fail "$description failed"
}

start_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
    return 0
  fi

  run_or_fail "qvm-start $vm" qvm-start "$vm"
}

run_qrexec_service_to_file() {
  vm="$1"
  description="$2"
  output="$3"
  stderr="$4"
  service="$5"

  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC $service+ dom0")
  printf '+'
  printf ' %q' "${qrexec_args[@]}"
  printf '\n'

  if ! "${qrexec_args[@]}" > "$output" 2> "$stderr"; then
    cat "$stderr" >&2 || true
    fail "$description failed"
  fi
}

run_qrexec_service_with_stdin() {
  vm="$1"
  description="$2"
  input="$3"
  output="$4"
  stderr="$5"
  service="$6"

  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC $service+ dom0")
  printf '+'
  printf ' %q' "${qrexec_args[@]}"
  printf ' < %q\n' "$input"

  if ! "${qrexec_args[@]}" < "$input" > "$output" 2> "$stderr"; then
    cat "$stderr" >&2 || true
    fail "$description failed"
  fi
}

verify_offline_vm() {
  vm="$1"
  label="$2"

  netvm="$(qvm-prefs "$vm" netvm 2>/dev/null || true)"
  case "$netvm" in
    ""|none|None)
      ;;
    *)
      fail "$label VM is not offline: $vm netvm=$netvm"
      ;;
  esac

  provides_network="$(qvm-prefs "$vm" provides_network 2>/dev/null || true)"
  if [ "$provides_network" = "True" ] || [ "$provides_network" = "true" ]; then
    fail "$label VM provides network: $vm"
  fi
}

verify_candidate() {
  candidate_tar="$1"
  tmpdir="$(mktemp -d)"

  tar -tf "$candidate_tar" |
    awk '
      $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" {
        printf "unexpected candidate tar member: %s\n", $0 > "/dev/stderr"
        exit 1
      }
    ' || fail "candidate tar contains unexpected paths"
  tar -C "$tmpdir" -xf "$candidate_tar"
  [ -f "$tmpdir/config.bundle" ] || fail "candidate tar is missing config.bundle"
  [ -f "$tmpdir/validation.json" ] || fail "candidate tar is missing validation.json"
  [ -f "$tmpdir/manifest.json" ] || fail "candidate tar is missing manifest.json"

  cp "$tmpdir/validation.json" "$VALIDATION_JSON"
  cp "$tmpdir/manifest.json" "$MANIFEST_JSON"

  manifest_schema="$(jq -r '.schema' "$MANIFEST_JSON")"
  manifest_commit="$(jq -r '.commit' "$MANIFEST_JSON")"
  validation_schema="$(jq -r '.schema' "$VALIDATION_JSON")"
  validation_status="$(jq -r '.status' "$VALIDATION_JSON")"
  validation_commit="$(jq -r '.commit' "$VALIDATION_JSON")"

  [ "$manifest_schema" = "nixome.config-candidate.v1" ] ||
    fail "unsupported candidate manifest schema: $manifest_schema"
  [ "$validation_schema" = "nixome.config-validation.v1" ] ||
    fail "unsupported validation schema: $validation_schema"
  [ "$validation_status" = "passed" ] ||
    fail "candidate validation did not pass: $validation_status"
  [ "$manifest_commit" = "$validation_commit" ] ||
    fail "candidate manifest commit $manifest_commit does not match validation commit $validation_commit"

  verify_repo="$tmpdir/verify.git"
  git init --bare -b main "$verify_repo" >/dev/null
  git -C "$verify_repo" bundle verify "$tmpdir/config.bundle" > "$BUNDLE_VERIFY_LOG" 2>&1 ||
    fail "candidate bundle verification failed"
  candidate_commit="$(
    git -C "$verify_repo" bundle list-heads "$tmpdir/config.bundle" |
      awk 'NR == 1 { print $1 }'
  )"

  [ -n "$candidate_commit" ] || fail "candidate bundle did not expose a commit"
  [ "$candidate_commit" = "$validation_commit" ] ||
    fail "candidate commit $candidate_commit does not match validation commit $validation_commit"

  rm -rf "$tmpdir"
}

extract_vault_archive_json() {
  awk '
    index($0, "\"schema\":\"nixome.config-vault-archive.v1\"") { line = $0 }
    END {
      if (line != "") {
        print line
        exit 0
      }
      exit 1
    }
  ' "$VAULT_ARCHIVE_LOG" > "$VAULT_ARCHIVE_JSON"
}

for cmd in \
  awk \
  cp \
  git \
  grep \
  jq \
  qvm-check \
  qvm-prefs \
  qvm-start \
  qrexec-client \
  systemctl \
  tar; do
  require_command "$cmd"
done

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VERIFY" >/dev/null 2>&1 || fail "missing config verify VM: $VERIFY"
qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"

verify_offline_vm "$STAGING" "config staging"
verify_offline_vm "$VERIFY" "config verify"
verify_offline_vm "$VAULT" "config vault"
start_vm "$STAGING"
start_vm "$VERIFY"
start_vm "$VAULT"

run_qrexec_service_to_file "$STAGING" "config candidate export from $STAGING" "$STAGED_CANDIDATE_TAR" "$EXPORT_STDERR" "$CANDIDATE_EXPORT_SERVICE"
run_qrexec_service_with_stdin "$VERIFY" "config candidate verification in $VERIFY" "$STAGED_CANDIDATE_TAR" "$VERIFIED_CANDIDATE_TAR" "$VERIFY_STDERR" "$CANDIDATE_VERIFY_SERVICE"
verify_candidate "$VERIFIED_CANDIDATE_TAR"

run_qrexec_service_with_stdin "$VAULT" "archive candidate into $VAULT" "$VERIFIED_CANDIDATE_TAR" "$VAULT_ARCHIVE_LOG" "$VAULT_ARCHIVE_STDERR" "$ARCHIVE_SERVICE"
extract_vault_archive_json || fail "vault archive command did not emit result JSON"

vault_archive_status="$(jq -r '.status' "$VAULT_ARCHIVE_JSON")"
vault_archive_commit="$(jq -r '.candidate_commit' "$VAULT_ARCHIVE_JSON")"
vault_commit="$(jq -r '.vault_commit' "$VAULT_ARCHIVE_JSON")"

[ "$vault_archive_status" = passed ] || fail "vault archive status is not passed: $vault_archive_status"
[ "$vault_archive_commit" = "$candidate_commit" ] ||
  fail "vault archived commit $vault_archive_commit does not match candidate commit $candidate_commit"

write_result passed "config-staging candidate verified in config-verify and archived in config-vault before deployment"
cat "$RESULT_JSON"
