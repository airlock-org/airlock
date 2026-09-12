#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${AIRLOCK_LOG_DIR:-/var/log/airlock}"
SYSTEM_STATE_DIR="${AIRLOCK_SYSTEM_STATE_DIR:-/var/lib/airlock/system}"
BOOT_TOPOLOGY_JSON="${AIRLOCK_CONFIG_STATUS_BOOT_TOPOLOGY_JSON:-/etc/airlock/qubes-topology.json}"
CURRENT_TOPOLOGY_JSON="${AIRLOCK_CONFIG_STATUS_CURRENT_TOPOLOGY_JSON:-$SYSTEM_STATE_DIR/current-topology.json}"
TOPOLOGY_OVERRIDE="${AIRLOCK_CONFIG_STATUS_TOPOLOGY_JSON:-}"
SETUP_JSON="${AIRLOCK_CONFIG_STATUS_SETUP_JSON:-$LOG_DIR/setup-config.json}"
CURRENT_SETUP_JSON="${AIRLOCK_CONFIG_STATUS_CURRENT_SETUP_JSON:-$SYSTEM_STATE_DIR/current-setup.json}"
APPLY_JSON="${AIRLOCK_CONFIG_STATUS_APPLY_JSON:-$LOG_DIR/config-apply.json}"
QREXEC_POLICY_DIR="${AIRLOCK_QUBES_POLICY_DIR:-/var/lib/airlock/qubes-policy.d}"
QREXEC_MANAGED_POLICY="${AIRLOCK_QUBES_MANAGED_POLICY_FILE:-90-airlock.policy}"
QREXEC_POLICY_FILE="$QREXEC_POLICY_DIR/$QREXEC_MANAGED_POLICY"
QVM_LS_BIN="${AIRLOCK_QVM_LS_BIN:-qvm-ls}"
SYSTEM_PROFILE_PATH="${AIRLOCK_CONFIG_STATUS_SYSTEM_PROFILE:-/nix/var/nix/profiles/airlock-system}"
HOST_SYSTEM_PROFILE_PATH="${AIRLOCK_CONFIG_STATUS_HOST_SYSTEM_PROFILE:-/nix/var/nix/profiles/system}"
RUN_CURRENT_SYSTEM_PATH="${AIRLOCK_CONFIG_STATUS_RUN_CURRENT_SYSTEM:-/run/current-system}"

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

readlink_f_or_empty() {
  local path="$1"

  if [ -e "$path" ] || [ -L "$path" ]; then
    readlink -f "$path" || true
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
setup_log_json="$(json_file_or_null "$SETUP_JSON")"
current_setup_json="$(json_file_or_null "$CURRENT_SETUP_JSON")"
if [ "$setup_log_json" != null ]; then
  setup_json="$setup_log_json"
  setup_source="log"
else
  setup_json="$current_setup_json"
  setup_source="current"
fi
apply_json="$(json_file_or_null "$APPLY_JSON")"
current_system_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-system.json")"
current_plan_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-plan.json")"
last_apply_json="$(json_file_or_null "$SYSTEM_STATE_DIR/last-apply.json")"
current_deployment_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-deployment.json")"
last_deployment_json="$(json_file_or_null "$SYSTEM_STATE_DIR/last-deployment.json")"
current_config_apply_json="$(json_file_or_null "$SYSTEM_STATE_DIR/current-config-apply.json")"
last_config_apply_json="$(json_file_or_null "$SYSTEM_STATE_DIR/last-config-apply.json")"
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
  --arg setup_source "$setup_source" \
  --arg current_setup_path "$CURRENT_SETUP_JSON" \
  --arg apply_path "$APPLY_JSON" \
  --arg system_state_dir "$SYSTEM_STATE_DIR" \
  --arg qrexec_policy_file "$QREXEC_POLICY_FILE" \
  --arg qvm_status "$qvm_status" \
  --arg system_profile "$SYSTEM_PROFILE_PATH" \
  --arg system_profile_target "$(readlink_or_empty "$SYSTEM_PROFILE_PATH")" \
  --arg system_profile_resolved_target "$(readlink_f_or_empty "$SYSTEM_PROFILE_PATH")" \
  --arg host_system_profile "$HOST_SYSTEM_PROFILE_PATH" \
  --arg host_system_profile_target "$(readlink_or_empty "$HOST_SYSTEM_PROFILE_PATH")" \
  --arg host_system_profile_resolved_target "$(readlink_f_or_empty "$HOST_SYSTEM_PROFILE_PATH")" \
  --arg run_current_system "$RUN_CURRENT_SYSTEM_PATH" \
  --arg run_current_system_target "$(readlink_or_empty "$RUN_CURRENT_SYSTEM_PATH")" \
  --arg run_current_system_resolved_target "$(readlink_f_or_empty "$RUN_CURRENT_SYSTEM_PATH")" \
  --argjson topology "$topology_json" \
  --argjson boot_topology "$boot_topology_json" \
  --argjson current_topology "$current_topology_json" \
  --argjson setup_log "$setup_log_json" \
  --argjson current_setup "$current_setup_json" \
  --argjson setup "$setup_json" \
  --argjson apply "$apply_json" \
  --argjson current_system "$current_system_json" \
  --argjson current_plan "$current_plan_json" \
  --argjson last_apply "$last_apply_json" \
  --argjson current_deployment "$current_deployment_json" \
  --argjson last_deployment "$last_deployment_json" \
  --argjson current_config_apply "$current_config_apply_json" \
  --argjson last_config_apply "$last_config_apply_json" \
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

    def plan_bool($field):
      if (($current_plan[$field]? | type) == "boolean") then $current_plan[$field]
      elif (($last_apply[$field]? | type) == "boolean") then $last_apply[$field]
      else false
      end;

    def plan_array($field):
      if (($current_plan[$field]? | type) == "array") then $current_plan[$field]
      elif (($last_apply[$field]? | type) == "array") then $last_apply[$field]
      else []
      end;

    def dom0_generation_active:
      $host_system_profile_resolved_target != "" and
      $run_current_system_resolved_target != "" and
      $host_system_profile_resolved_target == $run_current_system_resolved_target;

    def dom0_system_boot_action:
      (.type? == "dom0-nixos-system") and
      ((.switch_action? // .action? // "") == "boot");

    def pending_reboot_actions:
      plan_array("reboot_actions")
      | if dom0_generation_active then
          map(select(dom0_system_boot_action | not))
        else
          .
        end;

    def pending_reboot_reasons:
      pending_reboot_actions as $actions
      | if ($actions | length) > 0 then
          $actions | map(.reason // empty)
        elif plan_bool("reboot_required") and ((plan_array("reboot_actions") | length) == 0) then
          plan_array("reboot_reasons")
        else
          []
        end;

    def pending_reboot_required:
      (pending_reboot_actions | length) > 0 or
      (plan_bool("reboot_required") and
       ((plan_array("reboot_actions") | length) == 0) and
       ((plan_array("reboot_reasons") | length) > 0));

    def setup_array($field):
      if (($setup[$field]? | type) == "array") then $setup[$field] else [] end;

    def pending_vm_restart_actions:
      setup_array("restart_actions");

    def pending_vm_restart_reasons:
      pending_vm_restart_actions | map(.reason // empty) | unique;

    def pending_vm_restart_domains:
      pending_vm_restart_actions | map(.domain // empty) | map(select(. != "")) | unique;

    def pending_vm_restart_required:
      (($setup.restart_required? | type) == "boolean" and $setup.restart_required) or
      ((pending_vm_restart_actions | length) > 0);

    def host_generation:
      {
        active: dom0_generation_active,
        current_source: $run_current_system_resolved_target,
        staged_source: (
          if pending_reboot_required then
            ($last_apply.host_system_source // $host_system_profile_resolved_target)
          else
            ""
          end
        ),
        profile: {
          path: $host_system_profile,
          target: $host_system_profile_target,
          resolved_target: $host_system_profile_resolved_target
        },
        running: {
          path: $run_current_system,
          target: $run_current_system_target,
          resolved_target: $run_current_system_resolved_target
        },
        last_apply: {
          status: ($last_apply.status // null),
          host_system_status: ($last_apply.host_system_status // null),
          source: ($last_apply.host_system_source // ""),
          source_physical: ($last_apply.host_system_source_physical // ""),
          switch_action: ($last_apply.host_system_switch_action // ""),
          reboot_required: ($last_apply.reboot_required // false)
        },
        pending_reboot: {
          required: pending_reboot_required,
          actions: pending_reboot_actions,
          reasons: pending_reboot_reasons
        }
      };

    {
      schema: "airlock.config-status.v1",
      status: status,
      host_generation: host_generation,
      paths: {
        topology: $topology_path,
        boot_topology: $boot_topology_path,
        current_topology: $current_topology_path,
        setup: $setup_path,
        current_setup: $current_setup_path,
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
      pending_reboot: {
        required: pending_reboot_required,
        actions: pending_reboot_actions,
        reasons: pending_reboot_reasons
      },
      pending_vm_restarts: {
        required: pending_vm_restart_required,
        actions: pending_vm_restart_actions,
        domains: pending_vm_restart_domains,
        reasons: pending_vm_restart_reasons
      },
      deployment: {
        current: $current_deployment,
        last_attempt: $last_deployment,
        current_apply: $current_config_apply,
        last_apply_result: $last_config_apply
      },
      setup: $setup,
      setup_source: $setup_source,
      setup_log: $setup_log,
      current_setup: $current_setup,
      apply: $apply,
      system: {
        state_dir: $system_state_dir,
        current_system: $current_system,
        current_plan: $current_plan,
        pending_reboot: {
          required: pending_reboot_required,
          actions: pending_reboot_actions,
          reasons: pending_reboot_reasons
        },
        host_generation: host_generation,
        last_apply: $last_apply,
        profile: $system_profile,
        profile_target: $system_profile_target,
        profile_resolved_target: $system_profile_resolved_target,
        host_system_profile: $host_system_profile,
        host_system_profile_target: $host_system_profile_target,
        host_system_profile_resolved_target: $host_system_profile_resolved_target,
        run_current_system: $run_current_system,
        run_current_system_target: $run_current_system_target,
        run_current_system_resolved_target: $run_current_system_resolved_target
      }
    }
  '
