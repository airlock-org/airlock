from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass(frozen=True)
class VmFlow:
    name: str
    make_target: str
    marks: tuple[pytest.MarkDecorator, ...] = ()


VM_FLOWS = (
    VmFlow("vm-config-flow", "vm-config-flow"),
    VmFlow("vm-nix-closure-flow", "vm-nix-closure-flow"),
    VmFlow("vm-config-boot-flow", "vm-config-boot-flow"),
    VmFlow("vm-config-boot-activate-flow", "vm-config-boot-activate-flow"),
    VmFlow("vm-gui-smoke", "vm-gui-smoke", (pytest.mark.qubes,)),
)


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "vm_flow" not in metafunc.fixturenames:
        return

    params = [
        pytest.param(
            flow,
            id=flow.name,
            marks=(
                pytest.mark.vm,
                pytest.mark.slow,
                pytest.mark.timeout(7200),
                pytest.mark.xdist_group("vm"),
                *flow.marks,
            ),
        )
        for flow in VM_FLOWS
    ]
    metafunc.parametrize("vm_flow", params)


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


def test_vm_flow(
    vm_flow: VmFlow,
    repo_root: Path,
    nixome_log_dir: Path,
) -> None:
    if os.environ.get("NIXOME_RUN_VM_TESTS") != "1":
        pytest.skip("set NIXOME_RUN_VM_TESTS=1 to run nested VM flows")

    # VM flows are opt-in and serial by default. Their mutable state still gets
    # a per-flow log directory so runtime reports and copied artifacts are scoped.
    flow_log_dir = nixome_log_dir / vm_flow.name
    flow_log_dir.mkdir(parents=True, exist_ok=True)
    command = ["make", "-C", str(repo_root), vm_flow.make_target]
    env = os.environ.copy()
    env["NIXOME_LOG_DIR"] = str(flow_log_dir)

    result = subprocess.run(
        command,
        cwd=repo_root,
        env=env,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    log_path = flow_log_dir / "pytest-command.log"
    write_log(log_path, result)

    assert (
        result.returncode == 0
    ), f"{vm_flow.name} failed with exit {result.returncode}; log: {log_path}"
