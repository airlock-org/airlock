from __future__ import annotations

import json
import os
import shlex
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import TextIO

import pytest

REMOTE_WARM_BIN_DIR = "/run/nixome-warm/bin"
REMOTE_WARM_SEED_INPUTS = "/run/nixome-warm/config-seed-inputs"


@dataclass(frozen=True)
class StagedVmStep:
    name: str
    command: tuple[str, ...]
    env: dict[str, str]
    marks: tuple[pytest.MarkDecorator, ...] = ()


def run_nested_gui_smoke_env(**overrides: str) -> dict[str, str]:
    env = {
        "NIXOME_KEEP_VM": "1",
        "NIXOME_REUSE_WARM_VM": "1",
        "NIXOME_SYNC_DOM0_TOOLS": "0",
        "NIXOME_NESTED_WARM_STOP_NIX_DOMAINS": "1",
        "NIXOME_VM_SKIP_BASE_BUILD": "1",
        "NIXOME_VM_IMAGE_ATTR": "nixome-vm-gui-config-image",
        "NIXOME_VM_DISPLAY": "none",
        "NIXOME_VM_FRESH": "0",
        "NIXOME_BOOTSTRAP_QUBES": "0",
        "NIXOME_RUN_SETUP_CONFIG": "0",
        "NIXOME_RUN_CONFIG_SEED_INPUTS_IMPORT": "0",
        "NIXOME_RUN_CONFIG_VMS_SMOKE": "1",
        "NIXOME_RUN_NIX_CLOSURE_SMOKE": "0",
        "NIXOME_RUN_QUBES_SMOKE": "0",
        "NIXOME_RUN_GUI_SMOKE": "0",
        "NIXOME_CONFIG_SEED_INPUTS_SOURCE": REMOTE_WARM_SEED_INPUTS,
        "NIXOME_CONFIG_VMS_SMOKE_COMMAND": (
            f"{REMOTE_WARM_BIN_DIR}/nixome-config-vms-smoke"
        ),
        "NIXOME_SETUP_CONFIG_COMMAND": f"{REMOTE_WARM_BIN_DIR}/nixome-setup-config",
        "NIXOME_NIX_DOMAIN_STORAGE_HELPER": (
            f"{REMOTE_WARM_BIN_DIR}/nixome-nix-domain-storage"
        ),
    }
    env.update(overrides)
    return env


STAGED_VM_STEPS = (
    StagedVmStep("warm-start", ("make", "vm-warm-start"), {}),
    StagedVmStep("warm-baseline", ("make", "vm-warm-apply"), {}),
    StagedVmStep(
        "config-vms-preflight",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="preflight",
            NIXOME_CONFIG_VMS_IMPORT_SEED_INPUTS="0",
        ),
    ),
    StagedVmStep(
        "config-vms-stage-candidate",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(NIXOME_CONFIG_VMS_SMOKE_MODE="stage-candidate"),
    ),
    StagedVmStep(
        "config-apply-archive",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="apply-candidate",
            NIXOME_CONFIG_APPLY_STAGE="archive",
        ),
    ),
    StagedVmStep(
        "config-apply-resolve",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="apply-candidate",
            NIXOME_CONFIG_APPLY_STAGE="resolve",
        ),
    ),
    StagedVmStep(
        "config-apply-setup",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="apply-candidate",
            NIXOME_CONFIG_APPLY_STAGE="setup",
        ),
    ),
    StagedVmStep(
        "config-apply-system",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="apply-candidate",
            NIXOME_CONFIG_APPLY_STAGE="system",
        ),
    ),
    StagedVmStep(
        "config-apply-deploy",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(
            NIXOME_CONFIG_VMS_SMOKE_MODE="apply-candidate",
            NIXOME_CONFIG_APPLY_STAGE="deploy",
        ),
    ),
    StagedVmStep(
        "config-vms-post-apply",
        ("{root}/scripts/run-nested-gui-smoke.sh",),
        run_nested_gui_smoke_env(NIXOME_CONFIG_VMS_SMOKE_MODE="post-apply"),
    ),
)


def pytest_generate_tests(metafunc: pytest.Metafunc) -> None:
    if "staged_vm_step" not in metafunc.fixturenames:
        return

    params = []
    for step in STAGED_VM_STEPS:
        marks = [
            pytest.mark.vm,
            pytest.mark.slow,
            pytest.mark.warm,
            pytest.mark.staged_vm,
            pytest.mark.timeout(7200),
            pytest.mark.xdist_group("vm-staged"),
            *step.marks,
        ]
        params.append(pytest.param(step, id=step.name, marks=marks))
    metafunc.parametrize("staged_vm_step", params)


def format_command(command: list[str]) -> str:
    return shlex.join(command)


def format_step_command(command: tuple[str, ...], repo_root: Path) -> list[str]:
    return [arg.format(root=str(repo_root)) for arg in command]


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


@pytest.fixture(scope="module")
def staged_vm_state(nixome_log_dir: Path) -> Path:
    state_dir = nixome_log_dir / "vm-staged"
    state_dir.mkdir(parents=True, exist_ok=True)
    failure = state_dir / "failed-step.txt"
    failure.unlink(missing_ok=True)
    return state_dir


@pytest.fixture(scope="module")
def staged_apply_message(staged_vm_state: Path) -> str:
    configured = os.environ.get("NIXOME_CONFIG_APPLY_MESSAGE")
    if configured:
        return configured

    value = f"pytest-staged-{time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())}"
    (staged_vm_state / "apply-message.txt").write_text(value + "\n", encoding="utf-8")
    return value


def test_staged_vm_step(
    staged_vm_step: StagedVmStep,
    repo_root: Path,
    nixome_log_dir: Path,
    staged_vm_state: Path,
    staged_apply_message: str,
) -> None:
    if os.environ.get("NIXOME_RUN_VM_TESTS") != "1":
        pytest.skip("set NIXOME_RUN_VM_TESTS=1 to run nested VM flows")

    failure = staged_vm_state / "failed-step.txt"
    if failure.exists():
        failed_step = failure.read_text(encoding="utf-8").strip()
        pytest.skip(f"previous staged VM step failed: {failed_step}")

    step_log_dir = nixome_log_dir / "vm-staged" / staged_vm_step.name
    step_log_dir.mkdir(parents=True, exist_ok=True)
    command = format_step_command(staged_vm_step.command, repo_root)
    env = os.environ.copy()
    env.update(staged_vm_step.env)
    env["NIXOME_LOG_DIR"] = str(step_log_dir)
    env["NIXOME_CONFIG_APPLY_MESSAGE"] = staged_apply_message
    env["NIXOME_SKIP_GC_AUTO"] = "1"

    (step_log_dir / "pytest-vm-staged-env.json").write_text(
        json.dumps(
            {
                "NIXOME_CONFIG_APPLY_MESSAGE": staged_apply_message,
                **{key: env[key] for key in sorted(staged_vm_step.env) if key in env},
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"vm-staged: {staged_vm_step.name}: log dir: {step_log_dir}", flush=True)
    print(
        f"vm-staged: {staged_vm_step.name}: running: {format_command(command)}",
        flush=True,
    )
    result = run_streamed_command(command, cwd=repo_root, env=env)
    log_path = step_log_dir / "pytest-command.log"
    write_log(log_path, result)
    print(
        f"vm-staged: {staged_vm_step.name}: exit {result.returncode}; log: {log_path}",
        flush=True,
    )

    if result.returncode != 0:
        failure.write_text(staged_vm_step.name + "\n", encoding="utf-8")

    assert (
        result.returncode == 0
    ), f"{staged_vm_step.name} failed with exit {result.returncode}; log: {log_path}"
