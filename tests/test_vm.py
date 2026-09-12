from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import sys
import threading
from dataclasses import dataclass
from pathlib import Path
from typing import TextIO

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

BASELINE_BACKED_PARALLEL_FLOWS = frozenset(
    {
        "vm-config-flow",
        "vm-config-boot-flow",
        "vm-nix-closure-flow",
    }
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
    lanes = os.environ.get("AIRLOCK_VM_PARALLEL_LANES", "1")
    return lanes not in {"", "0", "1"}


def vm_parallel_ssh_base_port() -> int:
    value = os.environ.get("AIRLOCK_VM_PARALLEL_SSH_BASE_PORT", "2300")
    try:
        return int(value)
    except ValueError:
        pytest.fail(f"AIRLOCK_VM_PARALLEL_SSH_BASE_PORT must be an integer: {value}")


def vm_flow_has_mark(vm_flow: VmFlow, mark_name: str) -> bool:
    return any(mark.mark.name == mark_name for mark in vm_flow.marks)


def prepare_lane_image_result_link(
    vm_dir: Path,
    shared_image_attr: str,
    shared_image_link: Path,
) -> Path:
    if not shared_image_attr:
        pytest.fail("parallel VM image attr must not be empty")
    try:
        shared_image_output = shared_image_link.resolve(strict=True)
    except FileNotFoundError:
        pytest.fail(
            "parallel VM shared image result link is missing; "
            f"run make vm-parallel-prebuild-image: {shared_image_link}"
        )
    lane_image_link = vm_dir / f"{shared_image_attr}-result"
    if lane_image_link.exists() or lane_image_link.is_symlink():
        if lane_image_link.is_dir() and not lane_image_link.is_symlink():
            pytest.fail(f"refusing to replace non-symlink directory: {lane_image_link}")
        lane_image_link.unlink()
    lane_image_link.symlink_to(shared_image_output)
    return lane_image_link


def required_file_from_env(name: str, purpose: str) -> Path:
    value = os.environ.get(name)
    if not value:
        pytest.fail(f"{name} must be set for {purpose}")

    path = Path(value)
    if not path.is_file():
        pytest.fail(f"{name} for {purpose} is missing or not a file: {path}")
    return path


def format_command(command: list[str]) -> str:
    return shlex.join(command)


def _stream_pipe(pipe: TextIO, stream: TextIO, output: list[str]) -> None:
    try:
        for line in pipe:
            output.append(line)
            stream.write(line)
            stream.flush()
    finally:
        pipe.close()


def run_streamed_command(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert process.stdout is not None
    assert process.stderr is not None

    stdout: list[str] = []
    stderr: list[str] = []
    stdout_thread = threading.Thread(
        target=_stream_pipe,
        args=(process.stdout, sys.stdout, stdout),
        daemon=True,
    )
    stderr_thread = threading.Thread(
        target=_stream_pipe,
        args=(process.stderr, sys.stderr, stderr),
        daemon=True,
    )
    stdout_thread.start()
    stderr_thread.start()

    try:
        returncode = process.wait()
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        stdout_thread.join()
        stderr_thread.join()

    return subprocess.CompletedProcess(
        command,
        returncode,
        stdout="".join(stdout),
        stderr="".join(stderr),
    )


def write_log(path: Path, result: subprocess.CompletedProcess[str]) -> None:
    path.write_text(
        "".join(
            (
                f"$ {format_command(result.args)}\n",
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
    airlock_log_dir: Path,
) -> None:
    if os.environ.get("AIRLOCK_RUN_VM_TESTS") != "1":
        pytest.skip("set AIRLOCK_RUN_VM_TESTS=1 to run nested VM flows")

    parallel_enabled = vm_parallel_enabled()
    if vm_flow.serial and parallel_enabled:
        pytest.skip(f"{vm_flow.name} is not safe for multi-lane VM runs")

    lane = vm_lane_id()
    if parallel_enabled:
        flow_log_dir = airlock_log_dir / "vm-lanes" / str(lane) / vm_flow.name
    else:
        flow_log_dir = airlock_log_dir / vm_flow.name
    flow_log_dir.mkdir(parents=True, exist_ok=True)
    command = ["make", "-C", str(repo_root), vm_flow.make_target]
    env = os.environ.copy()
    env["AIRLOCK_LOG_DIR"] = str(flow_log_dir)
    lane_env = {
        "AIRLOCK_LOG_DIR": str(flow_log_dir),
        "AIRLOCK_VM_PARALLEL_LANES": os.environ.get("AIRLOCK_VM_PARALLEL_LANES", "1"),
    }
    if vm_flow_has_mark(vm_flow, "warm"):
        # Warm flows intentionally keep the nested VM alive between test steps.
        # The guarded Airlock GC must not run in the middle of that sequence.
        env["AIRLOCK_SKIP_GC_AUTO"] = "1"
        lane_env["AIRLOCK_SKIP_GC_AUTO"] = "1"
    if parallel_enabled:
        # VM lanes isolate mutable VM state, result links, logs, and SSH ports.
        # They intentionally share the configured Nix store/cache so expensive
        # builds are reused across workers. The lane-local image result link is
        # prepared from the Makefile prebuild so workers do not each resolve the
        # same base image with nix build.
        data_root = Path(env["AIRLOCK_DATA_ROOT"])
        vm_dir = data_root / "vm-lanes" / str(lane)
        result_link_dir = data_root / "result-links" / "vm-lanes" / str(lane)
        vm_dir.mkdir(parents=True, exist_ok=True)
        result_link_dir.mkdir(parents=True, exist_ok=True)
        shared_image_attr = env.get(
            "AIRLOCK_VM_PARALLEL_IMAGE_ATTR",
            "airlock-vm-gui-config-dev-image",
        )
        shared_image_link_value = env.get("AIRLOCK_VM_PARALLEL_IMAGE_LINK")
        lane_image_link = None
        if shared_image_link_value:
            lane_image_link = prepare_lane_image_result_link(
                vm_dir,
                shared_image_attr,
                Path(shared_image_link_value),
            )
            env["AIRLOCK_VM_SKIP_BASE_BUILD"] = "1"
        if vm_flow.name in BASELINE_BACKED_PARALLEL_FLOWS:
            # Each test lane gets its own writable qcow2 overlay, but the
            # overlay is backed by the read-only prepared baseline. The baseline
            # already ran the common boot/service/setup-config prefix, so these
            # flow targets skip that setup pass and start from the cloned state.
            baseline_disk = required_file_from_env(
                "AIRLOCK_VM_PARALLEL_BASELINE_DISK",
                "baseline-backed parallel VM tests",
            )
            baseline_ovmf_vars = required_file_from_env(
                "AIRLOCK_VM_PARALLEL_BASELINE_OVMF_VARS",
                "baseline-backed parallel VM tests",
            )
            env["AIRLOCK_VM_BACKING_IMAGE"] = str(baseline_disk)
            env["AIRLOCK_VM_OVMF_VARS_TEMPLATE"] = str(baseline_ovmf_vars)
            env["VM_FLOW_RUN_SETUP_CONFIG"] = "0"
        env["AIRLOCK_VM_LANE"] = str(lane)
        env["AIRLOCK_VM_DIR"] = str(vm_dir)
        env["AIRLOCK_RESULT_LINK_DIR"] = str(result_link_dir)
        ssh_port = vm_parallel_ssh_base_port() + lane
        env["AIRLOCK_VM_SSH_PORT"] = str(ssh_port)
        env["AIRLOCK_SKIP_GC_AUTO"] = "1"
        lane_env.update(
            {
                "AIRLOCK_VM_LANE": str(lane),
                "AIRLOCK_VM_DIR": str(vm_dir),
                "AIRLOCK_RESULT_LINK_DIR": str(result_link_dir),
                "AIRLOCK_VM_PARALLEL_SSH_BASE_PORT": os.environ.get(
                    "AIRLOCK_VM_PARALLEL_SSH_BASE_PORT",
                    "2300",
                ),
                "AIRLOCK_VM_SSH_PORT": str(ssh_port),
                "AIRLOCK_SKIP_GC_AUTO": "1",
                "AIRLOCK_VM_PARALLEL_IMAGE_ATTR": shared_image_attr,
                "AIRLOCK_VM_PARALLEL_IMAGE_LINK": shared_image_link_value,
                "AIRLOCK_VM_SKIP_BASE_BUILD": env.get(
                    "AIRLOCK_VM_SKIP_BASE_BUILD", "0"
                ),
                "AIRLOCK_VM_BACKING_IMAGE": env.get("AIRLOCK_VM_BACKING_IMAGE"),
                "AIRLOCK_VM_OVMF_VARS_TEMPLATE": env.get(
                    "AIRLOCK_VM_OVMF_VARS_TEMPLATE"
                ),
                "VM_FLOW_RUN_SETUP_CONFIG": env.get("VM_FLOW_RUN_SETUP_CONFIG"),
            }
        )
        if lane_image_link is not None:
            lane_env["AIRLOCK_VM_IMAGE_RESULT_LINK"] = str(lane_image_link)
    (flow_log_dir / "pytest-vm-env.json").write_text(
        json.dumps(lane_env, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    print(f"vm-flow: {vm_flow.name}: log dir: {flow_log_dir}", flush=True)
    print(f"vm-flow: {vm_flow.name}: running: {format_command(command)}", flush=True)
    result = run_streamed_command(
        command,
        cwd=repo_root,
        env=env,
    )
    log_path = flow_log_dir / "pytest-command.log"
    write_log(log_path, result)
    print(
        f"vm-flow: {vm_flow.name}: exit {result.returncode}; log: {log_path}",
        flush=True,
    )

    assert (
        result.returncode == 0
    ), f"{vm_flow.name} failed with exit {result.returncode}; log: {log_path}"
