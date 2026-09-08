#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/closure-volume-filtered-device"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-closure-volume-filtered.XXXXXX")"

cleanup() {
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR" "$WORK_DIR/pool/objects/segments"

PYTHONPATH="$ROOT/pkgs/nixome-qubes-storage/src" \
PYTHONDONTWRITEBYTECODE=1 \
python3 - "$WORK_DIR" "$RESULT_DIR/result.json" <<'PY'
import json
import os
import sys
import types
from pathlib import Path


work_dir = Path(sys.argv[1])
result_path = Path(sys.argv[2])


class StoragePoolException(Exception):
    pass


class FakePoolBase:
    def __init__(self, *, name, revisions_to_keep=0, ephemeral_volatile=False, **_kwargs):
        self.name = name
        self.revisions_to_keep = revisions_to_keep
        self.ephemeral_volatile = ephemeral_volatile


class FakeBlockDevice:
    def __init__(self, path, name, script=None, rw=True, domain=None, devtype="disk"):
        self.path = path
        self.name = name
        self.script = script
        self.rw = rw
        self.domain = domain
        self.devtype = devtype


class FakeVolumeBase:
    def __init__(self, **kwargs):
        self.name = kwargs.get("name", "")
        self.pool = kwargs.get("pool")
        self.vid = kwargs.get("vid", "")
        self.rw = bool(kwargs.get("rw", True))
        self.snap_on_start = bool(kwargs.get("snap_on_start", False))
        self.save_on_stop = bool(kwargs.get("save_on_stop", False))
        self.source = kwargs.get("source")
        self.revisions_to_keep = kwargs.get("revisions_to_keep", 0)
        self._size = int(kwargs.get("size", 0))
        self.path = kwargs.get("path")

    @property
    def config(self):
        return {
            "name": self.name,
            "pool": self.pool.name if hasattr(self.pool, "name") else self.pool,
            "vid": self.vid,
            "rw": self.rw,
            "snap_on_start": self.snap_on_start,
            "save_on_stop": self.save_on_stop,
            "source": self.source,
            "revisions_to_keep": self.revisions_to_keep,
            "size": self._size,
        }

    def block_device(self):
        return FakeBlockDevice(self.path, self.name, rw=self.rw)


qubes_module = types.ModuleType("qubes")
exc_module = types.ModuleType("qubes.exc")
storage_module = types.ModuleType("qubes.storage")
exc_module.StoragePoolException = StoragePoolException
storage_module.Pool = FakePoolBase
storage_module.Volume = FakeVolumeBase
storage_module.BlockDevice = FakeBlockDevice
qubes_module.exc = exc_module
qubes_module.storage = storage_module
sys.modules["qubes"] = qubes_module
sys.modules["qubes.exc"] = exc_module
sys.modules["qubes.storage"] = storage_module

from nixome_qubes_storage import engine
from nixome_qubes_storage.qubes import ClosurePool

pool_root = work_dir / "pool"
segment_blob = pool_root / "objects" / "segments" / "000001.blob"
segment_blob.write_bytes(b"segment")
blob_map = pool_root / "domains" / "config-vault" / "generations" / "1" / "store.blob-map.json"
blob_map.parent.mkdir(parents=True, exist_ok=True)
blob_map.write_text(
    json.dumps(
        {
            "schema": "nixome.closure-store.blob-map.v1",
            "segments": [{"id": "000001", "blob": str(segment_blob), "size": segment_blob.stat().st_size}],
            "logical_size": 512,
            "extents": [],
        }
    ),
    encoding="utf-8",
)
manifest = blob_map.parent / "manifest.json"
manifest.write_text("{}", encoding="utf-8")

pool = ClosurePool(name="nixome-closure", root_dir=str(pool_root))
vm = types.SimpleNamespace(name="config-vault")
volume = pool.init_volume(
    vm,
    {
        "name": "store-data",
        "pool": "nixome-closure",
        "vid": "domains/config-vault/generations/1/store-data",
        "size": 512,
        "rw": False,
        "snap_on_start": False,
        "save_on_stop": True,
        "source": None,
        "revisions_to_keep": 0,
        "domain_name": "config-vault",
        "generation": "1",
        "closure_hash": "abc",
        "view_kind": "erofs-blob-map",
        "view_path": "/dev/mapper/stale-nixome-config-vault-1-abc",
        "blob_map_path": str(blob_map),
        "blob_logical_size": 512,
        "manifest_path": str(manifest),
    },
)

original_attach = engine.attach_blob_view
calls = []


def filtered_attach(root_dir, domain, generation):
    calls.append({"root_dir": root_dir, "domain": domain, "generation": generation})
    return {
        "device": "/dev/mapper/nixome-config-vault-1-abc",
        "backing_paths": [str(segment_blob)],
    }


engine.attach_blob_view = filtered_attach
device = volume.block_device()
assert device.path == "/dev/mapper/nixome-config-vault-1-abc", device.path
assert os.path.realpath(device.path) != os.path.realpath(segment_blob)
assert calls == [{"root_dir": str(pool_root), "domain": "config-vault", "generation": "1"}], calls
assert "view_path" not in volume.config, volume.config

volume.path = "/dev/mapper/stale-nixome-config-vault-1-abc"
device = volume.block_device()
assert device.path == "/dev/mapper/nixome-config-vault-1-abc", device.path
assert calls == [
    {"root_dir": str(pool_root), "domain": "config-vault", "generation": "1"},
    {"root_dir": str(pool_root), "domain": "config-vault", "generation": "1"},
], calls


def unsafe_attach(_root_dir, _domain, _generation):
    return {
        "device": str(segment_blob),
        "backing_paths": [str(segment_blob)],
    }


engine.attach_blob_view = unsafe_attach
volume.path = None
try:
    volume.block_device()
except StoragePoolException as exc:
    assert "global Nixome backing storage" in str(exc), str(exc)
else:
    raise AssertionError("global backing storage was exposed to the guest")
finally:
    engine.attach_blob_view = original_attach

result_path.write_text(
    json.dumps(
        {
            "schema": "nixome.closure-volume-filtered-device.v1",
            "status": "passed",
            "guest_device": device.path,
            "backing_paths": [str(segment_blob)],
            "backing_storage_exposed": False,
        },
        sort_keys=True,
    )
    + "\n",
    encoding="utf-8",
)
PY

jq -e '
  .schema == "nixome.closure-volume-filtered-device.v1" and
  .status == "passed" and
  .guest_device == "/dev/mapper/nixome-config-vault-1-abc" and
  .backing_storage_exposed == false
' "$RESULT_DIR/result.json" >/dev/null

printf 'closure volume filtered device test ok\n'
