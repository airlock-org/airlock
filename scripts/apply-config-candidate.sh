#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-apply.json"
COMMAND_LOG="$LOG_DIR/config-apply.commands.log"
ARCHIVE_STDOUT="$LOG_DIR/config-apply-archive.stdout.log"
ARCHIVE_STDERR="$LOG_DIR/config-apply-archive.stderr.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
VALIDATED_CANDIDATE_TAR="$LOG_DIR/config-candidate-validated.tar"
VALIDATED_TOPOLOGY_JSON="$LOG_DIR/config-apply-topology-validated.json"
APPLY_TOPOLOGY_JSON="$LOG_DIR/config-apply-topology.json"
TEMPLATE_BUILDS_JSON="$LOG_DIR/config-apply-template-builds.json"
TEMPLATE_BUILD_LOG="$LOG_DIR/config-apply-template-builds.log"
DEPLOYMENT_JSON="$LOG_DIR/config-deployment.json"
DEPLOYMENT_TAR="$LOG_DIR/config-deployment.tar"
DEPLOYMENT_ARCHIVE_STDOUT="$LOG_DIR/config-apply-deployment-archive.stdout.log"
DEPLOYMENT_ARCHIVE_STDERR="$LOG_DIR/config-apply-deployment-archive.stderr.log"
DEPLOYMENT_ARCHIVE_JSON="$LOG_DIR/config-vault-deployment-archive.json"
CLONE_LOG="$LOG_DIR/config-apply-git-clone.log"
CHECKOUT_LOG="$LOG_DIR/config-apply-git-checkout.log"
SETUP_STDOUT="$LOG_DIR/config-apply-setup.stdout.log"
SETUP_STDERR="$LOG_DIR/config-apply-setup.stderr.log"
SETUP_JSON="$LOG_DIR/setup-config.json"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
DEPLOYMENT_ARCHIVE_SERVICE="${NIXOME_CONFIG_DEPLOYMENT_ARCHIVE_SERVICE:-nixome.Config.ArchiveDeployment}"
APPLY=0
result_written=0
candidate_commit=""
deployment_id=""
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
  deployment_json="$(json_file_or_null "$DEPLOYMENT_JSON")"
  deployment_archive_json="$(json_file_or_null "$DEPLOYMENT_ARCHIVE_JSON")"

  jq -c -n \
    --arg schema "nixome.config-apply.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg staging "$STAGING" \
    --arg validator "$VALIDATOR" \
    --arg vault "$VAULT" \
    --arg candidate_commit "$candidate_commit" \
    --arg deployment_id "$deployment_id" \
    --arg deployment_archive_service "$DEPLOYMENT_ARCHIVE_SERVICE" \
    --arg topology "$APPLY_TOPOLOGY_JSON" \
    --arg validated_topology "$VALIDATED_TOPOLOGY_JSON" \
    --arg template_builds "$TEMPLATE_BUILDS_JSON" \
    --arg template_build_log "$TEMPLATE_BUILD_LOG" \
    --arg deployment_record "$DEPLOYMENT_JSON" \
    --arg deployment_tar "$DEPLOYMENT_TAR" \
    --arg deployment_archive_stdout "$DEPLOYMENT_ARCHIVE_STDOUT" \
    --arg deployment_archive_stderr "$DEPLOYMENT_ARCHIVE_STDERR" \
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
    --argjson deployment "$deployment_json" \
    --argjson deployment_archive "$deployment_archive_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      staging: $staging,
      validator: $validator,
      vault: $vault,
      candidate_commit: $candidate_commit,
      deployment_id: $deployment_id,
      deployment_archive_service: $deployment_archive_service,
      topology: $topology,
      validated_topology: $validated_topology,
      template_builds: $template_builds,
      template_build_log: $template_build_log,
      deployment_record: $deployment_record,
      deployment_tar: $deployment_tar,
      deployment_archive_stdout: $deployment_archive_stdout,
      deployment_archive_stderr: $deployment_archive_stderr,
      command_log: $command_log,
      archive_stdout: $archive_stdout,
      archive_stderr: $archive_stderr,
      validated_candidate_tar: $validated_candidate_tar,
      clone_log: $clone_log,
      checkout_log: $checkout_log,
      setup_stdout: $setup_stdout,
      setup_stderr: $setup_stderr,
      archive: $archive,
      setup: $setup,
      deployment: $deployment,
      deployment_archive: $deployment_archive
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

run_qrexec_service_with_stdin() {
  vm="$1"
  input="$2"
  output="$3"
  stderr="$4"
  service="$5"

  qrexec_args=(qrexec-client -d "$vm" -T -w 120 "DEFAULT:QUBESRPC $service+ dom0")
  printf '+'
  printf ' %q' "${qrexec_args[@]}"
  printf ' < %q\n' "$input"

  "${qrexec_args[@]}" < "$input" > "$output" 2> "$stderr"
}

copy_or_empty() {
  source="$1"
  target="$2"

  if [ -r "$source" ]; then
    cp "$source" "$target"
  else
    : > "$target"
  fi
}

build_deployment_tar() {
  status="$1"
  reason="$2"
  deployed_at="$(date -u +%Y%m%dT%H%M%SZ)"
  short_commit="$(printf '%s' "$candidate_commit" | cut -c1-12)"
  record_dir="$tmpdir/deployment-record"

  deployment_id="$deployed_at-$short_commit"
  rm -rf "$record_dir"
  mkdir -p "$record_dir/logs"

  archive_json="$(json_file_or_null "$ARCHIVE_JSON")"
  setup_json="$(json_file_or_null "$SETUP_JSON")"

  jq -c -n \
    --arg schema "nixome.config-deployment.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg deployment_id "$deployment_id" \
    --arg deployed_at "$deployed_at" \
    --arg deployed_by "dom0" \
    --arg staging "$STAGING" \
    --arg validator "$VALIDATOR" \
    --arg vault "$VAULT" \
    --arg candidate_commit "$candidate_commit" \
    --arg topology "topology.json" \
    --arg apply "apply.json" \
    --arg archive "archive.json" \
    --arg setup "setup.json" \
    --arg template_builds "template-builds.json" \
    --arg validated_topology "validated-topology.json" \
    --arg validation "validation.json" \
    --arg manifest "manifest.json" \
    --argjson archive_result "$archive_json" \
    --argjson setup_result "$setup_json" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      deployment_id: $deployment_id,
      deployed_at: $deployed_at,
      deployed_by: $deployed_by,
      staging: $staging,
      validator: $validator,
      vault: $vault,
      candidate_commit: $candidate_commit,
      artifacts: {
        topology: $topology,
        apply: $apply,
        archive: $archive,
        setup: $setup,
        template_builds: $template_builds,
        validated_topology: $validated_topology,
        validation: $validation,
        manifest: $manifest
      },
      archive: $archive_result,
      setup: $setup_result
    }' > "$DEPLOYMENT_JSON"

  write_result "$status" "$reason"

  cp "$DEPLOYMENT_JSON" "$record_dir/deployment.json"
  cp "$RESULT_JSON" "$record_dir/apply.json"
  copy_or_empty "$ARCHIVE_JSON" "$record_dir/archive.json"
  copy_or_empty "$SETUP_JSON" "$record_dir/setup.json"
  copy_or_empty "$APPLY_TOPOLOGY_JSON" "$record_dir/topology.json"
  copy_or_empty "$VALIDATED_TOPOLOGY_JSON" "$record_dir/validated-topology.json"
  copy_or_empty "$TEMPLATE_BUILDS_JSON" "$record_dir/template-builds.json"
  copy_or_empty "$tmpdir/validation.json" "$record_dir/validation.json"
  copy_or_empty "$tmpdir/manifest.json" "$record_dir/manifest.json"
  copy_or_empty "$COMMAND_LOG" "$record_dir/logs/apply.commands.log"
  copy_or_empty "$ARCHIVE_STDOUT" "$record_dir/logs/archive.stdout.log"
  copy_or_empty "$ARCHIVE_STDERR" "$record_dir/logs/archive.stderr.log"
  copy_or_empty "$SETUP_STDOUT" "$record_dir/logs/setup.stdout.log"
  copy_or_empty "$SETUP_STDERR" "$record_dir/logs/setup.stderr.log"
  copy_or_empty "$TEMPLATE_BUILD_LOG" "$record_dir/logs/template-builds.log"
  copy_or_empty "$CLONE_LOG" "$record_dir/logs/clone.log"
  copy_or_empty "$CHECKOUT_LOG" "$record_dir/logs/checkout.log"

  tar -C "$record_dir" -cf "$DEPLOYMENT_TAR" \
    deployment.json \
    apply.json \
    archive.json \
    setup.json \
    topology.json \
    validated-topology.json \
    template-builds.json \
    validation.json \
    manifest.json \
    logs/apply.commands.log \
    logs/archive.stdout.log \
    logs/archive.stderr.log \
    logs/setup.stdout.log \
    logs/setup.stderr.log \
    logs/template-builds.log \
    logs/clone.log \
    logs/checkout.log
}

extract_deployment_archive_json() {
  awk '
    index($0, "\"schema\":\"nixome.config-vault-deployment.v1\"") { line = $0 }
    END {
      if (line != "") {
        print line
        exit 0
      }
      exit 1
    }
  ' "$DEPLOYMENT_ARCHIVE_STDOUT" > "$DEPLOYMENT_ARCHIVE_JSON"
}

archive_deployment_or_fail() {
  status="$1"
  reason="$2"

  build_deployment_tar "$status" "$reason"

  if ! run_qrexec_service_with_stdin \
    "$VAULT" \
    "$DEPLOYMENT_TAR" \
    "$DEPLOYMENT_ARCHIVE_STDOUT" \
    "$DEPLOYMENT_ARCHIVE_STDERR" \
    "$DEPLOYMENT_ARCHIVE_SERVICE"; then
    cat "$DEPLOYMENT_ARCHIVE_STDERR" >&2 || true
    write_result failed "deployment archive failed after setup-config $status"
    printf 'config apply failed: deployment archive failed after setup-config %s\n' "$status" >&2
    exit 1
  fi

  extract_deployment_archive_json ||
    fail "vault deployment archive command did not emit result JSON"

  deployment_archive_status="$(jq -r '.status' "$DEPLOYMENT_ARCHIVE_JSON")"
  deployment_archive_candidate="$(jq -r '.candidate_commit' "$DEPLOYMENT_ARCHIVE_JSON")"
  [ "$deployment_archive_status" = passed ] ||
    fail "vault deployment archive status is not passed: $deployment_archive_status"
  [ "$deployment_archive_candidate" = "$candidate_commit" ] ||
    fail "vault deployment candidate did not match: $deployment_archive_candidate"

  write_result "$status" "$reason"
}

safe_log_name() {
  printf '%s' "$1" | sed -e 's/[^A-Za-z0-9_.-]/_/g'
}

init_template_builds_json() {
  : > "$TEMPLATE_BUILD_LOG"
  jq -c -n \
    --arg schema "nixome.template-builds.v1" \
    '{schema: $schema, builds: []}' \
    > "$TEMPLATE_BUILDS_JSON"
}

record_template_build() {
  template="$1"
  attr="$2"
  subpath="$3"
  status="$4"
  output_path="$5"
  source_path="$6"
  log="$7"
  tmp="$TEMPLATE_BUILDS_JSON.tmp"

  jq -c \
    --arg template "$template" \
    --arg attr "$attr" \
    --arg subpath "$subpath" \
    --arg status "$status" \
    --arg output_path "$output_path" \
    --arg source_path "$source_path" \
    --arg log "$log" \
    '.builds += [{
      template: $template,
      attr: $attr,
      subpath: $subpath,
      status: $status,
      output_path: $output_path,
      source_path: $source_path,
      log: $log
    }]' \
    "$TEMPLATE_BUILDS_JSON" > "$tmp"
  mv "$tmp" "$TEMPLATE_BUILDS_JSON"
}

resolve_template_sources() {
  topology="$1"
  worktree="$2"
  resolved="$3"

  cp "$topology" "$VALIDATED_TOPOLOGY_JSON"
  cp "$topology" "$resolved"
  init_template_builds_json

  mapfile -t template_refs < <(
    jq -r '
      .templates
      | to_entries[]
      | select(.value.source_ref != null)
      | @base64
    ' "$topology"
  )

  if [ "${#template_refs[@]}" -eq 0 ]; then
    printf 'no staged template build references to resolve\n' >> "$TEMPLATE_BUILD_LOG"
    return
  fi

  for encoded in "${template_refs[@]}"; do
    template_json="$(printf '%s' "$encoded" | base64 -d)"
    template="$(printf '%s' "$template_json" | jq -r '.key')"
    attr="$(printf '%s' "$template_json" | jq -r '.value.source_ref.attr')"
    subpath="$(printf '%s' "$template_json" | jq -r '.value.source_ref.subpath // ""')"
    safe_template="$(safe_log_name "$template")"
    build_log="$LOG_DIR/config-apply-template-build-$safe_template.log"
    build_stdout="$tmpdir/config-apply-template-build-$safe_template.stdout"

    {
      printf 'template=%s\n' "$template"
      printf 'attr=%s\n' "$attr"
      printf 'subpath=%s\n' "$subpath"
      printf '+ nix build --impure --offline --no-link --print-out-paths %s#%s\n' "$worktree" "$attr"
    } > "$build_log"

    if
      nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        build \
        --impure \
        --offline \
        --no-write-lock-file \
        --no-link \
        --print-out-paths \
        --show-trace \
        "$worktree#$attr" > "$build_stdout" 2>> "$build_log"
    then
      cat "$build_stdout" >> "$build_log"
      output_path="$(awk 'NF { last = $0 } END { print last }' "$build_stdout")"
    else
      record_template_build "$template" "$attr" "$subpath" failed "" "" "$build_log"
      cat "$build_log" >> "$TEMPLATE_BUILD_LOG" || true
      fail "template build failed for $template from $attr"
    fi

    [ -n "$output_path" ] ||
      fail "template build did not print an output path for $template from $attr"

    source_path="$output_path"
    if [ -n "$subpath" ]; then
      source_path="$source_path/$subpath"
    fi

    [ -d "$source_path" ] ||
      fail "built template source directory is missing for $template: $source_path"
    [ -f "$source_path/root.img" ] ||
      fail "built template source root.img is missing for $template: $source_path/root.img"
    [ -f "$source_path/template.conf" ] ||
      fail "built template source template.conf is missing for $template: $source_path/template.conf"

    tmp_resolved="$resolved.tmp"
    jq \
      --arg name "$template" \
      --arg source "$source_path" \
      '.templates[$name].source = $source | .templates[$name].source_ref = null' \
      "$resolved" > "$tmp_resolved"
    mv "$tmp_resolved" "$resolved"

    record_template_build "$template" "$attr" "$subpath" passed "$output_path" "$source_path" "$build_log"
    cat "$build_log" >> "$TEMPLATE_BUILD_LOG" || true
  done

  unresolved="$(
    jq -r '
      .templates
      | to_entries[]
      | select(.value.source_ref != null)
      | .key
    ' "$resolved"
  )"
  [ -z "$unresolved" ] ||
    fail "unresolved template source references remain: $unresolved"
}

for cmd in \
  awk \
  base64 \
  cat \
  cp \
  cut \
  date \
  git \
  grep \
  jq \
  mkdir \
  mktemp \
  mv \
  nix \
  nixome-config-archive-candidate \
  nixome-setup-config \
  qrexec-client \
  rm \
  sed \
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
    $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" {
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

topology="$tmpdir/topology.json"
[ -r "$topology" ] || fail "validated candidate is missing evaluated topology.json"
jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  (.domains | type == "object")
' "$topology" >/dev/null || fail "validated candidate topology JSON is invalid"
resolve_template_sources "$topology" "$worktree" "$APPLY_TOPOLOGY_JSON"

printf '+ env NIXOME_LOG_DIR=%q nixome-setup-config --apply --topology %q\n' \
  "$LOG_DIR" \
  "$APPLY_TOPOLOGY_JSON"
if env NIXOME_LOG_DIR="$LOG_DIR" nixome-setup-config --apply --topology "$APPLY_TOPOLOGY_JSON" \
  > "$SETUP_STDOUT" 2> "$SETUP_STDERR"; then
  setup_rc=0
else
  setup_rc="$?"
fi

[ -r "$SETUP_JSON" ] || fail "setup-config result JSON is missing: $SETUP_JSON"
setup_status="$(jq -r '.status' "$SETUP_JSON")"
if [ "$setup_rc" != 0 ] || [ "$setup_status" != passed ]; then
  archive_deployment_or_fail failed "setup-config failed"
  printf 'config apply failed: setup-config failed\n' >&2
  exit 1
fi

archive_deployment_or_fail passed "validated config candidate applied"
cat "$RESULT_JSON"
