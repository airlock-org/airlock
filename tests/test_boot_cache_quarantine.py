import importlib.util
import json
from pathlib import Path
import struct
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "boot_cache", ROOT / "lifecycle/misc/tools/scripts/boot-cache-quarantine.py"
)
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


@pytest.fixture
def boot(tmp_path):
    esp, descriptors = tmp_path / "esp", tmp_path / "descriptors"
    descriptors.mkdir()
    for name in ("EFI/airlock", "EFI/nixos", "loader/entries"):
        (esp / name).mkdir(parents=True)
    config = b"kernel=kernel.efi init=/nix/store/system/init\nramdisk=initrd.efi\n"
    binary = bytearray(256 + len(config))
    binary[:2] = b"MZ"
    struct.pack_into("<I", binary, 60, 64)
    binary[64:68] = b"PE\0\0"
    struct.pack_into("<H", binary, 70, 1)
    binary[88:96] = b".config\0"
    struct.pack_into("<II", binary, 104, len(config), 256)
    binary[256:] = config
    files = {"EFI/airlock/xen.efi": bytes(binary),
             "EFI/airlock/kernel.efi": b"kernel",
             "EFI/airlock/initrd.efi": b"initrd",
             "loader/entries/airlock-xen.conf": b"title Airlock\nefi /EFI/airlock/xen.efi\n"}
    for name, data in files.items():
        (esp / name).write_bytes(data)
    record = {"schema": "airlock.host-boot-artifacts.v1", "entries": [{
        "entry": "airlock-xen.conf",
        "files": {name: cache.fingerprint(esp / name) for name in files}}]}
    (descriptors / "system.json").write_text(json.dumps(record))
    (esp / "EFI/nixos/initrd.efi").write_bytes(b"duplicate cache")
    (esp / "EFI/nixos/partial.tmp").write_bytes(b"incomplete copy")
    return esp, descriptors, files


def test_plan_authenticates_boot_and_does_not_mutate(boot):
    esp, descriptors, files = boot
    plan = cache.inspect(esp, descriptors)
    assert plan["sealed_entries"] == 1
    assert plan["bytes"] == len(b"duplicate cacheincomplete copy")
    assert len(list((esp / "EFI/nixos").iterdir())) == 2
    assert all((esp / name).read_bytes() == data for name, data in files.items())


@pytest.mark.parametrize("mutation", ["entry", "payload", "link"])
def test_refuses_unverified_boot_state(boot, mutation):
    esp, descriptors, _ = boot
    if mutation == "entry":
        (esp / "loader/entries/other.conf").write_text("efi /EFI/nixos/native.efi\n")
    elif mutation == "payload":
        (esp / "EFI/airlock/initrd.efi").write_bytes(b"corrupt")
    else:
        path = esp / "EFI/nixos/partial.tmp"
        path.unlink()
        path.symlink_to(esp / "EFI/airlock/initrd.efi")
    with pytest.raises(ValueError):
        cache.inspect(esp, descriptors)
    assert (esp / "EFI/nixos/initrd.efi").exists()


def different_filesystem(monkeypatch, esp):
    real_stat = cache.os.stat
    monkeypatch.setattr(cache.os, "stat", lambda path, *a, **kw:
                        SimpleNamespace(st_dev=-1) if Path(path) == esp
                        else real_stat(path, *a, **kw))


def test_quarantine_preserves_boot_and_recovery_bytes(boot, tmp_path, monkeypatch):
    esp, descriptors, files = boot
    different_filesystem(monkeypatch, esp)
    destination = tmp_path / "quarantine"
    result = cache.apply(esp, descriptors, destination)
    assert result["status"] == "complete"
    assert not list((esp / "EFI/nixos").iterdir())
    assert (destination / "initrd.efi").read_bytes() == b"duplicate cache"
    assert (destination / "partial.tmp").read_bytes() == b"incomplete copy"
    assert json.loads((destination / "receipt.json").read_text())["status"] == "complete"
    assert all((esp / name).read_bytes() == data for name, data in files.items())


def test_failed_copy_reclaims_nothing(boot, tmp_path, monkeypatch):
    esp, descriptors, _ = boot
    different_filesystem(monkeypatch, esp)
    real_copy = cache.shutil.copyfile

    def corrupt(source, target):
        real_copy(source, target)
        target.write_bytes(b"damaged")

    monkeypatch.setattr(cache.shutil, "copyfile", corrupt)
    with pytest.raises(ValueError, match="changed while copying"):
        cache.apply(esp, descriptors, tmp_path / "quarantine")
    assert len(list((esp / "EFI/nixos").iterdir())) == 2


def test_refuses_same_filesystem_quarantine(boot, tmp_path):
    esp, descriptors, _ = boot
    with pytest.raises(ValueError, match="another filesystem"):
        cache.apply(esp, descriptors, tmp_path / "quarantine")
