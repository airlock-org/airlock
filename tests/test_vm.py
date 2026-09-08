from __future__ import annotations

import json
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass(frozen=True)
class VmFlow:
    name: str
    make_target: str
    marks: tuple[pytest.MarkDecorator, ...] = ()
    serial: bool = False


VM_FLOWS = (
    VmFlow(
        "vm-prebuild-shared",
        "vm-prebuild-shared",
        (pytest.mark.prebuild, pytest.mark.serial_vm),
        serial=True,
    ),
    VmFlow("vm-config-flow", "vm-config-flow"),
    VmFlow("vm-nix-closure-flow", "vm-nix-closure-flow"),
    VmFlow("vm-config-boot-flow", "vm-config-boot-flow"),
    VmFlow(
        "vm-config-boot-activate-flow",
        "vm-config-boot-activate-flow",
        (pytest.mark.serial_vm,),
        serial=True,
    ),
    VmFlow(
        "vm-gui-smoke",
        "vm-gui-smoke",
        (pytest.mark.qubes, pytest.mark.serial_vm),
        serial=True,
    ),
    VmFlow("vm-warm-start", "vm-warm-start", (pytest.mark.warm,)),
    VmFlow("vm-warm-sync", "vm-warm-sync", (pytest.mark.warm,)),
    VmFlow("vm-warm-apply", "vm-warm-apply", (pytest.mark.warm,)),
    VmFlow("vm-warm-test-config", "vm-warm-test-config", (pytest.mark.warm,)),
    VmFlow("vm-config-warm-flow", "vm-config-warm-flow", (pytest.mark.warm,)),
    VmFlow("vm-nix-closure-warm-flow", "vm-nix-closure-warm-flow", (pytest.mark.warm,)),
)


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "vm_flow" not in metafunc.fixturenames:
        return

    params = []
    for flow in VM_FLOWS:
        marks = [
            pytest.mark.vm,
            pytest.mark.slow,
            pytest.mark.timeout(7200),
            *flow.marks,
        ]
        if flow.serial:
            marks.append(pytest.mark.xdist_group("vm"))
        params.append(pytest.param(flow, id=flow.name, marks=marks))
    metafunc.parametrize("vm_flow", params)


def vm_lane_id() -> int:
    worker = os.environ.get("PYTEST_XDIST_WORKER")
    if not worker:
        return 0

    match = re.fullmatch(r"gw(\d+)", worker)
    if not match:
        return 0
    return int(match.group(1))


def vm_parallel_enabled() -> bool:
    lanes = os.environ.get("NIXOME_VM_PARALLEL_LANES", "1")
    return lanes not in {"", "0", "1"}


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

    parallel_enabled = vm_parallel_enabled()
    if vm_flow.serial and parallel_enabled:
        pytest.skip(f"{vm_flow.name} is not safe for multi-lane VM runs")

    lane = vm_lane_id()
    if parallel_enabled:
        flow_log_dir = nixome_log_dir / "vm-lanes" / str(lane) / vm_flow.name
    else:
        flow_log_dir = nixome_log_dir / vm_flow.name
    flow_log_dir.mkdir(parents=True, exist_ok=True)
    command = ["make", "-C", str(repo_root), vm_flow.make_target]
    env = os.environ.copy()
    env["NIXOME_LOG_DIR"] = str(flow_log_dir)
    lane_env = {
        "NIXOME_LOG_DIR": str(flow_log_dir),
        "NIXOME_VM_PARALLEL_LANES": os.environ.get("NIXOME_VM_PARALLEL_LANES", "1"),
    }
    if parallel_enabled:
        # VM lanes isolate mutable VM state, result links, logs, and SSH ports.
        # They intentionally share the configured Nix store/cache so expensive
        # builds are reused across workers.
        data_root = Path(env["NIXOME_DATA_ROOT"])
        vm_dir = data_root / "vm-lanes" / str(lane)
        result_link_dir = data_root / "result-links" / "vm-lanes" / str(lane)
        vm_dir.mkdir(parents=True, exist_ok=True)
        result_link_dir.mkdir(parents=True, exist_ok=True)
        env["NIXOME_VM_LANE"] = str(lane)
        env["NIXOME_VM_DIR"] = str(vm_dir)
        env["NIXOME_RESULT_LINK_DIR"] = str(result_link_dir)
        env["NIXOME_VM_SSH_PORT"] = str(2222 + lane)
        env["NIXOME_SKIP_GC_AUTO"] = "1"
        lane_env.update(
            {
                "NIXOME_VM_LANE": str(lane),
                "NIXOME_VM_DIR": str(vm_dir),
                "NIXOME_RESULT_LINK_DIR": str(result_link_dir),
                "NIXOME_VM_SSH_PORT": str(2222 + lane),
                "NIXOME_SKIP_GC_AUTO": "1",
            }
        )
    (flow_log_dir / "pytest-vm-env.json").write_text(
        json.dumps(lane_env, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

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
