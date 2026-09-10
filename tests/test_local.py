from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import pytest

from local_pychecks import (
    check_host_capabilities,
    check_nix_closure_domain_volume_list,
    check_nix_closure_root_image,
    check_qrexec_policy_path,
    check_qube_guest_policy_probe,
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
        "config-seed-inputs-import-package",
        (
            "nix",
            "build",
            "{root}#packages.x86_64-linux.nixome-config-seed-inputs-import",
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
    LocalCheck("closure-store-engine", ("{root}/tests/closure-store-engine.sh",)),
    LocalCheck(
        "closure-store-failure-recovery",
        ("{root}/tests/closure-store-failure-recovery.sh",),
    ),
    LocalCheck(
        "closure-store-linux-storage", ("{root}/tests/closure-store-linux-storage.sh",)
    ),
    LocalCheck(
        "closure-volume-filtered-device",
        ("{root}/tests/closure-volume-filtered-device.sh",),
    ),
    LocalCheck(
        "nix-domain-storage-helper-shared",
        ("{root}/tests/nix-domain-storage-helper-shared.sh",),
    ),
    LocalCheck(
        "nix-closure-domain-volume-list",
        runner=check_nix_closure_domain_volume_list,
    ),
    LocalCheck("nix-closure-root-image", runner=check_nix_closure_root_image),
    LocalCheck(
        "setup-config-runtime-path", ("{root}/tests/setup-config-runtime-path.sh",)
    ),
    LocalCheck("gc-nixome-store", ("{root}/tests/gc-nixome-store.sh",)),
    LocalCheck(
        "config-topology-eval",
        ("{root}/tests/config-topology-eval.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "nixome-minimal-guest",
        ("{root}/tests/nixome-minimal-guest.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "nixome-minimal-closure",
        ("{root}/tests/nixome-minimal-closure.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "dev-role",
        ("{root}/tests/dev-role.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck("config-staging-validate", ("{root}/tests/config-staging-validate.sh",)),
    LocalCheck("qrexec-policy-path", runner=check_qrexec_policy_path),
    LocalCheck("qube-guest-policy-probe", runner=check_qube_guest_policy_probe),
    LocalCheck("topology-schema", ("{root}/tests/topology-schema.sh",)),
    LocalCheck("system-schema", ("{root}/tests/system-schema.sh",)),
    LocalCheck(
        "config-candidate-flake",
        ("{root}/tests/config-candidate-flake.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-validator-candidate",
        ("{root}/tests/config-validator-candidate.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-vault-deployment-archive",
        ("{root}/tests/config-vault-deployment-archive.sh",),
    ),
    LocalCheck(
        "config-archive-domain-references",
        ("{root}/tests/config-archive-domain-references.sh",),
    ),
    LocalCheck("nixome-cli", ("{root}/tests/nixome-cli.sh",), (pytest.mark.nix,)),
    LocalCheck("config-control", ("{root}/tests/config-control.sh",)),
    LocalCheck("config-boot-verify", ("{root}/tests/config-boot-verify.sh",)),
    LocalCheck("config-status", ("{root}/tests/config-status.sh",)),
    LocalCheck(
        "config-apply-template-build-ref",
        ("{root}/tests/config-apply-template-build-ref.sh",),
    ),
    LocalCheck(
        "config-apply-domain-references",
        ("{root}/tests/config-apply-domain-references.sh",),
    ),
    LocalCheck(
        "config-candidate-nix-native",
        ("{root}/tests/config-candidate-nix-native.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "config-apply-nix-domain-materialization",
        ("{root}/tests/config-apply-nix-domain-materialization.sh",),
    ),
    LocalCheck("config-system-apply", ("{root}/tests/config-system-apply.sh",)),
    LocalCheck(
        "config-system-apply-boot", ("{root}/tests/config-system-apply-boot.sh",)
    ),
    LocalCheck(
        "run-nested-gui-smoke-static", ("{root}/tests/run-nested-gui-smoke-static.sh",)
    ),
    LocalCheck(
        "host-capabilities",
        runner=check_host_capabilities,
    ),
    LocalCheck("setup-config-static", ("{root}/tests/setup-config-static.sh",)),
    LocalCheck(
        "setup-config-nix-native",
        ("{root}/tests/setup-config-nix-native.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "setup-config-nix-native-running-storage",
        ("{root}/tests/setup-config-nix-native-running-storage.sh",),
        (pytest.mark.nix,),
    ),
    LocalCheck(
        "setup-config-qrexec-policy", ("{root}/tests/setup-config-qrexec-policy.sh",)
    ),
    LocalCheck(
        "setup-config-external-netvm",
        ("{root}/tests/setup-config-external-netvm.sh",),
    ),
    LocalCheck("setup-config-idempotent", ("{root}/tests/setup-config-idempotent.sh",)),
    LocalCheck(
        "setup-config-restart-actions",
        ("{root}/tests/setup-config-restart-actions.sh",),
    ),
    LocalCheck("setup-config-pref-order", ("{root}/tests/setup-config-pref-order.sh",)),
    LocalCheck(
        "setup-config-empty-global-pref",
        ("{root}/tests/setup-config-empty-global-pref.sh",),
    ),
    LocalCheck(
        "qubes-smoke",
        ("{root}/tests/qubes-smoke.sh",),
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
