"""Provisioning safety contracts; no actual disks, mounts or credentials used."""

import importlib.util
from pathlib import Path

import pytest


@pytest.fixture
def usb():
    source = Path(__file__).resolve().parents[1] / "lifecycle/image/tools/usb-dev.py"
    spec = importlib.util.spec_from_file_location("usb_dev", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.mark.parametrize("device", ["/dev/sda", "/dev/nvme0n1", "/", "/dev/disk/by-id/ata-disk"])
def test_writer_requires_explicit_usb_identity(usb, device):
    with pytest.raises(ValueError, match="whole-disk"):
        usb.usb_disk(device)


def test_mutable_destination_rejects_symlinks(usb, tmp_path):
    (tmp_path / "home").symlink_to("/tmp")
    with pytest.raises(ValueError, match="symlink"):
        usb.beneath(tmp_path, "home/airlock/.codex")


def test_private_file_never_overwrites(usb, tmp_path):
    target = tmp_path / "existing"
    target.write_text("keep")
    with pytest.raises(FileExistsError):
        usb.private_file(target, b"replacement")
    assert target.read_text() == "keep"


def test_auth_cache_must_be_a_json_object(usb, tmp_path):
    valid = tmp_path / "auth.json"
    valid.write_text('{"tokens": {}}')
    assert usb.read_auth_cache(valid) == b'{"tokens": {}}'
    invalid = tmp_path / "invalid.json"
    invalid.write_text("[]")
    with pytest.raises(ValueError, match="Invalid auth JSON"):
        usb.read_auth_cache(invalid)


def test_auth_requires_mounted_vault(usb, tmp_path):
    with pytest.raises(ValueError, match="vault is locked"):
        usb.require_vault(tmp_path)


def test_flatten_includes_descendants(usb):
    nodes = [{"name": "disk", "children": [{"name": "partition", "children": [{"name": "crypt"}]}]}]
    assert [node["name"] for node in usb.flatten(nodes)] == ["disk", "partition", "crypt"]


@pytest.fixture
def amd():
    source = Path(__file__).resolve().parents[1] / "lifecycle/image/tools/amd-preflight.py"
    spec = importlib.util.spec_from_file_location("amd_preflight", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_amd_missing_evidence_is_a_failure(amd, tmp_path):
    report = amd.inspect(tmp_path / "sys", tmp_path / "proc")
    assert not report["assignment_gate_passed"]
    assert "not-xen-dom0" in report["failures"]
    assert "missing-or-shared-iommu-group" in report["failures"]
    assert not report["graphics_verified"]


def test_amd_bus_reset_is_not_accepted(amd, tmp_path):
    device = tmp_path / "sys/bus/pci/devices/0000:04:00.0"
    device.mkdir(parents=True)
    (device / "vendor").write_text("0x1002\n")
    (device / "device").write_text("0x1681\n")
    (device / "reset_method").write_text("bus\n")
    report = amd.inspect(tmp_path / "sys", tmp_path / "proc")
    assert "function-level-reset-not-selected-review-required" in report["failures"]
    assert not report["assignment_gate_passed"]
