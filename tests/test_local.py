"""Parent-owned nested VM orchestration contracts."""

import json
from pathlib import Path
import re
import subprocess

import pytest


def check_run_nested_gui_smoke_static(repo_root: Path, log_dir: Path) -> None:
    result_json = log_dir / "run-nested-gui-smoke-static.json"
    log_dir.mkdir(parents=True, exist_ok=True)

    for rel_path in (
        "lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh",
        "lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh",
        "lifecycle/misc/tools/scripts/sync-nested-codex-auth.sh",
        "lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh",
        "airlock-machine/lifecycle/runtime/tools/scripts/apply-config-candidate.sh",
        "airlock-machine/lifecycle/misc/tools/scripts/nixos-config-vms-smoke.sh",
        "airlock-machine/lifecycle/runtime/tools/scripts/config-boot-verify.sh",
        "lifecycle/misc/tools/scripts/resolve-vm-image-result.sh",
    ):
        subprocess.run(["bash", "-n", str(repo_root / rel_path)], cwd=repo_root, check=True)

    texts = {
        "Makefile": (repo_root / "Makefile").read_text(encoding="utf-8"),
        "run_nested": (
            repo_root / "lifecycle/misc/tools/scripts/run-nested-gui-smoke.sh"
        ).read_text(encoding="utf-8"),
        "config_vms": (
            repo_root / "airlock-machine/lifecycle/misc/tools/scripts/nixos-config-vms-smoke.sh"
        ).read_text(encoding="utf-8"),
        "boot_flow": (
            repo_root
            / "lifecycle/misc/tools/scripts/run-config-boot-activation-flow.sh"
        ).read_text(encoding="utf-8"),
        "run_vm": (
            repo_root / "lifecycle/misc/tools/scripts/run-nested-vm.sh"
        ).read_text(encoding="utf-8"),
        "test_vm": (repo_root / "tests/test_vm.py").read_text(encoding="utf-8"),
        "test_vm_staged": (repo_root / "tests/test_vm_staged.py").read_text(
            encoding="utf-8"
        ),
        "sync_tools": (
            repo_root / "lifecycle/misc/tools/scripts/sync-nested-dom0-tools.sh"
        ).read_text(encoding="utf-8"),
        "sync_codex_auth": (
            repo_root / "lifecycle/misc/tools/scripts/sync-nested-codex-auth.sh"
        ).read_text(encoding="utf-8"),
        "config_inputs_import": (
            repo_root / "airlock-machine/lifecycle/misc/tools/scripts/import-config-inputs.sh"
        ).read_text(encoding="utf-8"),
        "qube_guest": (
            repo_root / "airlock-machine/lifecycle/misc/modules/qube-guest.nix"
        ).read_text(encoding="utf-8"),
        "guest_config_inputs": (
            repo_root / "airlock-machine/lifecycle/misc/modules/qube-guest/config-inputs.nix"
        ).read_text(encoding="utf-8") + (
            repo_root / "airlock-machine/lifecycle/misc/modules/qube-guest/scripts/airlock-config-inputs-import-qrexec.sh"
        ).read_text(encoding="utf-8") + (
            repo_root / "airlock-machine/lifecycle/misc/modules/qube-guest/scripts/airlock-config-inputs-mount.sh"
        ).read_text(encoding="utf-8"),
        "guest_network": (
            repo_root / "airlock-machine/lifecycle/misc/modules/qube-guest/network.nix"
        ).read_text(encoding="utf-8"),
        "ssh_vm": (
            repo_root / "lifecycle/misc/tools/scripts/ssh-nested-vm.sh"
        ).read_text(encoding="utf-8"),
        "apply_candidate": (
            repo_root
            / "airlock-machine/lifecycle/runtime/tools/scripts/apply-config-candidate.sh"
        ).read_text(encoding="utf-8"),
        "closure_smoke": (
            repo_root / "airlock-machine/lifecycle/misc/tools/scripts/nix-closure-domain-smoke.sh"
        ).read_text(encoding="utf-8"),
    }

    for key in ("Makefile", "run_nested", "config_vms"):
        assert "AIRLOCK_CONFIG_BUILD_EXTRA_TEMPLATE" not in texts[key]
    assert (
        re.search(
            r"^(test|test-parallel|test-serial|test-vm|test-vm-parallel|"
            r"test-vm-warm|test-vm-staged|test-list):",
            texts["Makefile"],
            re.MULTILINE,
        )
        is None
    )

    expected_fragments = {
        "Makefile": [
            "vm-config-boot-flow",
            "vm-config-boot-activate-flow",
            "test-quick:",
            "test-warm:",
            "TEST_WARM_VM_MEMORY ?= 16384",
            'AIRLOCK_VM_MEMORY="$${AIRLOCK_VM_MEMORY:-$(TEST_WARM_VM_MEMORY)}"',
            "test-slow:",
            "tests/test_vm_staged.py",
            'AIRLOCK_VM_PARALLEL_LANES="$(TEST_VM_JOBS)"',
            'AIRLOCK_VM_PARALLEL_SSH_BASE_PORT="$(VM_PARALLEL_SSH_BASE_PORT)"',
            "VM_SLOW_SERIAL_VM_DIR ?=",
            "VM_SLOW_SERIAL_SSH_PORT ?=",
            "VM_SLOW_PARALLEL_FLOW_IDS =",
            "VM_SLOW_SELECTED_PARALLEL_TARGETS =",
            "VM_SLOW_SELECTED_SERIAL_TARGETS =",
            "VM_SLOW_DEFAULT_PARALLEL_FLOW_IDS = vm-config-flow vm-nix-closure-flow",
            "VM_SLOW_SERIAL_FLOW_IDS = vm-config-boot-activate-flow vm-pool-admission-flow",
            "tests/test_vm.py::test_vm_flow[$(test)]",
            "AIRLOCK_CONFIG_DOM0_SYSTEM_SWITCH_ACTION=boot",
            "AIRLOCK_AUTO_GC_AFTER_RUN=1",
            "AIRLOCK_KEEP_VM=1",
            "AIRLOCK_REUSE_WARM_VM=1",
            "AIRLOCK_SYNC_DOM0_TOOLS=1",
            "AIRLOCK_VM_SKIP_BASE_BUILD=1",
            "vm-warm-start",
            "vm-warm-sync",
            "vm-sync-codex-auth",
            "sync-nested-codex-auth.sh",
            "vm-warm-apply",
            "vm-warm-test-config",
            "vm-parallel-prebuild-image",
            "AIRLOCK_VM_PARALLEL_IMAGE_LINK",
            "pool-admission-smoke.py",
        ],
        "boot_flow": [
            "AIRLOCK_RUN_CONFIG_BOOT_VERIFY=1",
            "AIRLOCK_SHUTDOWN_VM_ON_SUCCESS=1",
            "resolve-vm-image-result.sh",
            "AIRLOCK_VM_SKIP_BASE_BUILD=1",
            "stage_status: .stage.status",
        ],
        "run_nested": [
            "AIRLOCK_CONFIG_DOM0_SYSTEM_SWITCH_ACTION",
            "AIRLOCK_CONFIG_DOM0_SYSTEM_REASON",
            "AIRLOCK_CONFIG_APPLY_STAGE",
            "AIRLOCK_CONFIG_VMS_SMOKE_MODE",
            "AIRLOCK_RUN_CONFIG_BOOT_VERIFY",
            "airlock-config-boot-verify",
            "AIRLOCK_SETUP_CONFIG_BOOT_WAIT_SECONDS",
            "wait_for_setup_config_boot_unit",
            "systemctl cat airlock-setup-config.service",
            "vm-gui-setup-config-boot.json",
            "copy_boot_setup_config_artifacts",
            "copy_boot_setup_config_result",
            "AIRLOCK_AUTO_GC_AFTER_RUN",
            "AIRLOCK_SYNC_DOM0_TOOLS",
            "AIRLOCK_REUSE_WARM_VM",
            "ssh: reusing existing nested dom0",
            "AIRLOCK_RUN_CONFIG_INPUTS_IMPORT",
            "AIRLOCK_NESTED_WARM_TOPOLOGY_PATH",
            "AIRLOCK_NESTED_WARM_CONFIG_INPUTS_PATH",
            "AIRLOCK_NESTED_WARM_STOP_NIX_DOMAINS",
            "qvm-kill",
            "affected domain did not halt",
            'CONFIG_VMS_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/airlock-config-vms-smoke"',
            'SETUP_CONFIG_TOPOLOGY="$REMOTE_WARM_TOPOLOGY_PATH"',
            'AIRLOCK_NIX_DOMAIN_STORAGE_HELPER="$REMOTE_WARM_BIN_DIR/airlock-nix-domain-storage"',
            "AIRLOCK_NIX_DOMAIN_STORAGE_HELPER \\",
            "AIRLOCK_SETUP_CONFIG_COMMAND;",
            'AIRLOCK_CONFIG_INPUTS_SOURCE="$REMOTE_WARM_CONFIG_INPUTS_PATH"',
            "AIRLOCK_CONFIG_INPUTS_SOURCE \\",
            '--topology $(shell_quote "$SETUP_CONFIG_TOPOLOGY")',
            "sync-nested-dom0-tools.sh",
            "prepare_warm_sync_boot_unit",
            "warm tool sync uses explicit setup-config",
            "Keep the boot oneshot in its active/exited state",
            "vm-gui-sync-dom0-tools.json",
            'AIRLOCK_SETUP_CONFIG_COMMAND="$REMOTE_WARM_BIN_DIR/airlock-setup-config"',
            "CONFIG_VMS_STAGING_UPDATE_LOG",
            "config-vms-config-staging-update.sh",
            "sudo -n $config_vms_env AIRLOCK_LOG_DIR=/var/log/airlock $config_vms_smoke_command",
            '"$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass',
            'NIX_CLOSURE_DOMAIN_SMOKE_COMMAND="$REMOTE_WARM_BIN_DIR/airlock-nix-closure-domain-smoke"',
        ],
        "config_vms": [
            'if $DOM0_SYSTEM_SWITCH_ACTION_NIX == "switch" then',
            'EXPECTED_SYSTEMD_ACTIONS_STATUS="skipped"',
            "grep -F 'rawLocalConfig = import ./airlock/runtime.nix {' flake.nix",
            "grep -F 'nixosConfigurations = nixNativeConfigurations // {' flake.nix",
            "grep -F 'airlock-dom0 = airlockDom0System;' flake.nix",
            "grep -F 'nixNative.domains.",
            'assert_nix_closure_domain "$VAULT"',
            "full|preflight|stage-candidate|apply-candidate|post-apply",
            "CONFIG_CURRENT_BOOTSTRAP_LOG",
            "airlock-config-bootstrap-current",
        ],
        "run_vm": [
            "AIRLOCK_VM_SKIP_BASE_BUILD",
            "using existing Airlock VM base image result link",
            "AIRLOCK_VM_IOMMU",
            "kernel-irqchip=split",
            "intel-iommu,intremap=on,caching-mode=on",
            "virtio-net-pci,disable-legacy=on,iommu_platform=on,vectors=0,netdev=sysnet0",
        ],
        "test_vm": [
            "AIRLOCK_VM_SKIP_BASE_BUILD",
            "AIRLOCK_VM_PARALLEL_SSH_BASE_PORT",
            "AIRLOCK_SKIP_GC_AUTO",
        ],
        "test_vm_staged": [
            "STAGED_VM_STEPS",
            "config-apply-resolve",
            "nix-closure-domain-smoke",
            "AIRLOCK_NIX_CLOSURE_DOMAIN_SMOKE_COMMAND",
        ],
        "sync_tools": [
            "nix-store --export",
            "sudo -n nix-store --import",
            "airlock-config-inputs-import",
            "REMOTE_CONFIG_INPUTS_SYSTEM_PATH",
            'nixos-qubes", .nixosQubes',
            "mount --bind",
            "airlock-config-vms-smoke",
            "nixosConfigurations.vm-gui-config.config.airlock.qubes.topology",
            "nixosConfigurations.airlock-domain-$domain.config.system.build.toplevel",
            "airlock-domain-$domain-store-closure",
            "PYTHONPATH=",
            "systemctl restart qubesd.service",
        ],
        "sync_codex_auth": [
            "airlock.nested-codex-auth-sync.v1",
            "LOCAL_AUTH_JSON",
            "auth.json",
            "AIRLOCK_SYNC_CODEX_CONFIG",
            "config.toml",
            "/etc/airlock/dev-mode",
            "nested dom0 SSH is not reachable",
            "remote nested dom0 is not a dev-mode image",
            "install -m 0600",
            "CODEX_HOME:-$HOME/.codex",
            '"$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass',
        ],
        "config_inputs_import": [
            "store-export.nar.gz",
            'nix-store -q --references "$store_root"',
            'cut -f3 "$tmpdir/config-inputs.tsv"',
            'nix-store --export $(cat "$inputs_store_paths")',
            "store_export_bytes",
        ],
        "qube_guest": ["ps.dbus-python"],
        "guest_config_inputs": [
            'nix-store --store "$store_root" --import',
            "input_relative_path=",
            '${input_path#"$input_store_path"}',
            'mount --bind "$physical_path" "$mountpoint_path"',
        ],
        "guest_network": [
            "airlock-qubes-setup-dnat-to-ns",
            "airlock-qubes-network-proxy-setup",
        ],
        "ssh_vm": [
            '"$ROOT/lifecycle/misc/tools/scripts/airlock-nix.sh" shell nixpkgs#sshpass'
        ],
        "apply_candidate": [
            "AIRLOCK_SETUP_CONFIG_COMMAND",
            'CONFIG_APPLY_STAGE="${AIRLOCK_CONFIG_APPLY_STAGE:-full}"',
            "full|archive|resolve|setup|system|deploy",
        ],
        "closure_smoke": ["AIRLOCK_SETUP_CONFIG_COMMAND"],
    }
    for key, fragments in expected_fragments.items():
        for fragment in fragments:
            assert fragment in texts[key], f"missing {fragment!r} in {key}"

    assert texts["config_vms"].index('run_or_fail "bootstrap current config"') < texts[
        "config_vms"
    ].index('run_qrexec_or_fail "$STAGING" "config staging init"')
    assert texts["config_vms"].index(
        'run_qrexec_or_fail "$STAGING" "config staging init"'
    ) < texts["config_vms"].index(
        'run_qrexec_or_fail "$STAGING" "config staging repo status"'
    )
    assert 'cat "$RESULT_JSON"' not in texts["boot_flow"]
    assert 'cat >> "$STAGING_UPDATE_SCRIPT"' not in texts["config_vms"]
    assert "systemctl stop airlock-setup-config.service" not in texts["run_nested"]
    scripts_text = "\n".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in (repo_root / "lifecycle/misc/tools/scripts").glob("*.sh")
    )
    assert "nix shell nixpkgs#sshpass" not in scripts_text

    result_json.write_text(
        json.dumps(
            {"schema": "airlock.run-nested-gui-smoke-static.v1", "status": "passed"}
        )
        + "\n",
        encoding="utf-8",
    )



@pytest.mark.local
@pytest.mark.parametrize("name", ["run-nested-gui-smoke-static"])
def test_local_check(name, repo_root, airlock_log_dir):
    check_run_nested_gui_smoke_static(repo_root, airlock_log_dir / name)


@pytest.mark.nix
def test_image_flake_check(repo_root, airlock_log_dir):
    """Keep the parent USB adapter covered alongside the machine's flake checks."""
    log = airlock_log_dir / "image-flake-check.log"
    with log.open("w", encoding="utf-8") as output:
        result = subprocess.run(
            [
                str(repo_root / "lifecycle/misc/tools/scripts/airlock-nix.sh"),
                "flake",
                "check",
                "--no-build",
                "--show-trace",
            ],
            cwd=repo_root,
            stdout=output,
            stderr=subprocess.STDOUT,
            check=False,
        )
    assert result.returncode == 0, f"parent image flake check failed; log: {log}"
