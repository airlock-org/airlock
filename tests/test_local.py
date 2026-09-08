from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass(frozen=True)
class LocalCheck:
    name: str
    command: tuple[str, ...]
    marks: tuple[pytest.MarkDecorator, ...] = ()


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
        ("{root}/tests/nix-closure-domain-volume-list.sh",),
    ),
    LocalCheck("nix-closure-root-image", ("{root}/tests/nix-closure-root-image.sh",)),
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
    LocalCheck("config-staging-validate", ("{root}/tests/config-staging-validate.sh",)),
    LocalCheck("qrexec-policy-path", ("{root}/tests/qrexec-policy-path.sh",)),
    LocalCheck("qube-guest-policy-probe", ("{root}/tests/qube-guest-policy-probe.sh",)),
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
    LocalCheck("config-boot-verify", ("{root}/tests/config-boot-verify.sh",)),
    LocalCheck("config-status", ("{root}/tests/config-status.sh",)),
    LocalCheck(
        "config-apply-template-build-ref",
        ("{root}/tests/config-apply-template-build-ref.sh",),
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
        (
            "{root}/tests/host-capabilities.sh",
            "--json",
            "{log_dir}/host-capabilities.json",
        ),
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
        (pytest.mark.qubes, pytest.mark.slow),
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


def format_command(
    command: tuple[str, ...], repo_root: Path, log_dir: Path
) -> list[str]:
    return [arg.format(root=str(repo_root), log_dir=str(log_dir)) for arg in command]


def write_log(path: Path, result: subprocess.CompletedProcess[str]) -> None:
    path.write_text(
        "".join(
            (
                f"$ {' '.join(result.args)}\n",
                f"exit_code={result.returncode}\n\n",
                "stdout:\n",
                result.stdout,
                "\n\nstderr:\n",
                result.stderr,
            )
        ),
        encoding="utf-8",
    )


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

    test_log_dir = nixome_log_dir / local_check.name
    test_log_dir.mkdir(parents=True, exist_ok=True)
    command = format_command(local_check.command, repo_root, test_log_dir)
    log_path = test_log_dir / "pytest-command.log"
    env = os.environ.copy()
    env["NIXOME_LOG_DIR"] = str(test_log_dir)

    result = subprocess.run(
        command,
        cwd=repo_root,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    write_log(log_path, result)

    assert (
        result.returncode == 0
    ), f"{local_check.name} failed with exit {result.returncode}; log: {log_path}"
