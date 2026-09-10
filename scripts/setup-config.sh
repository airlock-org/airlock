#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euo pipefail

PATH="/run/wrappers/bin:$PATH"
export PATH

TOPOLOGY="/etc/nixome/qubes-topology.json"
APPLY=0
PLAN=0
LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/setup-config.json"
COMMAND_LOG="$LOG_DIR/setup-config.commands.log"
LIVE_ACTIONS_JSONL="$LOG_DIR/setup-config-live-actions.jsonl"
RESTART_ACTIONS_JSONL="$LOG_DIR/setup-config-restart-actions.jsonl"
UNSUPPORTED_DOMAINS_JSONL="$LOG_DIR/setup-config-unsupported-domains.jsonl"
SYSTEM_STATE_DIR="${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
PERSIST_STATE="${NIXOME_SETUP_CONFIG_PERSIST_STATE:-1}"
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"
KERNEL_BASE="${NIXOME_QUBES_KERNEL_BASE:-/var/lib/qubes/vm-kernels}"
QUBES_XML="${NIXOME_QUBES_XML:-/var/lib/qubes/qubes.xml}"
POLICY_DIR="${NIXOME_QUBES_POLICY_DIR:-/var/lib/nixome/qubes-policy.d}"
MANAGED_POLICY_FILE="${NIXOME_QUBES_MANAGED_POLICY_FILE:-90-nixome.policy}"
MANAGED_POLICY_PATH="$POLICY_DIR/$MANAGED_POLICY_FILE"
RENDERED_POLICY="$LOG_DIR/$MANAGED_POLICY_FILE.rendered"
NIX_DOMAIN_STORAGE_HELPER="${NIXOME_NIX_DOMAIN_STORAGE_HELPER:-nixome-nix-domain-storage}"
result_written=0

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
Usage: nixome-setup-config (--plan|--apply) [--topology PATH]

Set up the Qubes templates and domains declared by Nixome.
EOF
  exit 1
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --apply)
      APPLY=1
      ;;
    --plan)
      PLAN=1
      ;;
    --topology)
      shift
      [ "$#" -gt 0 ] || usage
      TOPOLOGY="$1"
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

if [ "$APPLY" = "1" ] && [ "$PLAN" = "1" ]; then
  usage
fi

if [ "$APPLY" != "1" ] && [ "$PLAN" != "1" ]; then
  usage
fi

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"
: > "$LIVE_ACTIONS_JSONL"
: > "$RESTART_ACTIONS_JSONL"
: > "$UNSUPPORTED_DOMAINS_JSONL"

exec >> "$COMMAND_LOG" 2>&1
trap write_unexpected_failure EXIT

templates_changed=0
kernels_changed=0
domains_changed=0
prefs_changed=0
features_changed=0
services_changed=0
qrexec_policy_changed=0
unsupported_domains_count=0
root_images_changed=0
store_volumes_changed=0
state_volumes_changed=0
store_views_changed=0
qubesd_restarts=0

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

json_lines_array() {
  local file="$1"
  local first=1
  local line

  printf '['
  if [ -s "$file" ]; then
    while IFS= read -r line; do
      [ -n "$line" ] || continue
      if [ "$first" = "1" ]; then
        first=0
      else
        printf ','
      fi
      printf '%s' "$line"
    done < "$file"
  fi
  printf ']'
}

record_action() {
  local file="$1"
  local type="$2"
  local action="$3"
  local domain="${4:-}"
  local field="${5:-}"
  local value="${6:-}"
  local reason="${7:-}"
  local state="${8:-}"

  jq -c -n \
    --arg type "$type" \
    --arg action "$action" \
    --arg domain "$domain" \
    --arg field "$field" \
    --arg value "$value" \
    --arg reason "$reason" \
    --arg state "$state" \
    '{
      type: $type,
      action: $action
    }
    + (if $domain != "" then {domain: $domain} else {} end)
    + (if $field != "" then {field: $field} else {} end)
    + (if $value != "" then {value: $value} else {} end)
    + (if $reason != "" then {reason: $reason} else {} end)
    + (if $state != "" then {state: $state} else {} end)' \
    >> "$file"
}

record_live_action() {
  record_action "$LIVE_ACTIONS_JSONL" "$@"
}

record_unsupported_domain() {
  local domain="$1"
  local backend="$2"
  local reason="$3"

  jq -c -n \
    --arg domain "$domain" \
    --arg backend "$backend" \
    --arg reason "$reason" \
    '{
      domain: $domain,
      backend: $backend,
      reason: $reason
    }' \
    >> "$UNSUPPORTED_DOMAINS_JSONL"
  unsupported_domains_count=$((unsupported_domains_count + 1))
}

domain_state() {
  qvm-ls --raw-data --fields NAME,STATE --no-spinner 2>/dev/null |
    awk -F'|' -v name="$1" '$1 == name { print $2; found = 1 } END { exit(found ? 0 : 1) }'
}

record_domain_restart_if_running() {
  local domain="$1"
  local type="$2"
  local field="$3"
  local value="$4"
  local state=""

  [ "$APPLY" = "1" ] || return 0
  state="$(domain_state "$domain" || true)"
  [ "$state" = "Running" ] || return 0

  record_action \
    "$RESTART_ACTIONS_JSONL" \
    "$type" \
    restart \
    "$domain" \
    "$field" \
    "$value" \
    "domain is running; restart required for this change to fully take effect" \
    "$state"
}

domain_is_running() {
  local domain="$1"
  local state=""

  [ "$APPLY" = "1" ] || return 1
  state="$(domain_state "$domain" || true)"
  [ "$state" = "Running" ]
}

record_running_domain_storage_change() {
  local domain="$1"
  local type="$2"
  local field="$3"
  local value="$4"
  local state=""

  state="$(domain_state "$domain" || true)"
  record_action \
    "$RESTART_ACTIONS_JSONL" \
    "$type" \
    restart \
    "$domain" \
    "$field" \
    "$value" \
    "domain is running; stop it and re-run setup-config for this storage change to take effect" \
    "$state"
}

write_result() {
  status="$1"
  reason="$2"

  {
    printf '{'
    printf '"schema":"nixome.setup-config.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"mode":'
    if [ "$APPLY" = "1" ]; then
      json_string "apply"
    else
      json_string "plan"
    fi
    printf ',"reason":'
    json_string "$reason"
    printf ',"topology":'
    json_string "$TOPOLOGY"
    printf ',"templates_changed":%s' "$templates_changed"
    printf ',"kernels_changed":%s' "$kernels_changed"
    printf ',"domains_changed":%s' "$domains_changed"
    printf ',"prefs_changed":%s' "$prefs_changed"
    printf ',"features_changed":%s' "$features_changed"
    printf ',"services_changed":%s' "$services_changed"
    printf ',"qrexec_policy_changed":%s' "$qrexec_policy_changed"
    printf ',"root_images_changed":%s' "$root_images_changed"
    printf ',"store_volumes_changed":%s' "$store_volumes_changed"
    printf ',"state_volumes_changed":%s' "$state_volumes_changed"
    printf ',"store_views_changed":%s' "$store_views_changed"
    printf ',"qubesd_restarts":%s' "$qubesd_restarts"
    printf ',"live_actions":'
    json_lines_array "$LIVE_ACTIONS_JSONL"
    printf ',"restart_actions":'
    json_lines_array "$RESTART_ACTIONS_JSONL"
    printf ',"unsupported_domains":'
    json_lines_array "$UNSUPPORTED_DOMAINS_JSONL"
    printf ',"unsupported_domains_count":%s' "$unsupported_domains_count"
    printf ',"restart_required":'
    if [ -s "$RESTART_ACTIONS_JSONL" ]; then
      printf true
    else
      printf false
    fi
    printf ',"live_actions_log":'
    json_string "$LIVE_ACTIONS_JSONL"
    printf ',"restart_actions_log":'
    json_string "$RESTART_ACTIONS_JSONL"
    printf ',"unsupported_domains_log":'
    json_string "$UNSUPPORTED_DOMAINS_JSONL"
    printf ',"qrexec_policy_file":'
    json_string "$MANAGED_POLICY_PATH"
    printf ',"qrexec_policy_rendered":'
    json_string "$RENDERED_POLICY"
    printf ',"command_log":'
    json_string "$COMMAND_LOG"
    printf '}\n'
  } > "$RESULT_JSON"
  result_written=1
}

fail() {
  reason="$1"
  write_result failed "$reason"
  printf 'nixome setup-config failed: %s\n' "$reason" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    write_result failed "unexpected command failure; see command_log"
  fi
}

run_or_plan() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if [ "$APPLY" = "1" ]; then
    set +e
    "$@"
    rc="$?"
    set -e
    if [ "$rc" != "0" ]; then
      printf 'command failed with exit %s:' "$rc"
      printf ' %q' "$@"
      printf '\n'
      return "$rc"
    fi
  fi
}

restart_qubesd_after_storage_change() {
  printf '+ systemctl try-restart qubesd.service\n'

  if [ "$APPLY" = "1" ]; then
    systemctl try-restart qubesd.service
  fi

  qubesd_restarts=$((qubesd_restarts + 1))
  record_live_action systemd try-restart "" service qubesd.service
}

persist_setup_state() {
  case "$PERSIST_STATE" in
    1|true|yes) ;;
    0|false|no)
      printf 'setup state persistence disabled\n'
      return 0
      ;;
    *)
      fail "unsupported NIXOME_SETUP_CONFIG_PERSIST_STATE value: $PERSIST_STATE"
      ;;
  esac

  [ "$APPLY" = "1" ] || return 0

  mkdir -p "$SYSTEM_STATE_DIR"
  cp "$TOPOLOGY" "$SYSTEM_STATE_DIR/current-topology.json"
  cp "$RESULT_JSON" "$SYSTEM_STATE_DIR/current-setup.json"
  printf 'persisted setup state: %s\n' "$SYSTEM_STATE_DIR"
}

domain_exists() {
  qvm-ls --raw-data --fields NAME --no-spinner 2>/dev/null |
    grep -Fx "$1" >/dev/null 2>&1
}

domain_class() {
  qvm-ls --raw-data --fields NAME,CLASS --no-spinner 2>/dev/null |
    awk -F'|' -v name="$1" '$1 == name { print $2; found = 1 } END { exit(found ? 0 : 1) }'
}

template_exists() {
  domain_exists "$1"
}

jq_get() {
  filter="$1"
  shift
  jq -r "$@" "$filter" "$TOPOLOGY"
}

json_value() {
  filter="$1"
  shift
  jq -r "$@" "$filter | if type == \"boolean\" then (if . then \"True\" else \"False\" end) elif . == null then \"none\" else tostring end" "$TOPOLOGY"
}

value_matches() {
  desired="$1"
  current="$2"

  if [ "$desired" = "none" ]; then
    case "$current" in
      ""|none|None)
        return 0
        ;;
    esac
  fi

  [ "$current" = "$desired" ]
}

global_pref_explicitly_set() {
  pref="$1"

  [ -r "$QUBES_XML" ] || return 1

  python3 - "$QUBES_XML" "$pref" <<'PY'
import sys
import xml.etree.ElementTree as ET

tree = ET.parse(sys.argv[1])
properties = tree.getroot().find("properties")
if properties is None:
    sys.exit(1)

for prop in properties.findall("property"):
    if prop.get("name") == sys.argv[2]:
        sys.exit(0)

sys.exit(1)
PY
}

set_pref() {
  domain="$1"
  pref="$2"
  value="$3"

  if [ "$APPLY" = "1" ]; then
    if current="$(qvm-prefs "$domain" "$pref" 2>/dev/null)"; then
      if value_matches "$value" "$current"; then
        printf 'pref current: %s %s=%s\n' "$domain" "$pref" "$value"
        return
      fi
    fi
  fi

  run_or_plan qvm-prefs "$domain" "$pref" "$value"
  prefs_changed=$((prefs_changed + 1))
  record_live_action domain-pref set "$domain" "$pref" "$value"
  record_domain_restart_if_running "$domain" domain-pref "$pref" "$value"
}

set_global_pref() {
  pref="$1"
  value="$2"

  if [ "$APPLY" = "1" ]; then
    if current="$(qubes-prefs "$pref" 2>/dev/null)"; then
      if value_matches "$value" "$current"; then
        if [ -z "$value" ] && ! global_pref_explicitly_set "$pref"; then
          run_or_plan qubes-prefs "$pref" "$value"
          prefs_changed=$((prefs_changed + 1))
          return
        fi
        printf 'global pref current: %s=%s\n' "$pref" "$value"
        return
      fi
    fi
  fi

  run_or_plan qubes-prefs "$pref" "$value"
  prefs_changed=$((prefs_changed + 1))
  record_live_action global-pref set "" "$pref" "$value"
}

volume_size_bytes() {
  local domain="$1"
  local volume="$2"

  qvm-volume info "$domain:$volume" 2>/dev/null |
    awk '$1 == "size" { print $2; found = 1 } END { exit(found ? 0 : 1) }'
}

ensure_state_volume_size() {
  local domain="$1"
  local volume="$2"
  local desired_size="$3"
  local current_size=""

  [ -n "$desired_size" ] && [ "$desired_size" != "null" ] || return 0
  case "$desired_size" in
    *[!0-9]*)
      fail "invalid persistent state volume size for $domain: $desired_size"
      ;;
  esac

  if [ "$APPLY" = "1" ]; then
    current_size="$(volume_size_bytes "$domain" "$volume" || true)"
    if [ -n "$current_size" ] && [ "$current_size" -ge "$desired_size" ]; then
      printf 'state volume current: %s %s=%s\n' "$domain" "$volume" "$current_size"
      return 0
    fi
    if domain_is_running "$domain"; then
      record_running_domain_storage_change "$domain" domain-state-volume "$volume" "$desired_size"
      return 0
    fi
  fi

  run_or_plan qvm-volume resize "$domain:$volume" "$desired_size"
  state_volumes_changed=$((state_volumes_changed + 1))
  record_live_action domain-state-volume resize "$domain" "$volume" "$desired_size"
}

set_feature() {
  domain="$1"
  feature="$2"
  value="$3"

  if [ "$APPLY" = "1" ]; then
    if current="$(qvm-features "$domain" "$feature" 2>/dev/null)"; then
      if value_matches "$value" "$current"; then
        printf 'feature current: %s %s=%s\n' "$domain" "$feature" "$value"
        return
      fi
    fi
  fi

  run_or_plan qvm-features "$domain" "$feature" "$value"
  features_changed=$((features_changed + 1))
  record_live_action domain-feature set "$domain" "$feature" "$value"
  record_domain_restart_if_running "$domain" domain-feature "$feature" "$value"
}

set_service() {
  domain="$1"
  service="$2"
  value="$3"

  if [ "$APPLY" = "1" ]; then
    if current="$(qvm-service "$domain" "$service" 2>/dev/null)"; then
      if value_matches "$value" "$current"; then
        printf 'service current: %s %s=%s\n' "$domain" "$service" "$value"
        return
      fi
    fi
  fi

  run_or_plan qvm-service "$domain" "$service" "$value"
  services_changed=$((services_changed + 1))
  record_live_action domain-service set "$domain" "$service" "$value"
  record_domain_restart_if_running "$domain" domain-service "$service" "$value"
}

is_preferred_domain_pref() {
  case "$1" in
    virt_mode|kernel|kernelopts|memory|maxmem|qrexec_timeout|default_user)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

domain_pref_names() {
  domain="$1"
  local pref
  local name
  local names=()

  mapfile -t names < <(jq_get '.domains[$name].prefs | keys[]?' --arg name "$domain")

  for pref in virt_mode kernel kernelopts memory maxmem qrexec_timeout default_user; do
    for name in "${names[@]}"; do
      if [ "$name" = "$pref" ]; then
        printf '%s\n' "$name"
      fi
    done
  done

  for name in "${names[@]}"; do
    if ! is_preferred_domain_pref "$name"; then
      printf '%s\n' "$name"
    fi
  done
}

is_qubes_name() {
  [[ "$1" =~ ^[A-Za-z][A-Za-z0-9_.-]*$ ]]
}

is_qrexec_service() {
  [[ "$1" =~ ^nixome\.[A-Za-z0-9_.-]+$ ]]
}

is_qrexec_argument() {
  [ "$1" = "*" ] || [[ "$1" =~ ^[A-Za-z0-9_.:+-]+$ ]]
}

is_qrexec_endpoint() {
  [ "$1" = "dom0" ] || [ "$1" = "@anyvm" ] || is_qubes_name "$1"
}

validate_qrexec_rule() {
  index="$1"
  service="$2"
  argument="$3"
  source="$4"
  target="$5"
  action="$6"

  is_qrexec_service "$service" ||
    fail "qrexec policy rule $index has invalid service: $service"
  case "$service" in
    nixome.Config.CandidateExport|nixome.Config.ValidateCandidate|nixome.Config.SeedInputsImport|nixome.Config.ArchiveCandidate|nixome.Config.ArchiveDeployment|nixome.Config.Status|nixome.Config.Check|nixome.Config.Apply|nixome.Config.Rollback)
      fail "qrexec policy rule $index uses reserved bootstrap service: $service"
      ;;
  esac
  is_qrexec_argument "$argument" ||
    fail "qrexec policy rule $index has invalid argument: $argument"
  is_qrexec_endpoint "$source" ||
    fail "qrexec policy rule $index has invalid source: $source"
  is_qrexec_endpoint "$target" ||
    fail "qrexec policy rule $index has invalid target: $target"
  [ "$target" != "dom0" ] ||
    fail "qrexec policy rule $index targets dom0; topology-managed policy may not target dom0"

  case "$action" in
    allow)
      [ "$source" != "@anyvm" ] ||
        fail "qrexec policy rule $index allow uses @anyvm source"
      [ "$target" != "@anyvm" ] ||
        fail "qrexec policy rule $index allow uses @anyvm target"
      ;;
    deny)
      ;;
    *)
      fail "qrexec policy rule $index has invalid action: $action"
      ;;
  esac
}

policy_file_is_managed() {
  file="$1"

  grep -Fx "# Managed by Nixome setup-config." "$file" >/dev/null 2>&1
}

render_qrexec_policy() {
  output="$1"

  {
    printf '# Managed by Nixome setup-config.\n'
    printf '# Source: nixome.qubes.topology.qrexec_policy.\n'
    printf '# Bootstrap config transport policy remains in 30-nixome-config.policy.\n'

    rule_count="$(jq_get '(.qrexec_policy.rules // []) | length')"
    index=0
    while [ "$index" -lt "$rule_count" ]; do
      service="$(jq_get '.qrexec_policy.rules[$index].service' --argjson index "$index")"
      argument="$(jq_get '.qrexec_policy.rules[$index].argument' --argjson index "$index")"
      source="$(jq_get '.qrexec_policy.rules[$index].source' --argjson index "$index")"
      target="$(jq_get '.qrexec_policy.rules[$index].target' --argjson index "$index")"
      action="$(jq_get '.qrexec_policy.rules[$index].action' --argjson index "$index")"

      validate_qrexec_rule "$index" "$service" "$argument" "$source" "$target" "$action"
      printf '%s %s %s %s %s\n' "$service" "$argument" "$source" "$target" "$action"
      index=$((index + 1))
    done
  } > "$output"
}

reload_qrexec_policy_daemon() {
  printf '+ systemctl reload qubes-qrexec-policy-daemon.service\n'
  if systemctl reload qubes-qrexec-policy-daemon.service; then
    return
  fi

  printf '+ systemctl try-restart qubes-qrexec-policy-daemon.service\n'
  systemctl try-restart qubes-qrexec-policy-daemon.service
}

setup_qrexec_policy() {
  printf 'setting up Nixome-owned qrexec policy\n'
  render_qrexec_policy "$RENDERED_POLICY"

  if [ -e "$MANAGED_POLICY_PATH" ] && ! policy_file_is_managed "$MANAGED_POLICY_PATH"; then
    fail "qrexec policy file exists but is not Nixome-managed: $MANAGED_POLICY_PATH"
  fi

  if [ -e "$MANAGED_POLICY_PATH" ] && cmp -s "$RENDERED_POLICY" "$MANAGED_POLICY_PATH"; then
    printf 'qrexec policy current: %s\n' "$MANAGED_POLICY_PATH"
    return
  fi

  printf '+ install-qrexec-policy %q %q\n' "$RENDERED_POLICY" "$MANAGED_POLICY_PATH"
  if [ "$APPLY" = "1" ]; then
    mkdir -p "$POLICY_DIR"
    cp "$RENDERED_POLICY" "$MANAGED_POLICY_PATH"
    chmod 0644 "$MANAGED_POLICY_PATH"
    reload_qrexec_policy_daemon
  fi
  qrexec_policy_changed=$((qrexec_policy_changed + 1))
  record_live_action qrexec-policy reload "" "$MANAGED_POLICY_FILE" "$MANAGED_POLICY_PATH"
}

preflight_domain_backends() {
  local row
  local domain
  local backend
  local source_ref_components=()
  local unresolved=()

  mapfile -t source_ref_components < <(
    jq -r '
      (.domains // {})
      | to_entries[]
      | .key as $domain
      | (.value.backend // {}) as $backend
      | select((.value.backend.type // "qubes-template") == "nix-closure")
      | [
          { domain: $domain, component: "system", ref: ($backend.system.source_ref // null) },
          { domain: $domain, component: "kernel", ref: ($backend.kernel.source_ref // null) },
          { domain: $domain, component: "initrd", ref: ($backend.initrd.source_ref // null) },
          { domain: $domain, component: "modules", ref: ($backend.modules.source_ref // null) },
          { domain: $domain, component: "root", ref: ($backend.root.source_ref // null) },
          { domain: $domain, component: "store", ref: ($backend.store.source_ref // null) }
        ]
      | .[]
      | select(.ref != null)
      | [.domain, .component]
      | @tsv
    ' "$TOPOLOGY"
  )

  for row in "${source_ref_components[@]}"; do
    [ -n "$row" ] || continue
    IFS=$'\t' read -r domain backend <<< "$row"
    unresolved+=("$domain.$backend")
  done

  if [ "${#unresolved[@]}" -gt 0 ]; then
    local IFS=,
    fail "Nix-native domain source reference was not resolved before setup-config: ${unresolved[*]}"
  fi
}

kernel_source_matches_target() {
  source="$1"
  target="$2"

  [ -d "$target" ] || return 1
  [ -f "$target/vmlinuz" ] || return 1

  while IFS= read -r source_file; do
    relative_file="${source_file#"$source"/}"
    target_file="$target/$relative_file"
    [ -f "$target_file" ] || return 1
    cmp -s "$source_file" "$target_file" || return 1
  done < <(find "$source" -maxdepth 1 -type f | sort)
}

kernel_is_current() {
  name="$1"
  source="$2"
  target="$KERNEL_BASE/$name"
  marker="$target/MANAGED_BY_NIXOME"

  [ -f "$marker" ] || return 1
  grep -Fx "source=$source" "$marker" >/dev/null 2>&1 || return 1
  kernel_source_matches_target "$source" "$target"
}

mark_kernel_managed() {
  name="$1"
  source="$2"
  target="$KERNEL_BASE/$name"

  {
    printf 'source=%s\n' "$source"
    printf 'schema=nixome.qubes-kernel.v1\n'
  } > "$target/MANAGED_BY_NIXOME"
}

install_kernel() {
  name="$1"
  source="$2"
  target="$KERNEL_BASE/$name"
  tmp="$KERNEL_BASE/.nixome-$name.tmp"

  rm -rf "$tmp"
  mkdir -p "$tmp"
  cp -a "$source/." "$tmp/"
  {
    printf 'source=%s\n' "$source"
    printf 'schema=nixome.qubes-kernel.v1\n'
  } > "$tmp/MANAGED_BY_NIXOME"
  rm -rf "$target"
  mv "$tmp" "$target"
}

resolve_root_image_file() {
  source="$1"
  source_physical="$(nixome_physical_store_path "$source")"

  if [ -d "$source_physical" ]; then
    [ -f "$source_physical/root.img" ] ||
      fail "Nix-native root image directory is missing root.img: $source/root.img"
    printf '%s/root.img\n' "$source_physical"
  else
    [ -f "$source_physical" ] ||
      fail "Nix-native root image source is not a file: $source"
    printf '%s\n' "$source_physical"
  fi
}

resolve_store_artifact() {
  source="$1"
  source_physical="$(nixome_physical_store_path "$source")"

  [ -d "$source_physical" ] ||
    fail "Nix-native shared closure source is not a directory: $source"
  [ -f "$source_physical/store-paths" ] ||
    fail "Nix-native shared closure source is missing store-paths: $source/store-paths"
  [ -f "$source_physical/manifest.json" ] ||
    fail "Nix-native shared closure source is missing manifest.json: $source/manifest.json"
  printf '%s\n' "$source_physical"
}

nix_domain_root_current() {
  domain="$1"
  root_source="$2"

  [ "$APPLY" = "1" ] || return 1
  current="$(qvm-features "$domain" nixome-root-source 2>/dev/null || true)"
  [ "$current" = "$root_source" ]
}

nix_domain_store_current() {
  domain="$1"
  store_source="$2"
  store_mode="$3"

  [ "$APPLY" = "1" ] || return 1
  current="$(qvm-features "$domain" nixome-store-source 2>/dev/null || true)"
  current_mode="$(qvm-features "$domain" nixome-store-mode 2>/dev/null || true)"
  [ -n "$current_mode" ] || current_mode="shared-closure"
  [ "$current" = "$store_source" ] && [ "$current_mode" = "$store_mode" ]
}

nix_domain_system_current() {
  domain="$1"
  system_source="$2"

  [ "$APPLY" = "1" ] || return 1
  current="$(qvm-features "$domain" nixome-system-source 2>/dev/null || true)"
  [ "$current" = "$system_source" ]
}

is_nix_closure_boot_feature() {
  case "$1" in
    nixome-system-source|\
    nixome-root-source|\
    nixome-store-source|\
    nixome-store-mode|\
    nixome-store-generation|\
    nixome-store-closure-hash|\
    nixome-store-view-image|\
    nixome-store-view-backend|\
    nixome-store-blob-device|\
    nixome-store-blob-volume|\
    nixome-store-blob-map|\
    nixome-store-filesystem|\
    nixome-store-mountpoint|\
    nixome-store-device|\
    nixome-store-read-only)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

install_nix_domain_root() {
  domain="$1"
  root_source="$2"
  root_image="$3"
  nix_domain_root_deferred=0

  if nix_domain_root_current "$domain" "$root_source"; then
    printf 'Nix-native root current: %s source=%s\n' "$domain" "$root_source"
    return
  fi

  if domain_is_running "$domain"; then
    printf 'Nix-native root image update deferred for running domain: %s source=%s\n' \
      "$domain" "$root_source"
    nix_domain_root_deferred=1
    record_running_domain_storage_change "$domain" domain-root source "$root_source"
    return
  fi

  run_or_plan qvm-volume import "$domain:root" "$root_image"
  root_images_changed=$((root_images_changed + 1))
  record_live_action domain-root import "$domain" source "$root_source"
}

append_kernelopt_if_missing() {
  opts="$1"
  key="$2"
  value="$3"

  if printf '%s\n' "$opts" | tr ' ' '\n' | grep -q "^$key="; then
    current="$(
      printf '%s\n' "$opts" |
        tr ' ' '\n' |
        sed -n "s|^$key=||p" |
        head -n 1
    )"
    [ "$current" = "$value" ] ||
      fail "nix-closure kernelopts $key=$current does not match expected $value"
    printf '%s\n' "$opts"
    return
  fi

  if [ -n "$opts" ]; then
    printf '%s %s=%s\n' "$opts" "$key" "$value"
  else
    printf '%s=%s\n' "$key" "$value"
  fi
}

ensure_nix_closure_kernelopts() {
  domain="$1"
  system_source="$2"
  store_device="$3"
  store_filesystem="$4"
  store_blob_device="$5"
  opts="$(json_value '.domains[$name].prefs.kernelopts // ""' --arg name "$domain")"

  if [ -z "$opts" ] || [ "$opts" = "null" ]; then
    opts="console=hvc0 init=/sbin/init"
  fi

  if printf '%s\n' "$opts" | tr ' ' '\n' | grep -q '^init='; then
    current_init="$(
      printf '%s\n' "$opts" |
        tr ' ' '\n' |
        sed -n 's|^init=||p' |
        head -n 1
    )"
    [ "$current_init" = "/sbin/init" ] ||
      fail "nix-closure kernelopts init=$current_init does not match expected /sbin/init"
  else
    opts="$opts init=/sbin/init"
  fi

  opts="$(append_kernelopt_if_missing "$opts" nixome.system "$system_source")"
  opts="$(append_kernelopt_if_missing "$opts" nixome.storedev "$store_device")"
  opts="$(append_kernelopt_if_missing "$opts" nixome.storefmt "$store_filesystem")"
  if [ "$store_filesystem" = "erofs" ]; then
    opts="$(append_kernelopt_if_missing "$opts" nixome.storeblobdev "$store_blob_device")"
  fi

  set_pref "$domain" kernelopts "$opts"
}

install_nix_domain_store() {
  domain="$1"
  store_source="$2"
  store_artifact="$3"
  store_mode="$4"
  system_source="$5"
  root_source="$6"
  kernel_name="$7"
  store_device="$8"
  store_mountpoint="$9"
  store_blob_device="${10}"
  store_view_backend="${11}"
  helper_output=""
  nix_domain_store_deferred=0
  nix_domain_store_generation=""
  nix_domain_store_closure_hash=""
  nix_domain_store_view_image=""
  nix_domain_store_blob_map=""
  nix_domain_store_blob_volume=""

  if [ "$APPLY" = "1" ] && nix_domain_store_current "$domain" "$store_source" "$store_mode"; then
    printf 'Nix-native store source current: %s source=%s\n' "$domain" "$store_source"
  elif domain_is_running "$domain"; then
    printf 'Nix-native store update deferred for running domain: %s source=%s\n' \
      "$domain" "$store_source"
    nix_domain_store_deferred=1
    record_running_domain_storage_change "$domain" domain-store source "$store_source"
    return
  fi

  printf '+'
  printf ' %q' "$NIX_DOMAIN_STORAGE_HELPER" ensure-store "$domain" "$store_artifact" "$store_source" \
    --mode "$store_mode" \
    --system "$system_source" \
    --root "$root_source" \
    --kernel "$kernel_name" \
    --device "$store_device" \
    --blob-device "$store_blob_device" \
    --mountpoint "$store_mountpoint" \
    --view-backend "$store_view_backend"
  printf '\n'

  if [ "$APPLY" = "1" ]; then
    set +e
    helper_output="$("$NIX_DOMAIN_STORAGE_HELPER" ensure-store "$domain" "$store_artifact" "$store_source" \
      --mode "$store_mode" \
      --system "$system_source" \
      --root "$root_source" \
      --kernel "$kernel_name" \
      --device "$store_device" \
      --blob-device "$store_blob_device" \
      --mountpoint "$store_mountpoint" \
      --view-backend "$store_view_backend")"
    rc="$?"
    set -e
    printf '%s\n' "$helper_output"
    if [ "$rc" != "0" ]; then
      printf 'command failed with exit %s: %q ensure-store %q %q %q --mode %q\n' \
        "$rc" "$NIX_DOMAIN_STORAGE_HELPER" "$domain" "$store_artifact" "$store_source" "$store_mode"
      return "$rc"
    fi

    while IFS= read -r created_volume; do
      [ -n "$created_volume" ] || continue
      store_volumes_changed=$((store_volumes_changed + 1))
      record_live_action domain-store-volume create "$domain" volume "$created_volume"
    done < <(printf '%s\n' "$helper_output" | jq -r '.volumes_created[]?')
    if ! printf '%s\n' "$helper_output" | jq -e 'has("volumes_created")' >/dev/null &&
      printf '%s\n' "$helper_output" | jq -e '.volume_created == true' >/dev/null; then
      store_volumes_changed=$((store_volumes_changed + 1))
      record_live_action domain-store-volume create "$domain" volume store
    fi

    if printf '%s\n' "$helper_output" | jq -e '.view_created == true' >/dev/null; then
      store_views_changed=$((store_views_changed + 1))
      record_live_action domain-store-view materialize "$domain" source "$store_source"
    fi

    nix_domain_store_generation="$(printf '%s\n' "$helper_output" | jq -r '.generation // ""')"
    nix_domain_store_closure_hash="$(printf '%s\n' "$helper_output" | jq -r '.closure_hash // ""')"
    nix_domain_store_view_image="$(printf '%s\n' "$helper_output" | jq -r '.view_image // .image // ""')"
    nix_domain_store_blob_map="$(printf '%s\n' "$helper_output" | jq -r '.blob_map // ""')"
    nix_domain_store_blob_volume="$(printf '%s\n' "$helper_output" | jq -r '.blob_volume // ""')"

    if printf '%s\n' "$helper_output" | jq -e '.qubesd_restarted == true' >/dev/null; then
      qubesd_restarts=$((qubesd_restarts + 1))
      record_live_action systemd restart "" service qubesd.service
    elif printf '%s\n' "$helper_output" | jq -e '.qubesd_restart_required == true' >/dev/null; then
      restart_qubesd_after_storage_change
    fi
  else
    store_volumes_changed=$((store_volumes_changed + 2))
    record_live_action domain-store-volume create "$domain" volume store
    record_live_action domain-store-volume create "$domain" volume store-data
    store_views_changed=$((store_views_changed + 1))
    record_live_action domain-store-view materialize "$domain" source "$store_source"
  fi
}

install_domain_kernel_source() {
  domain="$1"
  name="$2"
  source="$3"

  [ -n "$source" ] || return 0
  if [ -z "$name" ] || [ "$name" = "null" ]; then
    name="nixome-$domain"
  fi

  case "$name" in
    ""|*/*)
      fail "invalid Nix-native kernel name for $domain: $name"
      ;;
  esac

  source_physical="$(nixome_physical_store_path "$source")"
  [ -d "$source_physical" ] ||
    fail "Nix-native kernel source directory is missing for $domain: $source"
  [ -f "$source_physical/vmlinuz" ] ||
    fail "Nix-native kernel source vmlinuz is missing for $domain: $source/vmlinuz"

  if kernel_is_current "$name" "$source_physical"; then
    printf 'kernel current: %s\n' "$name"
    return 0
  fi

  target="$KERNEL_BASE/$name"
  if [ -d "$target" ] && [ ! -f "$target/MANAGED_BY_NIXOME" ]; then
    if kernel_source_matches_target "$source_physical" "$target"; then
      printf '+ adopt-kernel %q %q\n' "$name" "$source"
      if [ "$APPLY" = "1" ]; then
        mark_kernel_managed "$name" "$source_physical"
      fi
      kernels_changed=$((kernels_changed + 1))
      record_live_action kernel adopt "$domain" "$name" "$source"
      return 0
    fi

    fail "kernel exists but is not managed by Nixome: $target"
  fi

  printf '+ install-kernel %q %q\n' "$name" "$source"
  if [ "$APPLY" = "1" ]; then
    install_kernel "$name" "$source_physical"
  fi
  kernels_changed=$((kernels_changed + 1))
  record_live_action kernel install "$domain" "$name" "$source"
}

setup_nix_closure_domain() {
  domain="$1"
  class="$(jq_get '.domains[$name].class // "StandaloneVM"' --arg name "$domain")"
  label="$(jq_get '.domains[$name].label // "green"' --arg name "$domain")"
  netvm="$(json_value '.domains[$name].netvm' --arg name "$domain")"
  system_source="$(jq_get '.domains[$name].backend.system.source // ""' --arg name "$domain")"
  kernel_name="$(jq_get '.domains[$name].backend.kernel.name // ""' --arg name "$domain")"
  kernel_source="$(jq_get '.domains[$name].backend.kernel.source // ""' --arg name "$domain")"
  root_source="$(jq_get '.domains[$name].backend.root.source // ""' --arg name "$domain")"
  store_source="$(jq_get '.domains[$name].backend.store.source // ""' --arg name "$domain")"
  store_mode="$(jq_get '.domains[$name].backend.store.mode // "shared-closure"' --arg name "$domain")"
  store_mountpoint="$(jq_get '.domains[$name].backend.store.mountpoint // "/nix/store"' --arg name "$domain")"
  store_device="$(jq_get '.domains[$name].backend.store.device // "/dev/xvde"' --arg name "$domain")"
  store_blob_device="$(jq_get '.domains[$name].backend.store.blob_device // .domains[$name].backend.store.blobDevice // "/dev/xvdf"' --arg name "$domain")"
  store_view_backend="$(jq_get '.domains[$name].backend.store.view_backend // .domains[$name].backend.store.viewBackend // "erofs-objectmap"' --arg name "$domain")"
  persistent_size="$(jq_get '.domains[$name].backend.state.persistent_size // .domains[$name].backend.state.persistentSize // null' --arg name "$domain")"
  store_filesystem="erofs"
  root_image=""
  store_artifact=""
  nix_domain_root_deferred=0
  nix_domain_store_deferred=0
  nix_domain_storage_deferred=0

  [ "$class" = "StandaloneVM" ] ||
    fail "nix-closure backend requires StandaloneVM class for $domain"
  [ -n "$system_source" ] ||
    fail "Nix-native domain system source is missing: $domain"
  [ -n "$root_source" ] ||
    fail "Nix-native domain root image source is missing: $domain"
  [ -n "$store_source" ] ||
    fail "Nix-native domain store source is missing: $domain"
  [ "$store_mode" = "shared-closure" ] ||
    fail "unsupported Nix-native domain store mode for $domain: $store_mode"
  [ "$store_view_backend" = "erofs-objectmap" ] ||
    fail "unsupported Nix-native domain store view backend for $domain: $store_view_backend"
  [ "$store_mountpoint" = "/nix/store" ] ||
    fail "unsupported Nix-native domain store mountpoint for $domain: $store_mountpoint"
  [ "$store_device" = "/dev/xvde" ] ||
    fail "unsupported Nix-native domain store device for $domain: $store_device"
  if [ "$store_filesystem" = "erofs" ] && [ "$store_blob_device" != "/dev/xvdf" ]; then
    fail "unsupported Nix-native domain store blob device for $domain: $store_blob_device"
  fi

  root_image="$(resolve_root_image_file "$root_source")"
  store_artifact="$(resolve_store_artifact "$store_source")"
  install_domain_kernel_source "$domain" "$kernel_name" "$kernel_source"

  if [ -z "$kernel_name" ] || [ "$kernel_name" = "null" ]; then
    kernel_name="nixome-$domain"
  fi

  if domain_exists "$domain"; then
    current_class="$(domain_class "$domain" || true)"
    [ "$current_class" = "StandaloneVM" ] ||
      fail "nix-closure domain exists with class $current_class, expected StandaloneVM: $domain"
    printf 'domain exists: %s\n' "$domain"
  else
    run_or_plan qvm-create --class StandaloneVM --label "$label" "$domain"
    domains_changed=$((domains_changed + 1))
    record_live_action domain create "$domain" class StandaloneVM
  fi

  install_nix_domain_root "$domain" "$root_source" "$root_image"
  ensure_state_volume_size "$domain" private "$persistent_size"
  install_nix_domain_store \
    "$domain" \
    "$store_source" \
    "$store_artifact" \
    "$store_mode" \
    "$system_source" \
    "$root_source" \
    "$kernel_name" \
    "$store_device" \
    "$store_mountpoint" \
    "$store_blob_device" \
    "$store_view_backend"
  if [ "$nix_domain_root_deferred" = "1" ] || [ "$nix_domain_store_deferred" = "1" ]; then
    nix_domain_storage_deferred=1
  fi

  if [ "$APPLY" = "1" ] || domain_exists "$domain"; then
    set_pref "$domain" label "$label"
    if [ "$nix_domain_storage_deferred" = "1" ]; then
      printf 'Nix-native boot identity update deferred for running domain: %s system=%s\n' \
        "$domain" "$system_source"
      if ! nix_domain_system_current "$domain" "$system_source"; then
        record_running_domain_storage_change "$domain" domain-system source "$system_source"
      fi
    else
      set_pref "$domain" kernel "$kernel_name"
      ensure_nix_closure_kernelopts "$domain" "$system_source" "$store_device" "$store_filesystem" "$store_blob_device"
    fi

    if jq -e --arg name "$domain" '.domains[$name] | has("netvm")' "$TOPOLOGY" >/dev/null; then
      set_pref "$domain" netvm "$netvm"
    fi

    set_feature "$domain" nixome-backend nix-closure
    if [ "$nix_domain_storage_deferred" != "1" ]; then
      set_feature "$domain" nixome-system-source "$system_source"
      set_feature "$domain" nixome-root-source "$root_source"
      set_feature "$domain" nixome-store-source "$store_source"
      set_feature "$domain" nixome-store-mode "$store_mode"
      set_feature "$domain" nixome-store-view-backend "$store_view_backend"
      set_feature "$domain" nixome-store-filesystem "$store_filesystem"
      if [ -n "$nix_domain_store_generation" ]; then
        set_feature "$domain" nixome-store-generation "$nix_domain_store_generation"
      fi
      if [ -n "$nix_domain_store_closure_hash" ]; then
        set_feature "$domain" nixome-store-closure-hash "$nix_domain_store_closure_hash"
      fi
      if [ -n "$nix_domain_store_view_image" ]; then
        set_feature "$domain" nixome-store-view-image "$nix_domain_store_view_image"
      fi
      if [ -n "$nix_domain_store_blob_map" ]; then
        set_feature "$domain" nixome-store-blob-map "$nix_domain_store_blob_map"
      fi
      if [ -n "$nix_domain_store_blob_volume" ]; then
        set_feature "$domain" nixome-store-blob-volume "$nix_domain_store_blob_volume"
      fi
      set_feature "$domain" nixome-store-mountpoint "$store_mountpoint"
      set_feature "$domain" nixome-store-device "$store_device"
      if [ "$store_filesystem" = "erofs" ]; then
        set_feature "$domain" nixome-store-blob-device "$store_blob_device"
      fi
      set_feature "$domain" nixome-store-read-only "True"
    fi

    mapfile -t pref_names < <(domain_pref_names "$domain")
    for pref in "${pref_names[@]}"; do
      [ "$pref" != "kernelopts" ] || continue
      [ "$pref" != "kernel" ] || continue
      value="$(json_value '.domains[$name].prefs[$pref]' --arg name "$domain" --arg pref "$pref")"
      set_pref "$domain" "$pref" "$value"
    done

    mapfile -t feature_names < <(jq_get '.domains[$name].features | keys[]?' --arg name "$domain")
    for feature in "${feature_names[@]}"; do
      if [ "$nix_domain_storage_deferred" = "1" ] && is_nix_closure_boot_feature "$feature"; then
        printf 'Nix-native boot feature update deferred for running domain: %s %s\n' \
          "$domain" "$feature"
        continue
      fi
      value="$(json_value '.domains[$name].features[$feature]' --arg name "$domain" --arg feature "$feature")"
      set_feature "$domain" "$feature" "$value"
    done

    mapfile -t service_names < <(jq_get '.domains[$name].services | keys[]?' --arg name "$domain")
    for service in "${service_names[@]}"; do
      value="$(jq_get '.domains[$name].services[$service] | if . then "on" else "off" end' --arg name "$domain" --arg service "$service")"
      set_service "$domain" "$service" "$value"
    done
  fi
}

[ -r "$TOPOLOGY" ] || fail "topology file is not readable: $TOPOLOGY"

topology_has_nix_closure_domains() {
  jq -e '
    (.domains // {})
    | to_entries
    | any((.value.backend.type // "qubes-template") == "nix-closure")
  ' "$TOPOLOGY" >/dev/null
}

for cmd in \
  awk \
  chmod \
  cmp \
  cp \
  grep \
  jq \
  mkdir \
  qubes-prefs \
  qvm-create \
  qvm-features \
  qvm-ls \
  qvm-prefs \
  qvm-service \
  qvm-template-postprocess \
  qvm-volume \
  qubesd-query \
  python3 \
  rm \
  systemctl; do
  require_command "$cmd"
done

if topology_has_nix_closure_domains; then
  require_command "$NIX_DOMAIN_STORAGE_HELPER"
fi

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

schema="$(jq_get '.schema')"
[ "$schema" = "nixome.qubes-topology.v1" ] || fail "unsupported topology schema: $schema"

preflight_domain_backends
setup_qrexec_policy

printf 'setting up global prefs\n'
mapfile -t global_pref_names < <(jq_get '.prefs | keys[]?')
for pref in "${global_pref_names[@]}"; do
  value="$(json_value '.prefs[$pref]' --arg pref "$pref")"
  set_global_pref "$pref" "$value"
done

printf 'setting up kernels\n'
mapfile -t kernel_names < <(jq_get '.kernels | keys[]?')
for kernel in "${kernel_names[@]}"; do
  case "$kernel" in
    ""|*/*)
      fail "invalid kernel name: $kernel"
      ;;
  esac

  source="$(jq_get '.kernels[$name].source // ""' --arg name "$kernel")"
  target="$KERNEL_BASE/$kernel"

  [ -n "$source" ] || fail "kernel source is empty: $kernel"
  [ -d "$source" ] || fail "kernel source directory is missing: $source"
  [ -f "$source/vmlinuz" ] || fail "kernel source vmlinuz is missing: $source/vmlinuz"

  if kernel_is_current "$kernel" "$source"; then
    printf 'kernel current: %s\n' "$kernel"
    continue
  fi

  if [ -d "$target" ] && [ ! -f "$target/MANAGED_BY_NIXOME" ]; then
    if kernel_source_matches_target "$source" "$target"; then
      printf '+ adopt-kernel %q %q\n' "$kernel" "$source"
      if [ "$APPLY" = "1" ]; then
        mark_kernel_managed "$kernel" "$source"
      fi
      kernels_changed=$((kernels_changed + 1))
      record_live_action kernel adopt "" "$kernel" "$source"
      continue
    fi

    fail "kernel exists but is not managed by Nixome: $target"
  fi

  printf '+ install-kernel %q %q\n' "$kernel" "$source"
  if [ "$APPLY" = "1" ]; then
    install_kernel "$kernel" "$source"
  fi
  kernels_changed=$((kernels_changed + 1))
  record_live_action kernel install "" "$kernel" "$source"
done

printf 'setting up templates\n'
mapfile -t template_names < <(jq_get '.templates | keys[]')
for template in "${template_names[@]}"; do
  source="$(jq_get '.templates[$name].source // ""' --arg name "$template")"
  source_ref_type="$(jq_get '.templates[$name].source_ref.type // ""' --arg name "$template")"
  import_template="$(jq_get '.templates[$name].import // false' --arg name "$template")"
  skip_start="$(jq_get '.templates[$name].skip_start // true' --arg name "$template")"

  [ -z "$source_ref_type" ] ||
    fail "template source reference was not resolved before setup-config: $template"

  if template_exists "$template"; then
    printf 'template exists: %s\n' "$template"
    continue
  fi

  [ "$import_template" = "true" ] || fail "template is missing and import is disabled: $template"
  [ -n "$source" ] || fail "template is missing and source is empty: $template"
  [ -d "$source" ] || fail "template source directory is missing: $source"
  [ -f "$source/root.img" ] || fail "template source root.img is missing: $source/root.img"

  args=(qvm-template-postprocess --really --keep-source --no-installed-by-rpm)
  if [ "$skip_start" = "true" ]; then
    args+=(--skip-start)
  fi
  args+=(post-install "$template" "$source")

  run_or_plan "${args[@]}"
  templates_changed=$((templates_changed + 1))
  record_live_action template import "$template" source "$source"
done

printf 'setting up domains\n'
mapfile -t domain_names < <(jq_get '.domains | keys[]')
for domain in "${domain_names[@]}"; do
  backend_type="$(jq_get '.domains[$name].backend.type // "qubes-template"' --arg name "$domain")"
  if [ "$backend_type" = "nix-closure" ]; then
    setup_nix_closure_domain "$domain"
    continue
  fi

  class="$(jq_get '.domains[$name].class // "AppVM"' --arg name "$domain")"
  template="$(jq_get '.domains[$name].template // ""' --arg name "$domain")"
  label="$(jq_get '.domains[$name].label // "red"' --arg name "$domain")"
  netvm="$(json_value '.domains[$name].netvm' --arg name "$domain")"

  if domain_exists "$domain"; then
    printf 'domain exists: %s\n' "$domain"
  else
    create_args=(qvm-create --class "$class" --label "$label")
    if [ -n "$template" ] && [ "$template" != "null" ]; then
      create_args+=(--template "$template")
    fi
    create_args+=("$domain")
    run_or_plan "${create_args[@]}"
    domains_changed=$((domains_changed + 1))
    record_live_action domain create "$domain" class "$class"
  fi

  if [ "$APPLY" = "1" ] || domain_exists "$domain"; then
    set_pref "$domain" label "$label"

    if jq -e --arg name "$domain" '.domains[$name] | has("netvm")' "$TOPOLOGY" >/dev/null; then
      set_pref "$domain" netvm "$netvm"
    fi

    mapfile -t pref_names < <(domain_pref_names "$domain")
    for pref in "${pref_names[@]}"; do
      value="$(json_value '.domains[$name].prefs[$pref]' --arg name "$domain" --arg pref "$pref")"
      set_pref "$domain" "$pref" "$value"
    done

    mapfile -t feature_names < <(jq_get '.domains[$name].features | keys[]?' --arg name "$domain")
    for feature in "${feature_names[@]}"; do
      value="$(json_value '.domains[$name].features[$feature]' --arg name "$domain" --arg feature "$feature")"
      set_feature "$domain" "$feature" "$value"
    done

    mapfile -t service_names < <(jq_get '.domains[$name].services | keys[]?' --arg name "$domain")
    for service in "${service_names[@]}"; do
      value="$(jq_get '.domains[$name].services[$service] | if . then "on" else "off" end' --arg name "$domain" --arg service "$service")"
      set_service "$domain" "$service" "$value"
    done
  fi
done

write_result passed "Qubes config setup completed"
persist_setup_state
cat "$RESULT_JSON"
