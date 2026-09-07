#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash -n "$ROOT/scripts/run-nested-gui-smoke.sh"
bash -n "$ROOT/scripts/run-config-boot-activation-flow.sh"
bash -n "$ROOT/scripts/nixos-config-vms-smoke.sh"
bash -n "$ROOT/scripts/config-boot-verify.sh"

grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'vm-config-boot-flow' "$ROOT/Makefile" >/dev/null
grep -F 'vm-config-boot-activate-flow' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_RUN_CONFIG_BOOT_VERIFY=1' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
grep -F 'NIXOME_SHUTDOWN_VM_ON_SUCCESS=1' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
grep -F 'NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_CONFIG_DOM0_SYSTEM_REASON' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_RUN_CONFIG_BOOT_VERIFY' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'nixome-config-boot-verify' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_SETUP_CONFIG_BOOT_WAIT_SECONDS' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'wait_for_setup_config_boot_unit' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'systemctl cat nixome-setup-config.service' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'vm-gui-setup-config-boot.json' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'copy_boot_setup_config_artifacts' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'copy_boot_setup_config_result' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'if $DOM0_SYSTEM_SWITCH_ACTION_NIX == "switch" then' \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'EXPECTED_SYSTEMD_ACTIONS_STATUS="skipped"' \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
! grep -F 'cat >> "$STAGING_UPDATE_SCRIPT"' "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'NIXOME_AUTO_GC_AFTER_RUN=1' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_AUTO_GC_AFTER_RUN' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'sudo -n $config_vms_env NIXOME_LOG_DIR=/var/log/nixome nixome-config-vms-smoke' \
  "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null

printf '{"schema":"nixome.run-nested-gui-smoke-static.v1","status":"passed"}\n'
