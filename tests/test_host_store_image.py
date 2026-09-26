"""Image bootstrap removal must retain only the independent recovery closure."""
import importlib.util
import json
import fcntl
import stat
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src"))
spec = importlib.util.spec_from_file_location("install_host_store", ROOT / "lifecycle/image/tools/install-host-store.py")
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)
spec = importlib.util.spec_from_file_location("prepare_host_store", ROOT / "lifecycle/image/tools/prepare-host-store.py")
preparer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preparer)


def test_seed_copy_reopens_locks_preserving_sealed_data(tmp_path):
    seed = tmp_path / "seed"
    pool = seed / "pool"
    (pool / "objects").mkdir(parents=True)
    names = ("admission.lock", "objects/segments.lock", "objects/sealed.blob", "index.erofs")
    for name in names:
        path = pool / name
        path.write_bytes(b"seed")
        path.chmod(0o444)
    (pool / "objects").chmod(0o555)
    pool.chmod(0o555)
    copied = tmp_path / "copy"
    preparer.copy_seed(seed, copied)
    for name in names[:2]:
        assert stat.S_IMODE((copied / name).stat().st_mode) == 0o600
        with (copied / name).open("a+") as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    for name in names[2:]:
        assert stat.S_IMODE((copied / name).stat().st_mode) == 0o444
        assert (copied / name).read_bytes() == b"seed"
    for name in names:
        assert stat.S_IMODE((pool / name).stat().st_mode) == 0o444


def image(tmp_path):
    paths = ["/nix/store/" + digit * 32 + name for digit, name in [("0", "-host"), ("1", "-recovery")]]
    for path in paths:
        target = tmp_path / path.lstrip("/")
        target.mkdir(parents=True)
        (target / "payload").write_text(path)
    pool = tmp_path / "var/lib/airlock/closure-store"
    (pool / "host/boots").mkdir(parents=True)
    (pool / "generation.json").write_text(json.dumps({"store": {"paths": paths}}))
    (pool / "host/boots/host.json").write_text(json.dumps({"manifest": "/var/lib/airlock/closure-store/generation.json", "system": paths[0], "generation": 1}))
    entries = tmp_path / "boot/loader/entries"
    entries.mkdir(parents=True)
    (entries / "xen-generation-1-specialisation-usb-text-recovery.conf").write_text("recovery")
    recovery = tmp_path / "recovery-paths"
    recovery.write_text(paths[1] + "\n")
    return recovery, paths


def test_seed_removal_keeps_recovery_without_second_host_copy(tmp_path, monkeypatch):
    recovery, paths = image(tmp_path)
    monkeypatch.setattr(installer.os, "sync", lambda: None)
    installer.install(tmp_path, recovery)
    assert not list((tmp_path / "nix/store").iterdir())
    retained = tmp_path / "var/lib/airlock/recovery-store"
    assert [p.name for p in retained.iterdir()] == [Path(paths[1]).name]


def test_seed_removal_refuses_unpublished_objects(tmp_path):
    recovery, paths = image(tmp_path)
    extra = tmp_path / "nix/store/unpublished"
    extra.mkdir()
    with pytest.raises(ValueError, match="inventory"):
        installer.install(tmp_path, recovery)
    assert extra.exists()
    assert (tmp_path / paths[0].lstrip("/")).exists()
