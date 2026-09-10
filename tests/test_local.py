from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import pytest

from local_pychecks import (
    check_closure_volume_filtered_device,
    check_closure_store_linux_storage,
    check_closure_store_engine,
    check_closure_store_failure_recovery,
    check_config_candidate_nix_native,
    check_config_candidate_flake,
    check_config_control,
    check_config_apply_domain_references,
    check_config_apply_nix_domain_materialization,
    check_config_apply_template_build_ref,
    check_config_archive_domain_references,
    check_config_system_apply_boot,
    check_config_staging_validate,
    check_config_status,
    check_config_system_apply,
    check_config_topology_eval,
    check_config_validator_candidate,
    check_config_vault_deployment_archive,
    check_dev_role,
    check_host_capabilities,
    check_config_boot_verify,
    check_gc_nixome_store,
    check_nix_domain_storage_helper_shared,
    check_nix_closure_domain_volume_list,
    check_nix_closure_root_image,
    check_nixome_minimal_closure,
    check_nixome_minimal_guest,
    check_nixome_cli,
    check_qrexec_policy_path,
    check_qube_guest_policy_probe,
    check_run_nested_gui_smoke_static,
    check_setup_config_empty_global_pref,
    check_setup_config_auto_sys_net_pci,
    check_setup_config_external_netvm,
    check_setup_config_idempotent,
    check_setup_config_nix_native,
    check_setup_config_nix_native_running_storage,
    check_setup_config_pref_order,
    check_setup_config_qrexec_policy,
    check_setup_config_restart_actions,
    check_setup_config_runtime_path,
    check_setup_config_static,
    check_setup_config_sys_net_order,
    check_system_schema,
    check_topology_schema,
)
from local_harness import run_local_command


@dataclass(frozen=True)
class LocalCheck:
    name: str
    command: tuple[str, ...] = ()
    marks: tuple[pytest.MarkDecorator, ...] = ()
    runner: Callable[[Path, Path], None] | None = None


NIX_BUILD_FLAGS = ("--print-build-logs", "--show-trace")


LOCAL_CHECKS = (
    LocalCheck(
        "flake-check-no-build",
        ("nix", "flake", "check", "{root}", "--no-build", "--show-trace"),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "module-contract",
        (
            "nix",
            "build",
            "{root}#checks.x86_64-linux.module-contract",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-apply-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-apply",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-archive-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-archive-candidate",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-boot-verify-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-boot-verify",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-status-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-status",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-inputs-import-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-inputs-import",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-vms-smoke-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-vms-smoke",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-bootstrap-current-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-bootstrap-current",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "observe-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-observe-dom0",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "setup-config-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-setup-config",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "qubes-storage-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-qubes-storage",
            *NIX_BUILD_FLAGS,
        ),
        (pytest.mark.nix,),
    ),
    LocalCheck("closure-store-engine", runner=check_closure_store_engine),
    LocalCheck(
        "closure-store-failure-recovery",
        runner=check_closure_store_failure_recovery,
    ),
    LocalCheck("closure-store-linux-storage", runner=check_closure_store_linux_storage),
    LocalCheck(
        "closure-volume-filtered-device",
        runner=check_closure_volume_filtered_device,
    ),
    LocalCheck(
        "nix-domain-storage-helper-shared",
        runner=check_nix_domain_storage_helper_shared,
    ),
    LocalCheck(
        "nix-closure-domain-volume-list",
        runner=check_nix_closure_domain_volume_list,
    ),
    LocalCheck("nix-closure-root-image", runner=check_nix_closure_root_image),
    LocalCheck(
        "setup-config-runtime-path",
        runner=check_setup_config_runtime_path,
    ),
    LocalCheck("gc-nixome-store", runner=check_gc_nixome_store),
    LocalCheck(
        "config-topology-eval",
        marks=(pytest.mark.nix,),
        runner=check_config_topology_eval,
    ),
    LocalCheck(
        "nixome-minimal-guest",
        marks=(pytest.mark.nix,),
        runner=check_nixome_minimal_guest,
    ),
    LocalCheck(
        "nixome-minimal-closure",
        marks=(pytest.mark.nix,),
        runner=check_nixome_minimal_closure,
    ),
    LocalCheck("dev-role", marks=(pytest.mark.nix,), runner=check_dev_role),
    LocalCheck("config-staging-validate", runner=check_config_staging_validate),
    LocalCheck("qrexec-policy-path", runner=check_qrexec_policy_path),
    LocalCheck("qube-guest-policy-probe", runner=check_qube_guest_policy_probe),
    LocalCheck("topology-schema", runner=check_topology_schema),
    LocalCheck("system-schema", runner=check_system_schema),
    LocalCheck(
        "config-candidate-flake",
        marks=(pytest.mark.nix,),
        runner=check_config_candidate_flake,
    ),
    LocalCheck(
        "config-validator-candidate",
        marks=(pytest.mark.nix,),
        runner=check_config_validator_candidate,
    ),
    LocalCheck(
        "config-vault-deployment-archive",
        runner=check_config_vault_deployment_archive,
    ),
    LocalCheck(
        "config-archive-domain-references",
        runner=check_config_archive_domain_references,
    ),
    LocalCheck("nixome-cli", marks=(pytest.mark.nix,), runner=check_nixome_cli),
    LocalCheck("config-control", runner=check_config_control),
    LocalCheck("config-boot-verify", runner=check_config_boot_verify),
    LocalCheck("config-status", runner=check_config_status),
    LocalCheck(
        "config-apply-template-build-ref",
        runner=check_config_apply_template_build_ref,
    ),
    LocalCheck(
        "config-apply-domain-references",
        runner=check_config_apply_domain_references,
    ),
    LocalCheck(
        "config-candidate-nix-native",
        marks=(pytest.mark.nix,),
        runner=check_config_candidate_nix_native,
    ),
    LocalCheck(
        "config-apply-nix-domain-materialization",
        runner=check_config_apply_nix_domain_materialization,
    ),
    LocalCheck("config-system-apply", runner=check_config_system_apply),
    LocalCheck("config-system-apply-boot", runner=check_config_system_apply_boot),
    LocalCheck(
        "run-nested-gui-smoke-static",
        runner=check_run_nested_gui_smoke_static,
    ),
    LocalCheck(
        "host-capabilities",
        runner=check_host_capabilities,
    ),
    LocalCheck("setup-config-static", runner=check_setup_config_static),
    LocalCheck(
        "setup-config-nix-native",
        marks=(pytest.mark.nix,),
        runner=check_setup_config_nix_native,
    ),
    LocalCheck(
        "setup-config-nix-native-running-storage",
        runner=check_setup_config_nix_native_running_storage,
        marks=(pytest.mark.nix,),
    ),
    LocalCheck(
        "setup-config-qrexec-policy",
        runner=check_setup_config_qrexec_policy,
    ),
    LocalCheck(
        "setup-config-external-netvm",
        runner=check_setup_config_external_netvm,
    ),
    LocalCheck(
        "setup-config-auto-sys-net-pci",
        runner=check_setup_config_auto_sys_net_pci,
    ),
    LocalCheck(
        "setup-config-sys-net-order",
        runner=check_setup_config_sys_net_order,
    ),
    LocalCheck("setup-config-idempotent", runner=check_setup_config_idempotent),
    LocalCheck(
        "setup-config-restart-actions",
        runner=check_setup_config_restart_actions,
    ),
    LocalCheck("setup-config-pref-order", runner=check_setup_config_pref_order),
    LocalCheck(
        "setup-config-empty-global-pref",
        runner=check_setup_config_empty_global_pref,
    ),
    LocalCheck(
        "qubes-smoke",
        ("nix", "run", "{root}#nixome-qubes-smoke"),
        (pytest.mark.qubes, pytest.mark.slow, pytest.mark.xdist_group("qubes")),
    ),
)


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "local_check" not in metafunc.fixturenames:
        return

    params = [
        pytest.param(check, id=check.name, marks=(pytest.mark.local, *check.marks))
        for check in LOCAL_CHECKS
    ]
    metafunc.parametrize("local_check", params)


def test_local_check(
    local_check: LocalCheck,
    repo_root: Path,
    nixome_log_dir: Path,
) -> None:
    if (
        local_check.name == "qubes-smoke"
        and os.environ.get("NIXOME_RUN_QUBES_SMOKE") != "1"
    ):
        pytest.skip("set NIXOME_RUN_QUBES_SMOKE=1 on a Xen dom0 target")

    if local_check.runner is not None:
        test_log_dir = nixome_log_dir / local_check.name
        test_log_dir.mkdir(parents=True, exist_ok=True)
        local_check.runner(repo_root, test_log_dir)
        return

    run_local_command(
        name=local_check.name,
        command=local_check.command,
        repo_root=repo_root,
        nixome_log_dir=nixome_log_dir,
    )
