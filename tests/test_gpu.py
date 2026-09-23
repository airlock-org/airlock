from __future__ import annotations

import json
import os
import signal
import socket
import subprocess
import tempfile
import time
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.vm
@pytest.mark.timeout(3600)
@pytest.mark.skipif(
    os.environ.get("AIRLOCK_RUN_GPU_LAB_TESTS") != "1",
    reason="opt-in isolated nested Xen GPU lab; run make vm-gpu-lab-test",
)
def test_gpu_lab_vm():
    """Prove lifecycle and diagnostic transport, explicitly without a GPU."""
    log_dir = Path(os.environ["AIRLOCK_LOG_DIR"]) / "gpu-lab-vm"
    log_dir.mkdir(parents=True, exist_ok=True)
    vm_root = Path(os.environ["AIRLOCK_DATA_ROOT"]) / "gpu-lab-tests"
    vm_root.mkdir(parents=True, exist_ok=True)
    vm_dir = Path(tempfile.mkdtemp(prefix="run-", dir=vm_root))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    env = os.environ | {
        "AIRLOCK_VM_DIR": str(vm_dir),
        "AIRLOCK_VM_SSH_PORT": str(port),
        "AIRLOCK_VM_IMAGE_ATTR": "airlock-vm-gpu-lab-image",
        "AIRLOCK_VM_DISPLAY": "none",
        "AIRLOCK_VM_MEMORY": "12288",
    }
    (log_dir / "environment.json").write_text(
        json.dumps({"vm_dir": str(vm_dir), "ssh_port": port}, indent=2)
    )

    def ssh(command, *, check=True):
        result = subprocess.run(
            [str(ROOT / "lifecycle/misc/tools/scripts/ssh-nested-vm.sh"), command],
            env=env,
            capture_output=True,
            text=True,
            timeout=180,
        )
        with (log_dir / "commands.log").open("a") as output:
            output.write(f"$ {command}\nexit={result.returncode}\n")
            output.write(result.stdout + result.stderr + "\n")
        if check:
            assert result.returncode == 0, result.stdout + result.stderr
        return result

    with (log_dir / "console.log").open("w") as console:
        vm = subprocess.Popen(
            [str(ROOT / "lifecycle/misc/tools/scripts/run-nested-vm.sh")],
            cwd=ROOT,
            env=env,
            stdout=console,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 2400
            while time.monotonic() < deadline:
                assert vm.poll() is None, "GPU lab exited; inspect console.log"
                if (
                    ssh(
                        "systemctl is-active --quiet airlock-setup-config-deferred "
                        "&& sudo qvm-check sys-gpu",
                        check=False,
                    ).returncode
                    == 0
                ):
                    break
                time.sleep(5)
            else:
                pytest.fail("sys-gpu was not created; inspect console.log")
            assert ssh("sudo qvm-check --running sys-gpu", check=False).returncode != 0
            ssh("sudo qvm-start sys-gpu")
            report = ssh("sudo qvm-run --pass-io sys-gpu airlock-gpu-probe")
            (log_dir / "probe.json").write_text(report.stdout)
            data = json.loads(report.stdout)
            assert data["schema"] == "airlock.gpu-probe.v1"
            assert data["hostname"] == "sys-gpu"
            assert data["hardware_api_initialized"] is False
            assert data["cross_vm_graphics_verified"] is False
            assert (
                ssh(
                    "sudo qvm-run --pass-io sys-gpu 'airlock-gpu-probe --require-hardware'",
                    check=False,
                ).returncode
                == 1
            )
            ssh("sudo qvm-shutdown --wait sys-gpu")
            ssh("sudo qvm-start sys-gpu")
            ssh("sudo qvm-run --pass-io sys-gpu 'test $(hostname) = sys-gpu'")
            ssh("sudo qvm-shutdown --wait sys-gpu")
            ssh("sudo qvm-check --running sys-gui")
        finally:
            if vm.poll() is None:
                os.killpg(vm.pid, signal.SIGTERM)
                try:
                    vm.wait(timeout=20)
                except subprocess.TimeoutExpired:
                    os.killpg(vm.pid, signal.SIGKILL)
                    vm.wait(timeout=10)
