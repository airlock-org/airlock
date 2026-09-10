#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-archive.json"
COMMAND_LOG="$LOG_DIR/config-archive.commands.log"
STAGED_CANDIDATE_TAR="$LOG_DIR/config-candidate-staged.tar"
VALIDATED_CANDIDATE_TAR="$LOG_DIR/config-candidate-validated.tar"
EXPORT_STDERR="$LOG_DIR/config-candidate-export.stderr.log"
VALIDATE_STDERR="$LOG_DIR/config-candidate-validate.stderr.log"
VALIDATION_JSON="$LOG_DIR/config-candidate-validation.json"
MANIFEST_JSON="$LOG_DIR/config-candidate-manifest.json"
TOPOLOGY_JSON="$LOG_DIR/config-candidate-topology.json"
SYSTEM_JSON="$LOG_DIR/config-candidate-system.json"
DOMAIN_REFERENCES_JSON="$LOG_DIR/config-candidate-domain-references.json"
BUNDLE_VERIFY_LOG="$LOG_DIR/config-candidate-bundle-verify.log"
VAULT_ARCHIVE_LOG="$LOG_DIR/config-vault-archive.log"
VAULT_ARCHIVE_STDERR="$LOG_DIR/config-vault-archive.stderr.log"
VAULT_ARCHIVE_JSON="$LOG_DIR/config-vault-archive.json"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
CANDIDATE_EXPORT_SERVICE="${NIXOME_CONFIG_CANDIDATE_EXPORT_SERVICE:-nixome.Config.CandidateExport}"
CANDIDATE_VALIDATE_SERVICE="${NIXOME_CONFIG_CANDIDATE_VALIDATE_SERVICE:-nixome.Config.ValidateCandidate}"
ARCHIVE_SERVICE="${NIXOME_CONFIG_ARCHIVE_SERVICE:-nixome.Config.ArchiveCandidate}"
result_written=0
candidate_commit=""
validation_commit=""
vault_commit=""
CANDIDATE_TAR=""
VALIDATE_ONLY=0

usage() {
  cat >&2 <<'EOF'
Usage: nixome-config-archive-candidate [--staging VM] [--validator VM] [--vault VM]
       nixome-config-archive-candidate --candidate-tar PATH [--validate-only] [--validator VM] [--vault VM]

Fetch a config candidate from config-staging, validate it in config-validator, and
archive the validated candidate into config-vault before any deployment attempt.
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
    --candidate-tar)
      shift
      [ "$#" -gt 0 ] || usage
      CANDIDATE_TAR="$1"
      ;;
    --validate-only)
      VALIDATE_ONLY=1
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

exec >> "$COMMAND_LOG" 2>&1
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
  domain_references_json="$(json_file_or_null "$DOMAIN_REFERENCES_JSON")"

  jq -c -n \
    --arg schema "nixome.config-archive.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg staging "$STAGING" \
    --arg validator "$VALIDATOR" \
    --arg vault "$VAULT" \
    --arg candidate_export_service "$CANDIDATE_EXPORT_SERVICE" \
    --arg candidate_validate_service "$CANDIDATE_VALIDATE_SERVICE" \
    --arg archive_service "$ARCHIVE_SERVICE" \
    --arg candidate_commit "$candidate_commit" \
    --arg validation_commit "$validation_commit" \
    --arg vault_commit "$vault_commit" \
    --arg staged_candidate_tar "$STAGED_CANDIDATE_TAR" \
    --arg validated_candidate_tar "$VALIDATED_CANDIDATE_TAR" \
    --arg topology "$TOPOLOGY_JSON" \
    --arg system "$SYSTEM_JSON" \
    --arg domain_references "$DOMAIN_REFERENCES_JSON" \
    --arg command_log "$COMMAND_LOG" \
    --arg export_stderr "$EXPORT_STDERR" \
    --arg validate_stderr "$VALIDATE_STDERR" \
    --arg bundle_verify_log "$BUNDLE_VERIFY_LOG" \
    --arg vault_archive_log "$VAULT_ARCHIVE_LOG" \
    --arg vault_archive_stderr "$VAULT_ARCHIVE_STDERR" \
    --argjson validation "$validation_json" \
    --argjson vault_archive "$vault_archive_json" \
    --argjson domain_references_result "$domain_references_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      staging: $staging,
      validator: $validator,
      vault: $vault,
      candidate_export_service: $candidate_export_service,
      candidate_validate_service: $candidate_validate_service,
      archive_service: $archive_service,
      candidate_commit: $candidate_commit,
      validation_commit: $validation_commit,
      vault_commit: $vault_commit,
      staged_candidate_tar: $staged_candidate_tar,
      validated_candidate_tar: $validated_candidate_tar,
      topology: $topology,
      system: $system,
      domain_references: $domain_references,
      command_log: $command_log,
      export_stderr: $export_stderr,
      validate_stderr: $validate_stderr,
      bundle_verify_log: $bundle_verify_log,
      vault_archive_stderr: $vault_archive_stderr,
      vault_archive_log: $vault_archive_log,
      validation: $validation,
      domain_references_result: $domain_references_result,
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

write_domain_reference_report() {
  local topology_file="$1"
  local references_json="$2"
  local missing_json="$3"
  local existing_domains_file="$4"
  local status="$5"
  local reason="$6"

  jq -c -n \
    --arg schema "nixome.domain-references.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg topology "$topology_file" \
    --slurpfile references "$references_json" \
    --slurpfile missing "$missing_json" \
    --rawfile existing_domains "$existing_domains_file" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      topology: $topology,
      existing_domains: ($existing_domains | split("\n") | map(select(. != ""))),
      references: ($references[0] // []),
      missing: ($missing[0] // [])
    }' > "$DOMAIN_REFERENCES_JSON"
}

validate_domain_references() {
  local topology_file="$1"
  local work_dir="$2"
  local references_json="$work_dir/domain-references.json"
  local missing_json="$work_dir/domain-references-missing.json"
  local existing_domains_file="$work_dir/domain-references-existing.txt"
  local reference_count
  local missing_count

  jq -c '
    def ref($kind; $domain; $field; $target):
      if $target == null or $target == "" or $target == "none" or $target == "None" then
        empty
      else
        {
          kind: $kind,
          domain: $domain,
          field: $field,
          target: ($target | tostring)
        }
      end;

    [
      (.domains // {} | to_entries[] | .key as $domain | .value as $domain_cfg |
        ref("netvm"; $domain; "netvm"; $domain_cfg.netvm),
        if (($domain_cfg.backend.type // "qubes-template") == "qubes-template") then
          ref("template"; $domain; "template"; ($domain_cfg.template // $domain_cfg.backend.template))
        else
          empty
        end
      ),
      (.prefs // {} | to_entries[] |
        select(.key == "clockvm"
          or .key == "default_dispvm"
          or .key == "default_netvm"
          or .key == "default_template"
          or .key == "management_dispvm"
          or .key == "updatevm") |
        ref("global-pref"; null; .key; .value)
      ),
      (.qrexec_policy.rules // [] | to_entries[] | .key as $index | .value as $rule |
        ref("qrexec-source"; null; ("qrexec_policy.rules[" + ($index | tostring) + "].source"); $rule.source),
        ref("qrexec-target"; null; ("qrexec_policy.rules[" + ($index | tostring) + "].target"); $rule.target)
      )
    ]
    | map(select(.target != "dom0" and .target != "@anyvm"))
  ' "$topology_file" > "$references_json"

  reference_count="$(jq -r 'length' "$references_json")"
  if [ "$reference_count" = "0" ]; then
    : > "$existing_domains_file"
    printf '[]\n' > "$missing_json"
    write_domain_reference_report \
      "$topology_file" \
      "$references_json" \
      "$missing_json" \
      "$existing_domains_file" \
      passed \
      "topology has no domain references"
    return 0
  fi

  if ! command -v qvm-ls >/dev/null 2>&1; then
    : > "$existing_domains_file"
    printf '%s\n' "$(<"$references_json")" > "$missing_json"
    write_domain_reference_report \
      "$topology_file" \
      "$references_json" \
      "$missing_json" \
      "$existing_domains_file" \
      failed \
      "missing command: qvm-ls"
    fail "missing command: qvm-ls"
  fi

  if ! qvm-ls --raw-data --fields NAME --no-spinner > "$existing_domains_file"; then
    : > "$existing_domains_file"
    printf '%s\n' "$(<"$references_json")" > "$missing_json"
    write_domain_reference_report \
      "$topology_file" \
      "$references_json" \
      "$missing_json" \
      "$existing_domains_file" \
      failed \
      "cannot query existing Qubes domains"
    fail "cannot query existing Qubes domains for reference validation"
  fi

  jq -c \
    --slurpfile references "$references_json" \
    --rawfile existing_domains "$existing_domains_file" \
    '
      ((.domains // {}) | keys) as $declared_domains |
      ((.templates // {}) | keys) as $declared_templates |
      ($existing_domains | split("\n") | map(select(. != ""))) as $existing_domains_list |
      ($declared_domains + $declared_templates + $existing_domains_list) as $known_domains |
      ($references[0] // [])
      | map(select(.target as $target | ($known_domains | index($target)) == null))
    ' "$topology_file" > "$missing_json"

  missing_count="$(jq -r 'length' "$missing_json")"
  if [ "$missing_count" != "0" ]; then
    write_domain_reference_report \
      "$topology_file" \
      "$references_json" \
      "$missing_json" \
      "$existing_domains_file" \
      failed \
      "topology references missing domains"
    jq -r '
      .[]
      | if .domain == null then
          "\(.field) references missing domain \(.target)"
        else
          "domain \(.domain) \(.field) references missing domain \(.target)"
        end
    ' "$missing_json" >&2 || true
    fail "topology references missing domains"
  fi

  write_domain_reference_report \
    "$topology_file" \
    "$references_json" \
    "$missing_json" \
    "$existing_domains_file" \
    passed \
    "all referenced domains are declared by the candidate or exist in Qubes"
}

verify_candidate() {
  candidate_tar="$1"
  tmpdir="$(mktemp -d)"

  tar -tf "$candidate_tar" |
    awk '
    $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" && $0 != "system.json" {
        printf "unexpected candidate tar member: %s\n", $0 > "/dev/stderr"
        exit 1
      }
    ' || fail "candidate tar contains unexpected paths"
  tar -C "$tmpdir" -xf "$candidate_tar"
  [ -f "$tmpdir/config.bundle" ] || fail "candidate tar is missing config.bundle"
  [ -f "$tmpdir/validation.json" ] || fail "candidate tar is missing validation.json"
  [ -f "$tmpdir/manifest.json" ] || fail "candidate tar is missing manifest.json"
  [ -f "$tmpdir/topology.json" ] || fail "candidate tar is missing topology.json"
  [ -f "$tmpdir/system.json" ] || fail "candidate tar is missing system.json"

  cp "$tmpdir/validation.json" "$VALIDATION_JSON"
  cp "$tmpdir/manifest.json" "$MANIFEST_JSON"
  cp "$tmpdir/topology.json" "$TOPOLOGY_JSON"
  cp "$tmpdir/system.json" "$SYSTEM_JSON"

  jq -e '
    .schema == "nixome.qubes-topology.v1" and
    (.templates | type == "object") and
    (.kernels | type == "object") and
    (.prefs | type == "object") and
    (.domains | type == "object")
  ' "$TOPOLOGY_JSON" >/dev/null || fail "evaluated candidate topology JSON is invalid"
  jq -e '
    .schema == "nixome.system.v1" and
    (.dom0 | type == "object") and
    (.plan | type == "object") and
    (.plan.live_actions | type == "array") and
    (.plan.reboot_actions | type == "array") and
    (.plan.reboot_required | type == "boolean") and
    (.plan.reboot_reasons | type == "array")
  ' "$SYSTEM_JSON" >/dev/null || fail "evaluated candidate system JSON is invalid"

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

  validate_domain_references "$TOPOLOGY_JSON" "$tmpdir"

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

if [ -n "$CANDIDATE_TAR" ] && [ ! -r "$CANDIDATE_TAR" ]; then
  fail "candidate tar is not readable: $CANDIDATE_TAR"
fi

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

if [ -z "$CANDIDATE_TAR" ]; then
  qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
fi
qvm-check "$VALIDATOR" >/dev/null 2>&1 || fail "missing config validator VM: $VALIDATOR"
if [ "$VALIDATE_ONLY" != "1" ]; then
  qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"
fi

if [ -z "$CANDIDATE_TAR" ]; then
  verify_offline_vm "$STAGING" "config staging"
fi
verify_offline_vm "$VALIDATOR" "config validator"
if [ "$VALIDATE_ONLY" != "1" ]; then
  verify_offline_vm "$VAULT" "config vault"
fi
if [ -z "$CANDIDATE_TAR" ]; then
  start_vm "$STAGING"
fi
start_vm "$VALIDATOR"
if [ "$VALIDATE_ONLY" != "1" ]; then
  start_vm "$VAULT"
fi

if [ -n "$CANDIDATE_TAR" ]; then
  cp "$CANDIDATE_TAR" "$STAGED_CANDIDATE_TAR"
else
  run_qrexec_service_to_file "$STAGING" "config candidate export from $STAGING" "$STAGED_CANDIDATE_TAR" "$EXPORT_STDERR" "$CANDIDATE_EXPORT_SERVICE"
fi
run_qrexec_service_with_stdin "$VALIDATOR" "config candidate validation in $VALIDATOR" "$STAGED_CANDIDATE_TAR" "$VALIDATED_CANDIDATE_TAR" "$VALIDATE_STDERR" "$CANDIDATE_VALIDATE_SERVICE"
verify_candidate "$VALIDATED_CANDIDATE_TAR"

if [ "$VALIDATE_ONLY" = "1" ]; then
  write_result passed "config candidate validated in config-validator"
  cat "$RESULT_JSON"
  exit 0
fi

run_qrexec_service_with_stdin "$VAULT" "archive candidate into $VAULT" "$VALIDATED_CANDIDATE_TAR" "$VAULT_ARCHIVE_LOG" "$VAULT_ARCHIVE_STDERR" "$ARCHIVE_SERVICE"
extract_vault_archive_json || fail "vault archive command did not emit result JSON"

vault_archive_status="$(jq -r '.status' "$VAULT_ARCHIVE_JSON")"
vault_archive_commit="$(jq -r '.candidate_commit' "$VAULT_ARCHIVE_JSON")"
vault_commit="$(jq -r '.vault_commit' "$VAULT_ARCHIVE_JSON")"

[ "$vault_archive_status" = passed ] || fail "vault archive status is not passed: $vault_archive_status"
[ "$vault_archive_commit" = "$candidate_commit" ] ||
  fail "vault archived commit $vault_archive_commit does not match candidate commit $candidate_commit"

write_result passed "config-staging candidate validated in config-validator and archived in config-vault before deployment"
cat "$RESULT_JSON"
