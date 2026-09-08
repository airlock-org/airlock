#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

bash -n "$ROOT/scripts/run-nested-gui-smoke.sh"
bash -n "$ROOT/scripts/sync-nested-dom0-tools.sh"
bash -n "$ROOT/scripts/run-config-boot-activation-flow.sh"
bash -n "$ROOT/scripts/nixos-config-vms-smoke.sh"
bash -n "$ROOT/scripts/config-boot-verify.sh"
bash -n "$ROOT/scripts/resolve-vm-image-result.sh"

! grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/Makefile" >/dev/null
! grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
! grep -F 'NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE' "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'vm-config-boot-flow' "$ROOT/Makefile" >/dev/null
grep -F 'vm-config-boot-activate-flow' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_RUN_CONFIG_BOOT_VERIFY=1' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
grep -F 'NIXOME_SHUTDOWN_VM_ON_SUCCESS=1' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
grep -F 'stage_status: .stage.status' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
! grep -F 'cat "$RESULT_JSON"' "$ROOT/scripts/run-config-boot-activation-flow.sh" >/dev/null
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
grep -F "grep -F 'rawLocalConfig = import ./nixome/machine.nix {' flake.nix" \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F "grep -F 'nixosConfigurations = nixNativeConfigurations // {' flake.nix" \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F "grep -F 'nixome-dom0 = nixomeDom0System;' flake.nix" \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F "grep -F 'nixNative.domains." "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'assert_nix_closure_domain "$VAULT"' \
  "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
! grep -F 'cat >> "$STAGING_UPDATE_SCRIPT"' "$ROOT/scripts/nixos-config-vms-smoke.sh" >/dev/null
grep -F 'NIXOME_AUTO_GC_AFTER_RUN=1' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_KEEP_VM=1' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_REUSE_WARM_VM=1' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_SYNC_DOM0_TOOLS=1' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_VM_SKIP_BASE_BUILD=1' "$ROOT/Makefile" >/dev/null
grep -F 'vm-warm-start' "$ROOT/Makefile" >/dev/null
grep -F 'vm-warm-sync' "$ROOT/Makefile" >/dev/null
grep -F 'vm-warm-apply' "$ROOT/Makefile" >/dev/null
grep -F 'vm-warm-test-config' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_AUTO_GC_AFTER_RUN' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_SYNC_DOM0_TOOLS' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_REUSE_WARM_VM' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'ssh: reusing existing nested dom0' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_NESTED_WARM_TOPOLOGY_PATH' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_NESTED_WARM_SEED_INPUTS_PATH' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_NESTED_WARM_STOP_NIX_DOMAINS' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'qvm-kill' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'affected domain did not halt' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'CONFIG_VMS_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-config-vms-smoke"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'SETUP_CONFIG_TOPOLOGY="$REMOTE_WARM_TOPOLOGY_PATH"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_NIX_DOMAIN_STORAGE_HELPER="$REMOTE_WARM_BIN_DIR/nixome-nix-domain-storage"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_NIX_DOMAIN_STORAGE_HELPER;' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_CONFIG_SEED_INPUTS_SOURCE="$REMOTE_WARM_SEED_INPUTS_PATH"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_CONFIG_SEED_INPUTS_SOURCE \' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F -- '--topology $(shell_quote "$SETUP_CONFIG_TOPOLOGY")' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIXOME_VM_SKIP_BASE_BUILD' "$ROOT/scripts/run-nested-vm.sh" >/dev/null
grep -F 'using existing Nixome VM base image result link' "$ROOT/scripts/run-nested-vm.sh" >/dev/null
grep -F 'vm-parallel-prebuild-image' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_VM_PARALLEL_IMAGE_LINK' "$ROOT/Makefile" >/dev/null
grep -F 'NIXOME_VM_SKIP_BASE_BUILD' "$ROOT/tests/test_vm.py" >/dev/null
grep -F 'sync-nested-dom0-tools.sh' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'prepare_warm_sync_boot_unit' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'warm tool sync uses explicit setup-config' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'vm-gui-sync-dom0-tools.json' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'SETUP_CONFIG_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-setup-config"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/nixome-nix-closure-domain-smoke"' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'nix-store --export' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'sudo -n nix-store --import' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'nixome-config-seed-inputs-import' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'REMOTE_SEED_INPUTS_COMPAT_PATH' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'nixos-qubes", .nixosQubes' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'mount --bind' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'store-export.nar.gz' "$ROOT/scripts/import-config-seed-inputs.sh" >/dev/null
grep -F 'nix-store -q --references "$store_root"' "$ROOT/scripts/import-config-seed-inputs.sh" >/dev/null
grep -F 'cut -f3 "$tmpdir/seed-inputs.tsv"' "$ROOT/scripts/import-config-seed-inputs.sh" >/dev/null
grep -F 'nix-store --export $(cat "$seed_store_paths")' "$ROOT/scripts/import-config-seed-inputs.sh" >/dev/null
grep -F 'store_export_bytes' "$ROOT/scripts/import-config-seed-inputs.sh" >/dev/null
grep -F 'nix-store --store "$store_root" --import' "$ROOT/modules/qube-guest.nix" >/dev/null
grep -F 'input_relative_path="'"''"'${input_path#"$input_store_path"}"' "$ROOT/modules/qube-guest.nix" >/dev/null
grep -F 'mount --bind "$physical_path" "$mountpoint_path"' "$ROOT/modules/qube-guest.nix" >/dev/null
grep -F 'nixome-config-vms-smoke' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'nixosConfigurations.vm-gui-config.config.nixome.qubes.topology' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'nixosConfigurations.nixome-domain-$domain.config.system.build.toplevel' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'nixome-domain-$domain-store-closure' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'PYTHONPATH=' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'systemctl restart qubesd.service' "$ROOT/scripts/sync-nested-dom0-tools.sh" >/dev/null
grep -F 'CONFIG_VMS_STAGING_UPDATE_LOG' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'config-vms-config-staging-update.sh' "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F 'sudo -n $config_vms_env NIXOME_LOG_DIR=/var/log/nixome $config_vms_smoke_command' \
  "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F '"$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass' \
  "$ROOT/scripts/run-nested-gui-smoke.sh" >/dev/null
grep -F '"$ROOT/scripts/nixome-nix.sh" shell nixpkgs#sshpass' \
  "$ROOT/scripts/ssh-nested-vm.sh" >/dev/null
! grep -R -F 'nix shell nixpkgs#sshpass' "$ROOT/scripts" >/dev/null

printf '{"schema":"nixome.run-nested-gui-smoke-static.v1","status":"passed"}\n'
