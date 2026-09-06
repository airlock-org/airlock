#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-apply.json"
COMMAND_LOG="$LOG_DIR/config-apply.commands.log"
ARCHIVE_STDOUT="$LOG_DIR/config-apply-archive.stdout.log"
ARCHIVE_STDERR="$LOG_DIR/config-apply-archive.stderr.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
VALIDATED_CANDIDATE_TAR="$LOG_DIR/config-candidate-validated.tar"
APPLY_TOPOLOGY_JSON="$LOG_DIR/config-apply-topology.json"
CLONE_LOG="$LOG_DIR/config-apply-git-clone.log"
CHECKOUT_LOG="$LOG_DIR/config-apply-git-checkout.log"
SETUP_STDOUT="$LOG_DIR/config-apply-setup.stdout.log"
SETUP_STDERR="$LOG_DIR/config-apply-setup.stderr.log"
SETUP_JSON="$LOG_DIR/setup-config.json"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
TOPOLOGY_REL="${NIXOME_CONFIG_TOPOLOGY_PATH:-topology.json}"
APPLY=0
result_written=0
candidate_commit=""
tmpdir=""

usage() {
  cat >&2 <<'EOF'
Usage: nixome-config-apply --apply [--staging VM] [--validator VM] [--vault VM]

Validate a staged config candidate in config-validator, archive it into
config-vault, then apply the validated candidate topology from dom0.
EOF
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --apply)
      APPLY=1
      ;;
    --staging)
      shift
      [ "$#" -gt 0 ] || usage
      STAGING="$1"
      ;;
    --validator)
      shift
      [ "$#" -gt 0 ] || usage
      VALIDATOR="$1"
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

[ "$APPLY" = "1" ] || usage

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec > >(tee -a "$COMMAND_LOG") 2>&1
trap cleanup EXIT

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
  archive_json="$(json_file_or_null "$ARCHIVE_JSON")"
  setup_json="$(json_file_or_null "$SETUP_JSON")"

  jq -c -n \
    --arg schema "nixome.config-apply.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg staging "$STAGING" \
    --arg validator "$VALIDATOR" \
    --arg vault "$VAULT" \
    --arg candidate_commit "$candidate_commit" \
    --arg topology "$APPLY_TOPOLOGY_JSON" \
    --arg command_log "$COMMAND_LOG" \
    --arg archive_stdout "$ARCHIVE_STDOUT" \
    --arg archive_stderr "$ARCHIVE_STDERR" \
    --arg validated_candidate_tar "$VALIDATED_CANDIDATE_TAR" \
    --arg clone_log "$CLONE_LOG" \
    --arg checkout_log "$CHECKOUT_LOG" \
    --arg setup_stdout "$SETUP_STDOUT" \
    --arg setup_stderr "$SETUP_STDERR" \
    --argjson archive "$archive_json" \
    --argjson setup "$setup_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      staging: $staging,
      validator: $validator,
      vault: $vault,
      candidate_commit: $candidate_commit,
      topology: $topology,
      command_log: $command_log,
      archive_stdout: $archive_stdout,
      archive_stderr: $archive_stderr,
      validated_candidate_tar: $validated_candidate_tar,
      clone_log: $clone_log,
      checkout_log: $checkout_log,
      setup_stdout: $setup_stdout,
      setup_stderr: $setup_stderr,
      archive: $archive,
      setup: $setup
    }' > "$RESULT_JSON"
  result_written=1
}

cleanup() {
  rc="$?"

  if [ -n "$tmpdir" ]; then
    rm -rf "$tmpdir"
  fi

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    write_result failed "unexpected command failure; see command_log"
  fi
}

fail() {
  reason="$1"
  write_result failed "$reason"
  printf 'config apply failed: %s\n' "$reason" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

run_to_files_or_fail() {
  description="$1"
  stdout="$2"
  stderr="$3"
  shift 3

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if ! "$@" > "$stdout" 2> "$stderr"; then
    cat "$stderr" >&2 || true
    fail "$description failed"
  fi
}

case "$TOPOLOGY_REL" in
  ""|/*|../*|*/../*|*/..|..)
    fail "invalid relative topology path: $TOPOLOGY_REL"
    ;;
esac

for cmd in \
  awk \
  cp \
  git \
  grep \
  jq \
  mktemp \
  nixome-config-archive-candidate \
  nixome-setup-config \
  rm \
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

run_to_files_or_fail "config candidate archive" "$ARCHIVE_STDOUT" "$ARCHIVE_STDERR" \
  env \
    NIXOME_LOG_DIR="$LOG_DIR" \
    NIXOME_CONFIG_STAGING_VM="$STAGING" \
    NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    NIXOME_CONFIG_VAULT_VM="$VAULT" \
    nixome-config-archive-candidate \
      --staging "$STAGING" \
      --validator "$VALIDATOR" \
      --vault "$VAULT"

[ -r "$ARCHIVE_JSON" ] || fail "archive result JSON is missing: $ARCHIVE_JSON"

archive_status="$(jq -r '.status' "$ARCHIVE_JSON")"
candidate_commit="$(jq -r '.candidate_commit' "$ARCHIVE_JSON")"
validation_status="$(jq -r '.validation.status' "$ARCHIVE_JSON")"

[ "$archive_status" = passed ] || fail "archive status is not passed: $archive_status"
[ "$validation_status" = passed ] || fail "validation status is not passed: $validation_status"
[ -n "$candidate_commit" ] && [ "$candidate_commit" != null ] ||
  fail "archive result did not include candidate_commit"
[ -r "$VALIDATED_CANDIDATE_TAR" ] ||
  fail "validated candidate tar is missing: $VALIDATED_CANDIDATE_TAR"

tmpdir="$(mktemp -d)"
tar -tf "$VALIDATED_CANDIDATE_TAR" |
  awk '
    $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" {
      printf "unexpected validated candidate tar member: %s\n", $0 > "/dev/stderr"
      exit 1
    }
  ' || fail "validated candidate tar contains unexpected paths"
tar -C "$tmpdir" -xf "$VALIDATED_CANDIDATE_TAR"

worktree="$tmpdir/worktree"
git clone "$tmpdir/config.bundle" "$worktree" > "$CLONE_LOG" 2>&1 ||
  fail "clone validated candidate bundle failed"
git -C "$worktree" checkout --detach "$candidate_commit" > "$CHECKOUT_LOG" 2>&1 ||
  fail "checkout validated candidate commit failed"

topology="$worktree/$TOPOLOGY_REL"
[ -r "$topology" ] || fail "validated candidate is missing topology: $TOPOLOGY_REL"
jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  (.domains | type == "object")
' "$topology" >/dev/null || fail "validated candidate topology JSON is invalid"
cp "$topology" "$APPLY_TOPOLOGY_JSON"

run_to_files_or_fail "setup-config apply from validated candidate" "$SETUP_STDOUT" "$SETUP_STDERR" \
  env NIXOME_LOG_DIR="$LOG_DIR" nixome-setup-config --apply --topology "$APPLY_TOPOLOGY_JSON"

[ -r "$SETUP_JSON" ] || fail "setup-config result JSON is missing: $SETUP_JSON"
setup_status="$(jq -r '.status' "$SETUP_JSON")"
[ "$setup_status" = passed ] || fail "setup-config status is not passed: $setup_status"

write_result passed "validated config candidate applied"
cat "$RESULT_JSON"
