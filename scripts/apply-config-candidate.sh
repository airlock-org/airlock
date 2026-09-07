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
VALIDATED_SYSTEM_JSON="$LOG_DIR/config-apply-system-validated.json"
APPLY_SYSTEM_JSON="$LOG_DIR/config-apply-system.json"
SYSTEM_PLAN_JSON="$LOG_DIR/config-apply-system-plan.json"
SYSTEM_BUILDS_JSON="$LOG_DIR/config-apply-system-builds.json"
SYSTEM_BUILD_LOG="$LOG_DIR/config-apply-system-builds.log"
SYSTEM_APPLY_JSON="$LOG_DIR/config-system-apply.json"
SYSTEM_APPLY_LOG="$LOG_DIR/config-system-apply.log"
PROFILE_ACTIVATION_LOG="$LOG_DIR/config-system-profile-activation.log"
SYSTEMD_ACTIONS_JSON="$LOG_DIR/config-system-systemd-actions.json"
SYSTEMD_ACTIONS_LOG="$LOG_DIR/config-system-systemd-actions.log"
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
SYSTEM_STATE_DIR="${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
SYSTEM_ETC_DIR="${NIXOME_SYSTEM_ETC_DIR:-/etc/nixome}"
SYSTEM_PROFILE="${NIXOME_SYSTEM_PROFILE:-/nix/var/nix/profiles/nixome-system}"
HOST_SYSTEM_PROFILE="${NIXOME_HOST_SYSTEM_PROFILE:-/nix/var/nix/profiles/system}"
HOST_CURRENT_SYSTEM_LINK="${NIXOME_HOST_CURRENT_SYSTEM_LINK:-/run/current-system}"
HOST_SYSTEM_ACTIVATION_LOG="$LOG_DIR/config-system-host-activation.log"
CONFIG_APPLY_NIX_OFFLINE="${NIXOME_CONFIG_APPLY_NIX_OFFLINE:-1}"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
DEPLOYMENT_ARCHIVE_SERVICE="${NIXOME_CONFIG_DEPLOYMENT_ARCHIVE_SERVICE:-nixome.Config.ArchiveDeployment}"
APPLY=0
result_written=0
candidate_commit=""
deployment_id=""
tmpdir=""

nixome_store_uses_chroot() {
  [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] &&
    [ -n "${NIXOME_NIX_ROOT:-}" ] &&
    [ "$NIXOME_NIX_ROOT" != "/" ]
}

nixome_physical_store_path() {
  local path="$1"

  if nixome_store_uses_chroot && [ "${path#/nix/store/}" != "$path" ]; then
    printf '%s%s\n' "$NIXOME_NIX_ROOT" "$path"
  else
    printf '%s\n' "$path"
  fi
}

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

exec >> "$COMMAND_LOG" 2>&1
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
  system_apply_json="$(json_file_or_null "$SYSTEM_APPLY_JSON")"
  systemd_actions_json="$(json_file_or_null "$SYSTEMD_ACTIONS_JSON")"
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
    --arg system "$APPLY_SYSTEM_JSON" \
    --arg validated_system "$VALIDATED_SYSTEM_JSON" \
    --arg system_plan "$SYSTEM_PLAN_JSON" \
    --arg system_builds "$SYSTEM_BUILDS_JSON" \
    --arg system_build_log "$SYSTEM_BUILD_LOG" \
    --arg system_apply_path "$SYSTEM_APPLY_JSON" \
    --arg system_apply_log "$SYSTEM_APPLY_LOG" \
    --arg profile_activation_log "$PROFILE_ACTIVATION_LOG" \
    --arg host_system_activation_log "$HOST_SYSTEM_ACTIVATION_LOG" \
    --arg systemd_actions "$SYSTEMD_ACTIONS_JSON" \
    --arg systemd_actions_log "$SYSTEMD_ACTIONS_LOG" \
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
    --argjson system_apply "$system_apply_json" \
    --argjson deployment "$deployment_json" \
    --argjson deployment_archive "$deployment_archive_json" \
    --argjson systemd_actions_result "$systemd_actions_json" \
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
      system: $system,
      validated_system: $validated_system,
      system_plan: $system_plan,
      system_builds: $system_builds,
      system_build_log: $system_build_log,
      system_apply: $system_apply_path,
      system_apply_log: $system_apply_log,
      profile_activation_log: $profile_activation_log,
      host_system_activation_log: $host_system_activation_log,
      systemd_actions: $systemd_actions,
      systemd_actions_log: $systemd_actions_log,
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
      system_apply_result: $system_apply,
      systemd_actions_result: $systemd_actions_result,
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

nix_build_print_path() {
  local worktree="$1"
  local attr="$2"
  local stdout="$3"
  local log="$4"
  local command=(
    nix
    --extra-experimental-features nix-command
    --extra-experimental-features flakes
    build
    --impure
    --no-write-lock-file
    --no-link
    --print-out-paths
    --show-trace
  )

  case "$CONFIG_APPLY_NIX_OFFLINE" in
    1|true|yes)
      command+=(--offline)
      ;;
    0|false|no)
      ;;
    *)
      fail "unsupported NIXOME_CONFIG_APPLY_NIX_OFFLINE value: $CONFIG_APPLY_NIX_OFFLINE"
      ;;
  esac

  command+=("$worktree#$attr")

  {
    printf '+'
    printf ' %q' "${command[@]}"
    printf '\n'
  } >> "$log"

  "${command[@]}" > "$stdout" 2>> "$log"
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
  system_apply_json="$(json_file_or_null "$SYSTEM_APPLY_JSON")"
  systemd_actions_json="$(json_file_or_null "$SYSTEMD_ACTIONS_JSON")"

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
    --arg system "system.json" \
    --arg resolved_system "resolved-system.json" \
    --arg system_plan "system-plan.json" \
    --arg system_apply "system-apply.json" \
    --arg system_builds "system-builds.json" \
    --arg systemd_actions "systemd-actions.json" \
    --arg apply "apply.json" \
    --arg archive "archive.json" \
    --arg setup "setup.json" \
    --arg template_builds "template-builds.json" \
    --arg validated_topology "validated-topology.json" \
    --arg validation "validation.json" \
    --arg manifest "manifest.json" \
    --argjson archive_result "$archive_json" \
    --argjson setup_result "$setup_json" \
    --argjson system_apply_result "$system_apply_json" \
    --argjson systemd_actions_result "$systemd_actions_json" \
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
        system: $system,
        resolved_system: $resolved_system,
        system_plan: $system_plan,
        system_apply: $system_apply,
        system_builds: $system_builds,
        systemd_actions: $systemd_actions,
        apply: $apply,
        archive: $archive,
        setup: $setup,
        template_builds: $template_builds,
        validated_topology: $validated_topology,
        validation: $validation,
        manifest: $manifest
      },
      archive: $archive_result,
      setup: $setup_result,
      system_apply: $system_apply_result,
      systemd_actions: $systemd_actions_result
    }' > "$DEPLOYMENT_JSON"

  write_result "$status" "$reason"

  cp "$DEPLOYMENT_JSON" "$record_dir/deployment.json"
  cp "$RESULT_JSON" "$record_dir/apply.json"
  copy_or_empty "$ARCHIVE_JSON" "$record_dir/archive.json"
  copy_or_empty "$SETUP_JSON" "$record_dir/setup.json"
  copy_or_empty "$APPLY_TOPOLOGY_JSON" "$record_dir/topology.json"
  copy_or_empty "$VALIDATED_TOPOLOGY_JSON" "$record_dir/validated-topology.json"
  copy_or_empty "$TEMPLATE_BUILDS_JSON" "$record_dir/template-builds.json"
  copy_or_empty "$VALIDATED_SYSTEM_JSON" "$record_dir/system.json"
  copy_or_empty "$APPLY_SYSTEM_JSON" "$record_dir/resolved-system.json"
  copy_or_empty "$SYSTEM_PLAN_JSON" "$record_dir/system-plan.json"
  copy_or_empty "$SYSTEM_APPLY_JSON" "$record_dir/system-apply.json"
  copy_or_empty "$SYSTEM_BUILDS_JSON" "$record_dir/system-builds.json"
  copy_or_empty "$SYSTEMD_ACTIONS_JSON" "$record_dir/systemd-actions.json"
  copy_or_empty "$tmpdir/validation.json" "$record_dir/validation.json"
  copy_or_empty "$tmpdir/manifest.json" "$record_dir/manifest.json"
  copy_or_empty "$COMMAND_LOG" "$record_dir/logs/apply.commands.log"
  copy_or_empty "$ARCHIVE_STDOUT" "$record_dir/logs/archive.stdout.log"
  copy_or_empty "$ARCHIVE_STDERR" "$record_dir/logs/archive.stderr.log"
  copy_or_empty "$SETUP_STDOUT" "$record_dir/logs/setup.stdout.log"
  copy_or_empty "$SETUP_STDERR" "$record_dir/logs/setup.stderr.log"
  copy_or_empty "$TEMPLATE_BUILD_LOG" "$record_dir/logs/template-builds.log"
  copy_or_empty "$SYSTEM_BUILD_LOG" "$record_dir/logs/system-builds.log"
  copy_or_empty "$SYSTEM_APPLY_LOG" "$record_dir/logs/system-apply.log"
  copy_or_empty "$PROFILE_ACTIVATION_LOG" "$record_dir/logs/profile-activation.log"
  copy_or_empty "$HOST_SYSTEM_ACTIVATION_LOG" "$record_dir/logs/host-system-activation.log"
  copy_or_empty "$SYSTEMD_ACTIONS_LOG" "$record_dir/logs/systemd-actions.log"
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
    system.json \
    resolved-system.json \
    system-plan.json \
    system-apply.json \
    system-builds.json \
    systemd-actions.json \
    validation.json \
    manifest.json \
    logs/apply.commands.log \
    logs/archive.stdout.log \
    logs/archive.stderr.log \
    logs/setup.stdout.log \
    logs/setup.stderr.log \
    logs/template-builds.log \
    logs/system-builds.log \
    logs/system-apply.log \
    logs/profile-activation.log \
    logs/host-system-activation.log \
    logs/systemd-actions.log \
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
  persist_deployment_state "$status"
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
    } > "$build_log"

    if nix_build_print_path "$worktree" "$attr" "$build_stdout" "$build_log"; then
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
    source_path_physical="$(nixome_physical_store_path "$source_path")"

    [ -d "$source_path_physical" ] ||
      fail "built template source directory is missing for $template: $source_path"
    [ -f "$source_path_physical/root.img" ] ||
      fail "built template source root.img is missing for $template: $source_path/root.img"
    [ -f "$source_path_physical/template.conf" ] ||
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

init_system_builds_json() {
  : > "$SYSTEM_BUILD_LOG"
  jq -c -n \
    --arg schema "nixome.system-builds.v1" \
    '{schema: $schema, builds: []}' \
    > "$SYSTEM_BUILDS_JSON"
}

record_system_build() {
  component="$1"
  attr="$2"
  subpath="$3"
  status="$4"
  output_path="$5"
  source_path="$6"
  log="$7"
  tmp="$SYSTEM_BUILDS_JSON.tmp"

  jq -c \
    --arg component "$component" \
    --arg attr "$attr" \
    --arg subpath "$subpath" \
    --arg status "$status" \
    --arg output_path "$output_path" \
    --arg source_path "$source_path" \
    --arg log "$log" \
    '.builds += [{
      component: $component,
      attr: $attr,
      subpath: $subpath,
      status: $status,
      output_path: $output_path,
      source_path: $source_path,
      log: $log
    }]' \
    "$SYSTEM_BUILDS_JSON" > "$tmp"
  mv "$tmp" "$SYSTEM_BUILDS_JSON"
}

init_systemd_actions_json() {
  : > "$SYSTEMD_ACTIONS_LOG"
  jq -c -n \
    --arg schema "nixome.systemd-actions.v1" \
    '{schema: $schema, status: "skipped", reason: "no systemd live actions declared", actions: []}' \
    > "$SYSTEMD_ACTIONS_JSON"
}

finish_systemd_actions_json() {
  local status="$1"
  local reason="$2"
  local tmp="$SYSTEMD_ACTIONS_JSON.tmp"

  jq -c \
    --arg status "$status" \
    --arg reason "$reason" \
    '.status = $status | .reason = $reason' \
    "$SYSTEMD_ACTIONS_JSON" > "$tmp"
  mv "$tmp" "$SYSTEMD_ACTIONS_JSON"
}

record_systemd_action() {
  local name="$1"
  local action="$2"
  local reason="$3"
  local status="$4"
  local log="$5"
  local tmp="$SYSTEMD_ACTIONS_JSON.tmp"

  jq -c \
    --arg name "$name" \
    --arg action "$action" \
    --arg reason "$reason" \
    --arg status "$status" \
    --arg log "$log" \
    '.actions += [{
      name: $name,
      action: $action,
      reason: $reason,
      status: $status,
      log: $log
    }]' \
    "$SYSTEMD_ACTIONS_JSON" > "$tmp"
  mv "$tmp" "$SYSTEMD_ACTIONS_JSON"
}

apply_systemd_actions() {
  local system_json="$1"
  local encoded
  local service_json
  local name
  local action
  local service_reason
  local safe_service
  local action_log
  local overall_status=skipped
  local overall_reason="no systemd live actions declared"

  init_systemd_actions_json

  mapfile -t service_actions < <(
    jq -r '
      (.dom0.systemd.services // {})
      | to_entries[]
      | select(.value.action != "none")
      | @base64
    ' "$system_json"
  )

  if [ "${#service_actions[@]}" -eq 0 ]; then
    printf 'no systemd live actions declared\n' >> "$SYSTEMD_ACTIONS_LOG"
    finish_systemd_actions_json "$overall_status" "$overall_reason"
    return 0
  fi

  overall_status=passed
  overall_reason="systemd live actions applied"

  for encoded in "${service_actions[@]}"; do
    service_json="$(printf '%s' "$encoded" | base64 -d)"
    name="$(printf '%s' "$service_json" | jq -r '.key')"
    action="$(printf '%s' "$service_json" | jq -r '.value.action')"
    service_reason="$(printf '%s' "$service_json" | jq -r '.value.reason')"
    safe_service="$(safe_log_name "$name")"
    action_log="$LOG_DIR/config-system-systemd-$safe_service-$action.log"

    case "$action" in
      restart|reload) ;;
      *)
        printf 'unsupported systemd action for %s: %s\n' "$name" "$action" > "$action_log"
        record_systemd_action "$name" "$action" "$service_reason" failed "$action_log"
        finish_systemd_actions_json failed "unsupported systemd live action: $action $name"
        cat "$action_log" >> "$SYSTEMD_ACTIONS_LOG" || true
        return 1
        ;;
    esac

    {
      printf 'service=%s\n' "$name"
      printf 'action=%s\n' "$action"
      printf 'reason=%s\n' "$service_reason"
      printf '+ systemctl %q %q\n' "$action" "$name"
    } >> "$SYSTEMD_ACTIONS_LOG"

    if systemctl "$action" "$name" > "$action_log" 2>&1; then
      record_systemd_action "$name" "$action" "$service_reason" passed "$action_log"
    else
      record_systemd_action "$name" "$action" "$service_reason" failed "$action_log"
      finish_systemd_actions_json failed "systemd live action failed: $action $name"
      cat "$action_log" >> "$SYSTEMD_ACTIONS_LOG" || true
      return 1
    fi

    cat "$action_log" >> "$SYSTEMD_ACTIONS_LOG" || true
  done

  finish_systemd_actions_json "$overall_status" "$overall_reason"
}

resolve_candidate_source_path() {
  component="$1"
  worktree="$2"
  source="$3"

  case "$source" in
    "")
      fail "empty direct system source for $component"
      ;;
    /*)
      printf '%s\n' "$source"
      ;;
    .|./|..|../*|*/..|*/../*|*//*)
      fail "unsafe relative system source for $component: $source"
      ;;
    ./*)
      printf '%s/%s\n' "$worktree" "${source#./}"
      ;;
    *)
      printf '%s/%s\n' "$worktree" "$source"
      ;;
  esac
}

validate_system_source() {
  component="$1"
  source="$2"
  source_physical="$3"
  system_json="$4"

  if [ "$component" = "dom0.profile" ]; then
    activation_command="$(jq -r '.dom0.profile.activation_command // "bin/nixome-dom0-profile-activate"' "$system_json")"
    [ -d "$source_physical" ] ||
      fail "direct dom0 profile source is not a directory: $source"
    [ -x "$source_physical/$activation_command" ] ||
      fail "direct dom0 profile activation command is missing or not executable: $source/$activation_command"
  fi

  if [ "$component" = "dom0.nixos.system" ]; then
    [ -d "$source_physical" ] ||
      fail "dom0 NixOS system source is not a directory: $source"
    [ -x "$source_physical/bin/switch-to-configuration" ] ||
      fail "dom0 NixOS system source is missing bin/switch-to-configuration: $source"
  fi
}

resolve_direct_system_sources() {
  resolved="$1"
  worktree="$2"

  mapfile -t direct_sources < <(
    jq -r '
      [
        { component: "dom0.profile", path: ["dom0", "profile"], source: .dom0.profile.source },
        { component: "dom0.nixos.system", path: ["dom0", "nixos", "system"], source: .dom0.nixos.system.source },
        { component: "dom0.nixos.kernel", path: ["dom0", "nixos", "kernel"], source: .dom0.nixos.kernel.source },
        { component: "dom0.nixos.xen", path: ["dom0", "nixos", "xen"], source: .dom0.nixos.xen.source },
        { component: "dom0.nixos.bootloader", path: ["dom0", "nixos", "bootloader"], source: .dom0.nixos.bootloader.source },
        { component: "dom0.nixos.qubes", path: ["dom0", "nixos", "qubes"], source: .dom0.nixos.qubes.source }
      ]
      | .[]
      | select(.source != null)
      | @base64
    ' "$resolved"
  )

  if [ "${#direct_sources[@]}" -eq 0 ]; then
    printf 'no direct system sources to resolve\n' >> "$SYSTEM_BUILD_LOG"
    return
  fi

  for encoded in "${direct_sources[@]}"; do
    source_json="$(printf '%s' "$encoded" | base64 -d)"
    component="$(printf '%s' "$source_json" | jq -r '.component')"
    source="$(printf '%s' "$source_json" | jq -r '.source')"
    source_path="$(printf '%s' "$source_json" | jq -c '.path')"
    resolved_source="$(resolve_candidate_source_path "$component" "$worktree" "$source")"
    resolved_source_physical="$(nixome_physical_store_path "$resolved_source")"
    source_was_relative=0

    case "$source" in
      /*) ;;
      *) source_was_relative=1 ;;
    esac

    [ -e "$resolved_source_physical" ] ||
      fail "direct system source is missing for $component: $source"

    if [ "$source_was_relative" = "1" ]; then
      persistent_source="$SYSTEM_STATE_DIR/sources/$candidate_commit/$(safe_log_name "$component")"
      rm -rf "$persistent_source"
      mkdir -p "$(dirname "$persistent_source")"
      cp -a "$resolved_source_physical" "$persistent_source"
      resolved_source="$persistent_source"
      resolved_source_physical="$persistent_source"
    fi

    validate_system_source "$component" "$source" "$resolved_source_physical" "$resolved"

    printf 'direct system source component=%s source=%s resolved=%s\n' \
      "$component" \
      "$source" \
      "$resolved_source" >> "$SYSTEM_BUILD_LOG"

    tmp_resolved="$resolved.tmp"
    jq \
      --argjson path "$source_path" \
      --arg source "$resolved_source" \
      'setpath($path + ["source"]; $source)' \
      "$resolved" > "$tmp_resolved"
    mv "$tmp_resolved" "$resolved"
  done
}

resolve_system_sources() {
  system_json="$1"
  worktree="$2"
  resolved="$3"

  cp "$system_json" "$VALIDATED_SYSTEM_JSON"
  cp "$system_json" "$resolved"
  init_system_builds_json

  mapfile -t system_refs < <(
    jq -r '
      [
        { component: "dom0.profile", path: ["dom0", "profile"], ref: .dom0.profile.source_ref },
        { component: "dom0.nixos.system", path: ["dom0", "nixos", "system"], ref: .dom0.nixos.system.source_ref },
        { component: "dom0.nixos.kernel", path: ["dom0", "nixos", "kernel"], ref: .dom0.nixos.kernel.source_ref },
        { component: "dom0.nixos.xen", path: ["dom0", "nixos", "xen"], ref: .dom0.nixos.xen.source_ref },
        { component: "dom0.nixos.bootloader", path: ["dom0", "nixos", "bootloader"], ref: .dom0.nixos.bootloader.source_ref },
        { component: "dom0.nixos.qubes", path: ["dom0", "nixos", "qubes"], ref: .dom0.nixos.qubes.source_ref }
      ]
      | .[]
      | select(.ref != null)
      | @base64
    ' "$system_json"
  )

  if [ "${#system_refs[@]}" -eq 0 ]; then
    printf 'no staged system build references to resolve\n' >> "$SYSTEM_BUILD_LOG"
  else
    for encoded in "${system_refs[@]}"; do
    ref_json="$(printf '%s' "$encoded" | base64 -d)"
    component="$(printf '%s' "$ref_json" | jq -r '.component')"
    attr="$(printf '%s' "$ref_json" | jq -r '.ref.attr')"
    subpath="$(printf '%s' "$ref_json" | jq -r '.ref.subpath // ""')"
    ref_path="$(printf '%s' "$ref_json" | jq -c '.path')"
    safe_component="$(safe_log_name "$component")"
    build_log="$LOG_DIR/config-apply-system-build-$safe_component.log"
    build_stdout="$tmpdir/config-apply-system-build-$safe_component.stdout"

    {
      printf 'component=%s\n' "$component"
      printf 'attr=%s\n' "$attr"
      printf 'subpath=%s\n' "$subpath"
    } > "$build_log"

    if nix_build_print_path "$worktree" "$attr" "$build_stdout" "$build_log"; then
      cat "$build_stdout" >> "$build_log"
      output_path="$(awk 'NF { last = $0 } END { print last }' "$build_stdout")"
    else
      record_system_build "$component" "$attr" "$subpath" failed "" "" "$build_log"
      cat "$build_log" >> "$SYSTEM_BUILD_LOG" || true
      cat "$build_log" >&2 || true
      fail "system build failed for $component from $attr"
    fi

    [ -n "$output_path" ] ||
      fail "system build did not print an output path for $component from $attr"

    source_path="$output_path"
    if [ -n "$subpath" ]; then
      source_path="$source_path/$subpath"
    fi
    source_path_physical="$(nixome_physical_store_path "$source_path")"

    [ -e "$source_path_physical" ] ||
      fail "built system source is missing for $component: $source_path"
    validate_system_source "$component" "$source_path" "$source_path_physical" "$resolved"

    tmp_resolved="$resolved.tmp"
    jq \
      --argjson path "$ref_path" \
      --arg source "$source_path" \
      'setpath($path + ["source"]; $source) | setpath($path + ["source_ref"]; null)' \
      "$resolved" > "$tmp_resolved"
    mv "$tmp_resolved" "$resolved"

    record_system_build "$component" "$attr" "$subpath" passed "$output_path" "$source_path" "$build_log"
    cat "$build_log" >> "$SYSTEM_BUILD_LOG" || true
    done
  fi

  resolve_direct_system_sources "$resolved" "$worktree"
}

install_profile_link() {
  profile_source="$1"
  profile_source_physical="$2"
  profile_parent="$(dirname "$SYSTEM_PROFILE")"

  mkdir -p "$profile_parent"

  if [ "${profile_source#/nix/store/}" = "$profile_source" ]; then
    ln -sfn "$profile_source_physical" "$SYSTEM_PROFILE"
  elif nixome_store_uses_chroot; then
    ln -sfn "$profile_source_physical" "$SYSTEM_PROFILE"
  else
    nix-env -p "$SYSTEM_PROFILE" --set "$profile_source"
  fi
}

install_host_system_profile() {
  host_system_source="$1"
  host_system_source_physical="$2"
  host_profile_parent="$(dirname "$HOST_SYSTEM_PROFILE")"

  mkdir -p "$host_profile_parent"

  if [ "${host_system_source#/nix/store/}" = "$host_system_source" ]; then
    ln -sfn "$host_system_source_physical" "$HOST_SYSTEM_PROFILE"
  elif nixome_store_uses_chroot; then
    ln -sfn "$host_system_source_physical" "$HOST_SYSTEM_PROFILE"
  else
    nix-env -p "$HOST_SYSTEM_PROFILE" --set "$host_system_source"
  fi
}

write_system_apply_json() {
  local systemd_actions_result

  if [ ! -r "$SYSTEMD_ACTIONS_JSON" ]; then
    init_systemd_actions_json
  fi

  cp "$system_json" "$SYSTEM_ETC_DIR/system.json"
  cp "$SYSTEM_PLAN_JSON" "$SYSTEM_ETC_DIR/system-plan.json"
  cp "$SYSTEM_BUILDS_JSON" "$SYSTEM_STATE_DIR/last-system-builds.json"
  cp "$SYSTEMD_ACTIONS_JSON" "$SYSTEM_STATE_DIR/last-systemd-actions.json"

  systemd_actions_result="$(json_file_or_null "$SYSTEMD_ACTIONS_JSON")"
  jq -c -n \
    --arg schema "nixome.system-apply.v1" \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg profile_status "$profile_status" \
    --arg host_system_status "$host_system_status" \
    --arg systemd_status "$systemd_status" \
    --arg profile_source "$profile_source" \
    --arg profile_source_physical "$profile_source_physical" \
    --arg profile "$SYSTEM_PROFILE" \
    --arg host_system_source "$host_system_source" \
    --arg host_system_source_physical "$host_system_source_physical" \
    --arg host_system_profile "$HOST_SYSTEM_PROFILE" \
    --arg host_current_system_link "$HOST_CURRENT_SYSTEM_LINK" \
    --arg host_current_system "$host_current_system" \
    --arg host_system_switch_action "$host_system_switch_action" \
    --arg state_dir "$SYSTEM_STATE_DIR" \
    --arg etc_dir "$SYSTEM_ETC_DIR" \
    --arg plan "$SYSTEM_PLAN_JSON" \
    --arg system "$system_json" \
    --arg build_log "$SYSTEM_BUILD_LOG" \
    --arg activation_log "$PROFILE_ACTIVATION_LOG" \
    --arg host_system_activation_log "$HOST_SYSTEM_ACTIVATION_LOG" \
    --arg systemd_actions "$SYSTEMD_ACTIONS_JSON" \
    --arg systemd_actions_log "$SYSTEMD_ACTIONS_LOG" \
    --argjson reboot_actions "$reboot_actions" \
    --argjson reboot_required "$reboot_required" \
    --argjson reboot_reasons "$reboot_reasons" \
    --argjson systemd_actions_result "$systemd_actions_result" \
    '{
      schema: $schema,
      status: $status,
      reason: $reason,
      profile_status: $profile_status,
      host_system_status: $host_system_status,
      systemd_status: $systemd_status,
      profile_source: $profile_source,
      profile_source_physical: $profile_source_physical,
      profile: $profile,
      host_system_source: $host_system_source,
      host_system_source_physical: $host_system_source_physical,
      host_system_profile: $host_system_profile,
      host_current_system_link: $host_current_system_link,
      host_current_system: $host_current_system,
      host_system_switch_action: $host_system_switch_action,
      state_dir: $state_dir,
      etc_dir: $etc_dir,
      system: $system,
      plan: $plan,
      build_log: $build_log,
      activation_log: $activation_log,
      host_system_activation_log: $host_system_activation_log,
      systemd_actions: $systemd_actions,
      systemd_actions_log: $systemd_actions_log,
      systemd_actions_result: $systemd_actions_result,
      reboot_actions: $reboot_actions,
      reboot_required: $reboot_required,
      reboot_reasons: $reboot_reasons
    }' > "$SYSTEM_APPLY_JSON"
  cp "$SYSTEM_APPLY_JSON" "$SYSTEM_STATE_DIR/last-apply.json"
}

finish_system_apply_failed() {
  status=failed
  reason="$1"
  printf 'system apply failed: %s\n' "$reason" >> "$SYSTEM_APPLY_LOG"
  write_system_apply_json
}

persist_current_topology() {
  mkdir -p "$SYSTEM_STATE_DIR"
  cp "$APPLY_TOPOLOGY_JSON" "$SYSTEM_STATE_DIR/current-topology.json"
  cp "$VALIDATED_TOPOLOGY_JSON" "$SYSTEM_STATE_DIR/current-validated-topology.json"
  cp "$SETUP_JSON" "$SYSTEM_STATE_DIR/current-setup.json"
}

persist_deployment_state() {
  local deployment_status="$1"

  mkdir -p "$SYSTEM_STATE_DIR"
  cp "$DEPLOYMENT_JSON" "$SYSTEM_STATE_DIR/last-deployment.json"
  cp "$RESULT_JSON" "$SYSTEM_STATE_DIR/last-config-apply.json"
  printf '%s\n' "$deployment_id" > "$SYSTEM_STATE_DIR/last-deployment-id"
  printf '%s\n' "$candidate_commit" > "$SYSTEM_STATE_DIR/last-candidate-commit"

  if [ "$deployment_status" = passed ]; then
    cp "$DEPLOYMENT_JSON" "$SYSTEM_STATE_DIR/current-deployment.json"
    cp "$RESULT_JSON" "$SYSTEM_STATE_DIR/current-config-apply.json"
    printf '%s\n' "$deployment_id" > "$SYSTEM_STATE_DIR/current-deployment-id"
    printf '%s\n' "$candidate_commit" > "$SYSTEM_STATE_DIR/current-candidate-commit"
  fi
}

apply_system() {
  system_json="$1"
  status=passed
  reason="system plan applied"
  profile_status=skipped
  host_system_status=skipped
  systemd_status=skipped
  profile_source=""
  profile_source_physical=""
  profile_activation_command=""
  host_system_enabled=false
  host_system_source=""
  host_system_source_physical=""
  host_system_switch_action=""
  host_current_system=""
  systemd_apply_rc=0
  reboot_actions="$(jq -c '.plan.reboot_actions' "$system_json")"
  reboot_required="$(jq -r '.plan.reboot_required' "$system_json")"
  reboot_reasons="$(jq -c '.plan.reboot_reasons' "$system_json")"

  : > "$SYSTEM_APPLY_LOG"
  : > "$HOST_SYSTEM_ACTIVATION_LOG"
  jq -c '.plan' "$system_json" > "$SYSTEM_PLAN_JSON"

  mkdir -p "$SYSTEM_STATE_DIR" "$SYSTEM_ETC_DIR"
  cp "$system_json" "$SYSTEM_STATE_DIR/last-system.json"
  cp "$SYSTEM_PLAN_JSON" "$SYSTEM_STATE_DIR/last-plan.json"

  profile_source="$(jq -r '.dom0.profile.source // ""' "$system_json")"
  profile_activation_command="$(jq -r '.dom0.profile.activation_command // "bin/nixome-dom0-profile-activate"' "$system_json")"
  profile_activation_action="$(jq -r '.dom0.profile.activation_action // "run"' "$system_json")"

  if [ -n "$profile_source" ]; then
    profile_source_physical="$(nixome_physical_store_path "$profile_source")"
    activation_path="$profile_source_physical/$profile_activation_command"
    if [ ! -d "$profile_source_physical" ]; then
      finish_system_apply_failed "dom0 profile source is missing: $profile_source"
      return 1
    fi
    if [ ! -x "$activation_path" ]; then
      finish_system_apply_failed "dom0 profile activation command is missing or not executable: $profile_source/$profile_activation_command"
      return 1
    fi

    {
      printf 'profile_source=%s\n' "$profile_source"
      printf 'profile_source_physical=%s\n' "$profile_source_physical"
      printf 'profile=%s\n' "$SYSTEM_PROFILE"
      printf 'activation_command=%s\n' "$profile_activation_command"
      printf 'activation_action=%s\n' "$profile_activation_action"
    } >> "$SYSTEM_APPLY_LOG"

    if ! install_profile_link "$profile_source" "$profile_source_physical" >> "$SYSTEM_APPLY_LOG" 2>&1; then
      finish_system_apply_failed "installing dom0 profile failed"
      return 1
    fi

    if [ "$profile_activation_action" = run ]; then
      if ! env \
        NIXOME_SYSTEM_JSON="$system_json" \
        NIXOME_SYSTEM_PLAN_JSON="$SYSTEM_PLAN_JSON" \
        NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
        NIXOME_SYSTEM_ETC_DIR="$SYSTEM_ETC_DIR" \
        NIXOME_SYSTEM_PROFILE="$SYSTEM_PROFILE" \
        "$activation_path" > "$PROFILE_ACTIVATION_LOG" 2>&1; then
        cat "$PROFILE_ACTIVATION_LOG" >&2 || true
        finish_system_apply_failed "dom0 profile activation failed"
        return 1
      fi
    else
      printf 'activation action is %s; not running profile activation\n' "$profile_activation_action" > "$PROFILE_ACTIVATION_LOG"
    fi

    profile_status=passed
  else
    printf 'no dom0 profile source declared\n' > "$PROFILE_ACTIVATION_LOG"
  fi

  host_system_enabled="$(jq -r '.dom0.nixos.system.enable // false' "$system_json")"
  host_system_source="$(jq -r '.dom0.nixos.system.source // ""' "$system_json")"
  host_system_switch_action="$(jq -r '.dom0.nixos.system.switch_action // "switch"' "$system_json")"

  if [ "$host_system_enabled" = true ]; then
    if [ -z "$host_system_source" ]; then
      finish_system_apply_failed "dom0 NixOS system is enabled but source is empty"
      return 1
    fi

    case "$host_system_switch_action" in
      switch|boot|test|dry-activate) ;;
      *)
        finish_system_apply_failed "unsupported dom0 NixOS switch action: $host_system_switch_action"
        return 1
        ;;
    esac

    host_system_source_physical="$(nixome_physical_store_path "$host_system_source")"
    switch_to_configuration="$host_system_source_physical/bin/switch-to-configuration"

    if [ ! -d "$host_system_source_physical" ]; then
      finish_system_apply_failed "dom0 NixOS system source is missing: $host_system_source"
      return 1
    fi
    if [ ! -x "$switch_to_configuration" ]; then
      finish_system_apply_failed "dom0 NixOS switch-to-configuration is missing or not executable: $host_system_source/bin/switch-to-configuration"
      return 1
    fi

    {
      printf 'host_system_source=%s\n' "$host_system_source"
      printf 'host_system_source_physical=%s\n' "$host_system_source_physical"
      printf 'host_system_profile=%s\n' "$HOST_SYSTEM_PROFILE"
      printf 'host_current_system_link=%s\n' "$HOST_CURRENT_SYSTEM_LINK"
      printf 'switch_action=%s\n' "$host_system_switch_action"
    } >> "$SYSTEM_APPLY_LOG"

    if ! install_host_system_profile "$host_system_source" "$host_system_source_physical" >> "$SYSTEM_APPLY_LOG" 2>&1; then
      finish_system_apply_failed "installing dom0 NixOS system profile failed"
      return 1
    fi

    if ! "$switch_to_configuration" "$host_system_switch_action" > "$HOST_SYSTEM_ACTIVATION_LOG" 2>&1; then
      cat "$HOST_SYSTEM_ACTIVATION_LOG" >&2 || true
      finish_system_apply_failed "dom0 NixOS switch-to-configuration $host_system_switch_action failed"
      return 1
    fi

    if [ -L "$HOST_CURRENT_SYSTEM_LINK" ]; then
      host_current_system="$(readlink "$HOST_CURRENT_SYSTEM_LINK" || true)"
    fi

    printf '%s\n' "$host_system_source" > "$SYSTEM_STATE_DIR/current-host-system"
    printf '%s\n' "$HOST_SYSTEM_PROFILE" > "$SYSTEM_STATE_DIR/current-host-system-profile"
    printf '%s\n' "$host_system_switch_action" > "$SYSTEM_STATE_DIR/current-host-system-switch-action"
    host_system_status=passed
  else
    printf 'dom0 NixOS system generation is not enabled\n' > "$HOST_SYSTEM_ACTIVATION_LOG"
  fi

  if apply_systemd_actions "$system_json"; then
    systemd_status="$(jq -r '.status' "$SYSTEMD_ACTIONS_JSON")"
  else
    systemd_apply_rc="$?"
    systemd_status="$(jq -r '.status // "failed"' "$SYSTEMD_ACTIONS_JSON" 2>/dev/null || printf 'failed')"
    status=failed
    reason="systemd live actions failed"
  fi

  write_system_apply_json
  [ "$systemd_apply_rc" = 0 ]
}

for cmd in \
  awk \
  base64 \
  cat \
  cp \
  cut \
  date \
  dirname \
  git \
  grep \
  jq \
  ln \
  mkdir \
  mktemp \
  mv \
  nix \
  nix-env \
  nixome-config-archive-candidate \
  nixome-setup-config \
  qrexec-client \
  readlink \
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
    $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" && $0 != "system.json" {
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
system_json="$tmpdir/system.json"
[ -r "$topology" ] || fail "validated candidate is missing evaluated topology.json"
[ -r "$system_json" ] || fail "validated candidate is missing evaluated system.json"
jq -e '
  .schema == "nixome.qubes-topology.v1" and
  (.templates | type == "object") and
  (.kernels | type == "object") and
  (.prefs | type == "object") and
  (.domains | type == "object")
' "$topology" >/dev/null || fail "validated candidate topology JSON is invalid"
jq -e '
  .schema == "nixome.system.v1" and
    (.dom0 | type == "object") and
    (.plan | type == "object") and
    (.plan.live_actions | type == "array") and
    (.plan.reboot_actions | type == "array") and
    (.plan.reboot_required | type == "boolean") and
    (.plan.reboot_reasons | type == "array")
' "$system_json" >/dev/null || fail "validated candidate system JSON is invalid"
resolve_template_sources "$topology" "$worktree" "$APPLY_TOPOLOGY_JSON"
resolve_system_sources "$system_json" "$worktree" "$APPLY_SYSTEM_JSON"

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
persist_current_topology

if apply_system "$APPLY_SYSTEM_JSON"; then
  system_apply_rc=0
else
  system_apply_rc="$?"
fi

[ -r "$SYSTEM_APPLY_JSON" ] || fail "system apply result JSON is missing: $SYSTEM_APPLY_JSON"
system_apply_status="$(jq -r '.status' "$SYSTEM_APPLY_JSON")"
if [ "$system_apply_rc" != 0 ] || [ "$system_apply_status" != passed ]; then
  archive_deployment_or_fail failed "system apply failed"
  printf 'config apply failed: system apply failed\n' >&2
  exit 1
fi

archive_deployment_or_fail passed "validated config candidate applied"
cat "$RESULT_JSON"
