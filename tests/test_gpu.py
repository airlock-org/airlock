from __future__ import annotations

import importlib.util
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
PROBE = (
    ROOT
    / "airlock-machine/lifecycle/misc/tools/pkgs/airlock-gpu-probe/src/gpu-probe.py"
)


@pytest.fixture
def gpu_probe():
    spec = importlib.util.spec_from_file_location("gpu_probe", PROBE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    ("device_type", "returncode", "expected"),
    [
        ("CPU", 0, 1),
        ("VIRTUAL_GPU", 0, 1),
        ("DISCRETE_GPU", 0, 0),
        ("INTEGRATED_GPU", 0, 0),
        ("DISCRETE_GPU", 1, 1),
        ("DISCRETE_GPU", None, 1),
        ("OTHER", 0, 1),
    ],
)
def test_gpu_probe_requires_hardware(
    gpu_probe, monkeypatch, capsys, device_type, returncode, expected
):
    monkeypatch.setattr("sys.argv", ["airlock-gpu-probe", "--require-hardware"])
    monkeypatch.setattr(
        gpu_probe,
        "run_probe",
        lambda _: {
            "returncode": returncode,
            "stdout": f"deviceType = PHYSICAL_DEVICE_TYPE_{device_type}\n",
        },
    )
    assert gpu_probe.main() == expected
    report = json.loads(capsys.readouterr().out)
    assert report["hardware_api_initialized"] is (expected == 0)
    assert report["rendering_verified"] is False
    assert report["exclusive_ownership_verified"] is False
    assert report["cross_vm_graphics_verified"] is False


def test_gpu_probe_handles_missing_tools(gpu_probe, monkeypatch, capsys):
    monkeypatch.setattr("sys.argv", ["airlock-gpu-probe"])

    def missing(*args, **kwargs):
        raise FileNotFoundError("missing probe tool")

    monkeypatch.setattr(gpu_probe.subprocess, "run", missing)
    assert gpu_probe.main() == 0
    report = json.loads(capsys.readouterr().out)
    assert report["hardware_api_initialized"] is False
    assert report["probes"]["vulkan"]["returncode"] is None


def test_gpu_inventory_excludes_other_pci_devices(gpu_probe, tmp_path):
    gpu = tmp_path / "0000:01:00.0"
    gpu.mkdir()
    (gpu / "class").write_text("0x030000\n")
    (gpu / "vendor").write_text("0x10de\n")
    (gpu / "device").write_text("0x2204\n")
    (gpu / "driver").symlink_to("/sys/bus/pci/drivers/nvidia")
    other = tmp_path / "0000:01:00.1"
    other.mkdir()
    (other / "class").write_text("0x040300\n")
    inventory = gpu_probe.pci_inventory(tmp_path)
    assert len(inventory) == 1
    assert inventory[0]["driver"] == "nvidia"
    assert inventory[0]["address"] == "0000:01:00.0"


@pytest.mark.nix
def test_gpu_lab_is_opt_in_and_keeps_xfce():
    expression = f"""
      let
        flake = builtins.getFlake "{ROOT}";
        normal = flake.nixosConfigurations.vm-gui-config-dev.config;
        lab = flake.nixosConfigurations.vm-gpu-lab.config;
        guest = flake.nixosConfigurations.airlock-domain-sys-gpu.config;
      in {{
        normalHasGpu = normal.airlock.qubes.domains ? sys-gpu;
        gpu = lab.airlock.qubes.topology.domains.sys-gpu;
        normalGui = normal.airlock.qubes.domains.sys-gui;
        labGui = lab.airlock.qubes.domains.sys-gui;
        normalSession = normal.services.displayManager.defaultSession;
        labSession = lab.services.displayManager.defaultSession;
        guiVm = lab.airlock.qubes.prefs.default_guivm;
        hostName = guest.networking.hostName;
        graphics = guest.hardware.graphics.enable;
        packages = map (p: p.pname or p.name) guest.environment.systemPackages;
      }}
    """
    result = subprocess.run(
        ["nix", "eval", "--impure", "--json", "--expr", expression],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    report = json.loads(result.stdout)
    log_dir = Path(os.environ["AIRLOCK_LOG_DIR"]) / "gpu-lab-topology"
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "report.json").write_text(result.stdout)
    assert report["normalHasGpu"] is False
    assert report["normalGui"] == report["labGui"]
    assert report["normalSession"] == report["labSession"] == "sys-gui"
    assert report["guiVm"] == "sys-gui"
    assert report["hostName"] == "sys-gpu"
    assert report["graphics"] is True
    assert "airlock-gpu-probe" in report["packages"]
    gpu = report["gpu"]
    assert gpu["class"] == "StandaloneVM"
    assert gpu["netvm"] is None
    assert gpu["devices"]["pci"] == []
    assert gpu["prefs"]["autostart"] is False
    assert gpu["prefs"]["guivm"] is None
    assert gpu["prefs"]["virt_mode"] == "hvm"
    assert gpu["prefs"]["memory"] == gpu["prefs"]["maxmem"]


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
