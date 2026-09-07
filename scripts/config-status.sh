#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
SYSTEM_STATE_DIR="${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
BOOT_TOPOLOGY_JSON="${NIXOME_CONFIG_STATUS_BOOT_TOPOLOGY_JSON:-/etc/nixome/qubes-topology.json}"
CURRENT_TOPOLOGY_JSON="${NIXOME_CONFIG_STATUS_CURRENT_TOPOLOGY_JSON:-$SYSTEM_STATE_DIR/current-topology.json}"
TOPOLOGY_OVERRIDE="${NIXOME_CONFIG_STATUS_TOPOLOGY_JSON:-}"
SETUP_JSON="${NIXOME_CONFIG_STATUS_SETUP_JSON:-$LOG_DIR/setup-config.json}"
APPLY_JSON="${NIXOME_CONFIG_STATUS_APPLY_JSON:-$LOG_DIR/config-apply.json}"
QREXEC_POLICY_DIR="${NIXOME_QUBES_POLICY_DIR:-/var/lib/nixome/qubes-policy.d}"
QREXEC_MANAGED_POLICY="${NIXOME_QUBES_MANAGED_POLICY_FILE:-90-nixome.policy}"
QREXEC_POLICY_FILE="$QREXEC_POLICY_DIR/$QREXEC_MANAGED_POLICY"
QVM_LS_BIN="${NIXOME_QVM_LS_BIN:-qvm-ls}"

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

json_file_or_null() {
  local path="$1"

  if [ -s "$path" ] && jq -e . "$path" >/dev/null 2>&1; then
    cat "$path"
  else
    printf null
  fi
}

readlink_or_empty() {
  local path="$1"

  if [ -L "$path" ]; then
    readlink "$path" || true
  fi
}

policy_rule_count() {
  local path="$1"

  if [ ! -r "$path" ]; then
    printf 0
    return 0
  fi

  awk '
    /^[[:space:]]*$/ { next }
    /^[[:space:]]*#/ { next }
    { count += 1 }
    END { print count + 0 }
  ' "$path"
}

qvm_domains_json() {
  local raw="$tmpdir/qvm-ls.raw"

  if ! command -v "$QVM_LS_BIN" >/dev/null 2>&1; then
    printf 'missing' > "$tmpdir/qvm-ls.status"
    printf '[]'
    return 0
  fi

  if ! "$QVM_LS_BIN" --raw-data --fields NAME,CLASS,STATE,TEMPLATE,LABEL --no-spinner > "$raw" 2>/dev/null; then
    printf 'failed' > "$tmpdir/qvm-ls.status"
    printf '[]'
    return 0
  fi

  printf 'ok' > "$tmpdir/qvm-ls.status"
  jq -R -s '
    split("\n")
    | map(select(length > 0))
    | map(split("|"))
    | map({
        name: .[0],
        class: (.[1] // ""),
        state: (.[2] // ""),
        template: (.[3] // ""),
        label: (.[4] // "")
      })
  ' "$raw"
}

boot_topology_json="$(json_file_or_null "$BOOT_TOPOLOGY_JSON")"
current_topology_json="$(json_file_or_null "$CURRENT_TOPOLOGY_JSON")"
if [ -n "$TOPOLOGY_OVERRIDE" ]; then
  TOPOLOGY_JSON="$TOPOLOGY_OVERRIDE"
  topology_source="override"
  topology_json="$(json_file_or_null "$TOPOLOGY_JSON")"
elif [ "$current_topology_json" != null ]; then
  TOPOLOGY_JSON="$CURRENT_TOPOLOGY_JSON"
  topology_source="current"
  topology_json="$current_topology_json"
else
  TOPOLOGY_JSON="$BOOT_TOPOLOGY_JSON"
  topology_source="boot"
  topology_json="$boot_topology_json"
fi
setup_json="$(json_file_or_null "$SETUP_JSON")"
apply_json="$(json_file_or_null "$APPLY_JSON")"
current_system_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-system.json")"
current_plan_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-plan.json")"
last_apply_json="$(json_file_or_null "$SYSTEM_STATE_DIR/last-apply.json")"
qvm_domains="$(qvm_domains_json)"
qvm_status="$(cat "$tmpdir/qvm-ls.status" 2>/dev/null || printf unknown)"
qvm_available=false
if [ "$qvm_status" = ok ]; then
  qvm_available=true
fi
policy_exists=false
if [ -r "$QREXEC_POLICY_FILE" ]; then
  policy_exists=true
fi

jq -n \
  --arg topology_path "$TOPOLOGY_JSON" \
  --arg topology_source "$topology_source" \
  --arg boot_topology_path "$BOOT_TOPOLOGY_JSON" \
  --arg current_topology_path "$CURRENT_TOPOLOGY_JSON" \
  --arg setup_path "$SETUP_JSON" \
  --arg apply_path "$APPLY_JSON" \
  --arg system_state_dir "$SYSTEM_STATE_DIR" \
  --arg qrexec_policy_file "$QREXEC_POLICY_FILE" \
  --arg qvm_status "$qvm_status" \
  --arg system_profile "/nix/var/nix/profiles/nixome-system" \
  --arg system_profile_target "$(readlink_or_empty /nix/var/nix/profiles/nixome-system)" \
  --arg host_system_profile "/nix/var/nix/profiles/system" \
  --arg host_system_profile_target "$(readlink_or_empty /nix/var/nix/profiles/system)" \
  --arg run_current_system "/run/current-system" \
  --arg run_current_system_target "$(readlink_or_empty /run/current-system)" \
  --argjson topology "$topology_json" \
  --argjson boot_topology "$boot_topology_json" \
  --argjson current_topology "$current_topology_json" \
  --argjson setup "$setup_json" \
  --argjson apply "$apply_json" \
  --argjson current_system "$current_system_json" \
  --argjson current_plan "$current_plan_json" \
  --argjson last_apply "$last_apply_json" \
  --argjson qvm_domains "$qvm_domains" \
  --argjson qvm_available "$qvm_available" \
  --argjson qrexec_policy_exists "$policy_exists" \
  --argjson qrexec_policy_rule_count "$(policy_rule_count "$QREXEC_POLICY_FILE")" \
  '
    def keys_or_empty:
      if type == "object" then keys else [] end;

    def status:
      if ($apply.status? == "passed") then "applied"
      elif ($setup.status? == "passed") then "setup"
      elif $topology != null then "declared"
      else "unknown"
      end;

    {
      schema: "nixome.config-status.v1",
      status: status,
      paths: {
        topology: $topology_path,
        boot_topology: $boot_topology_path,
        current_topology: $current_topology_path,
        setup: $setup_path,
        apply: $apply_path,
        system_state_dir: $system_state_dir,
        qrexec_policy_file: $qrexec_policy_file
      },
      topology: {
        source: $topology_source,
        present: ($topology != null),
        boot_present: ($boot_topology != null),
        current_present: ($current_topology != null),
        domain_names: (($topology.domains // {}) | keys_or_empty),
        template_names: (($topology.templates // {}) | keys_or_empty),
        kernel_names: (($topology.kernels // {}) | keys_or_empty),
        qrexec_policy_rule_count: (($topology.qrexec_policy.rules // []) | length)
      },
      qvm: {
        available: $qvm_available,
        status: $qvm_status,
        domains: $qvm_domains,
        managed_domains: (
          $qvm_domains
          | map(select(.name as $name | (($topology.domains // {}) | has($name))))
        )
      },
      qrexec_policy: {
        file: $qrexec_policy_file,
        exists: $qrexec_policy_exists,
        rule_count: $qrexec_policy_rule_count
      },
      setup: $setup,
      apply: $apply,
      system: {
        state_dir: $system_state_dir,
        current_system: $current_system,
        current_plan: $current_plan,
        last_apply: $last_apply,
        profile: $system_profile,
        profile_target: $system_profile_target,
        host_system_profile: $host_system_profile,
        host_system_profile_target: $host_system_profile_target,
        run_current_system: $run_current_system,
        run_current_system_target: $run_current_system_target
      }
    }
  '
