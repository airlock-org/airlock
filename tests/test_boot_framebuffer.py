"""Firmware-only handoff; all sysfs writes use disposable pytest fixtures."""

import importlib.util
from pathlib import Path

import pytest

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "lifecycle/misc/tools/scripts/boot-framebuffer.py"
)
spec = importlib.util.spec_from_file_location("boot_framebuffer", SOURCE)
fb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fb)
BDF = "0000:09:03.0"


@pytest.fixture(params=["platform-root", "pci-parent"])
def layout(request):
    return request.param


@pytest.fixture(params=fb.DRIVERS)
def tree(tmp_path, request, layout):
    gpu = tmp_path / "devices/pci0000:00/0000:00:08.1" / BDF
    gpu.mkdir(parents=True)
    (gpu / "class").write_text("0x030000\n")
    pci = tmp_path / "bus/pci/devices"
    pci.mkdir(parents=True)
    (pci / BDF).symlink_to(gpu)
    name = request.param
    driver = tmp_path / "bus/platform/drivers" / name
    driver.mkdir(parents=True)
    (driver / "unbind").touch()
    parent = tmp_path / "devices/platform" if layout == "platform-root" else gpu
    device = parent / (name + ".0")
    device.mkdir(parents=True)
    (device / "driver_override").touch()
    (device / "driver").symlink_to(driver)
    (driver / device.name).symlink_to(device)
    platform = tmp_path / "bus/platform/devices"
    platform.mkdir(parents=True)
    (platform / device.name).symlink_to(device)
    (device / "subsystem").symlink_to(platform.parent)
    graphic = tmp_path / "class/graphics/fb0"
    graphic.mkdir(parents=True)
    (graphic / "device").symlink_to(device)
    console = tmp_path / "class/vtconsole/vtcon7"
    console.mkdir(parents=True)
    (console / "name").write_text("(M) frame buffer device\n")
    (console / "bind").write_text("1\n")
    writes = []

    def write(path, value):
        writes.append((path, value))
        path.write_text(value)
        if path == driver / "unbind":
            (device / "driver").unlink()
            (driver / device.name).unlink()
            (graphic / "device").unlink()
            graphic.rmdir()

    return tmp_path, driver, device, console, writes, write


def test_release_and_repeat(tree):
    root, driver, device, console, writes, write = tree
    result = fb.release(root, BDF, write)
    assert result["status"] == "released"
    assert writes == [
        (device / "driver_override", "airlock-released\n"),
        (console / "bind", "0\n"),
        (driver / "unbind", device.name + "\n"),
    ]
    assert fb.release(root, BDF, write)["status"] == "no-framebuffer"
    assert len(writes) == 3


def test_unbound_firmware_device_cannot_probe_later(tree):
    root, driver, device, _, writes, write = tree
    fb.release(root, BDF, write)
    (device / "driver_override").write_text("\n")
    writes.clear()
    assert fb.release(root, BDF, write)["status"] == "no-framebuffer"
    assert writes == [(device / "driver_override", "airlock-released\n")]


@pytest.mark.parametrize(
    "fault",
    [
        "other-gpu",
        "unknown-fb",
        "missing-override",
        "wrong-gpu",
        "wrong-bus",
        "missing-bus",
        "wrong-driver",
        "wrong-registration",
    ],
)
def test_validation_precedes_all_writes(tree, fault):
    root, driver, device, _, writes, write = tree
    bdf = BDF
    if fault == "other-gpu":
        other = root / "bus/pci/devices/0000:01:00.0"
        other.mkdir()
        (other / "class").write_text("0x030200\n")
    elif fault == "unknown-fb":
        graphic = root / "class/graphics/fb1"
        graphic.mkdir()
        (graphic / "device").symlink_to(root / "bus/pci/devices" / BDF)
    elif fault == "missing-override":
        (device / "driver_override").unlink()
    elif fault in ("wrong-bus", "missing-bus"):
        (device / "subsystem").unlink()
        if fault == "wrong-bus":
            (device / "subsystem").symlink_to(root / "bus/pci")
    elif fault == "wrong-driver":
        (device / "driver").unlink()
        other = root / "bus/platform/drivers/not-firmware"
        other.mkdir()
        (device / "driver").symlink_to(other)
    elif fault == "wrong-registration":
        (driver / device.name).unlink()
        (driver / device.name).symlink_to(root / "devices")
    else:
        bdf = "0000:01:00.0"
    with pytest.raises((ValueError, OSError)):
        fb.release(root, bdf, write)
    assert writes == []


@pytest.mark.parametrize("stuck", ["console", "driver"])
def test_handoff_failure_blocks_start(tree, stuck):
    root, driver, _, console, writes, write = tree

    def blocked(path, value):
        if path == (console / "bind" if stuck == "console" else driver / "unbind"):
            return
        write(path, value)

    with pytest.raises(ValueError, match="did not detach|still bound"):
        fb.release(root, BDF, blocked)
    assert writes[0][0].name == "driver_override"
