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
XEN_CAPABILITIES="${NIXOME_XEN_CAPABILITIES:-/proc/xen/capabilities}"
KERNEL_BASE="${NIXOME_QUBES_KERNEL_BASE:-/var/lib/qubes/vm-kernels}"
QUBES_XML="${NIXOME_QUBES_XML:-/var/lib/qubes/qubes.xml}"
POLICY_DIR="${NIXOME_QUBES_POLICY_DIR:-/etc/qubes/policy.d}"
MANAGED_POLICY_FILE="${NIXOME_QUBES_MANAGED_POLICY_FILE:-90-nixome.policy}"
MANAGED_POLICY_PATH="$POLICY_DIR/$MANAGED_POLICY_FILE"
RENDERED_POLICY="$LOG_DIR/$MANAGED_POLICY_FILE.rendered"
result_written=0

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

exec > >(tee -a "$COMMAND_LOG") 2>&1
trap write_unexpected_failure EXIT

templates_changed=0
kernels_changed=0
domains_changed=0
prefs_changed=0
features_changed=0
services_changed=0
qrexec_policy_changed=0

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
    "$@"
  fi
}

domain_exists() {
  qvm-ls --raw-data --fields NAME --no-spinner 2>/dev/null |
    grep -Fx "$1" >/dev/null 2>&1
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
    nixome.Config.CandidateExport|nixome.Config.ValidateCandidate|nixome.Config.ArchiveCandidate|nixome.Config.ArchiveDeployment)
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

[ -r "$TOPOLOGY" ] || fail "topology file is not readable: $TOPOLOGY"

for cmd in \
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
  qubesd-query \
  python3 \
  rm \
  systemctl; do
  require_command "$cmd"
done

if ! [ -r "$XEN_CAPABILITIES" ] || ! grep -qw control_d "$XEN_CAPABILITIES"; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service || fail "qubes-qrexec-policy-daemon.service is not active"

schema="$(jq_get '.schema')"
[ "$schema" = "nixome.qubes-topology.v1" ] || fail "unsupported topology schema: $schema"

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
      continue
    fi

    fail "kernel exists but is not managed by Nixome: $target"
  fi

  printf '+ install-kernel %q %q\n' "$kernel" "$source"
  if [ "$APPLY" = "1" ]; then
    install_kernel "$kernel" "$source"
  fi
  kernels_changed=$((kernels_changed + 1))
done

printf 'setting up templates\n'
mapfile -t template_names < <(jq_get '.templates | keys[]')
for template in "${template_names[@]}"; do
  source="$(jq_get '.templates[$name].source // ""' --arg name "$template")"
  import_template="$(jq_get '.templates[$name].import // false' --arg name "$template")"
  skip_start="$(jq_get '.templates[$name].skip_start // true' --arg name "$template")"

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
done

printf 'setting up domains\n'
mapfile -t domain_names < <(jq_get '.domains | keys[]')
for domain in "${domain_names[@]}"; do
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
  fi

  if [ "$APPLY" = "1" ] || domain_exists "$domain"; then
    set_pref "$domain" label "$label"

    if jq -e --arg name "$domain" '.domains[$name] | has("netvm")' "$TOPOLOGY" >/dev/null; then
      set_pref "$domain" netvm "$netvm"
    fi

    mapfile -t pref_names < <(jq_get '.domains[$name].prefs | keys[]?' --arg name "$domain")
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
cat "$RESULT_JSON"
