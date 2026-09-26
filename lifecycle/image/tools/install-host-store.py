"""Discard the conventional host bootstrap copy after image boot installation."""
import json
import os
from pathlib import Path
import shutil
import sys


def install(root, recovery_paths, recovery_system):
    root = Path(root)
    pool = root / "var/lib/airlock/closure-store"
    recovery = set(Path(recovery_paths).read_text().splitlines())
    admitted = set()
    records = list((pool / "host/boots").glob("*.json"))
    if not records or not recovery:
        raise ValueError("missing host publication or independent recovery closure")
    for path in records:
        record = json.loads(path.read_text())
        manifest = pool / Path(record["manifest"]).relative_to("/var/lib/airlock/closure-store")
        admitted.update(json.loads(manifest.read_text())["store"]["paths"])
    store = root / "nix/store"
    ordinary = {"/nix/store/" + path.name for path in store.iterdir() if not path.name.startswith(".")}
    if not recovery <= ordinary or not ordinary <= admitted:
        raise ValueError("image store differs from published host/recovery inventory")
    from airlock_qubes_storage.host_artifacts import inventory, seal
    # Use the configured system directly: absolute store symlinks inside the
    # mounted image must not be resolved against the build VM's root.
    if recovery_system not in recovery:
        raise ValueError("recovery system is absent from its closure")
    if not inventory(root / "boot", recovery_system):
        raise ValueError("independent recovery boot entry was not installed")
    from airlock_qubes_storage._engine.host import allocated
    staging_bytes = allocated(store)
    pool_bytes = allocated(pool)
    seal(pool, root / "boot")
    destination = root / "var/lib/airlock/recovery-store"
    destination.mkdir(parents=True)
    for path in sorted(store.iterdir()):
        if "/nix/store/" + path.name in recovery:
            path.rename(destination / path.name)
        elif path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink()
    recovery_bytes = allocated(destination)
    receipt = {"schema": "airlock.host-store.install.v1", "recovery_objects": len(recovery),
               "recovery_allocated_bytes": recovery_bytes, "ordinary_host_objects_remaining": 0,
               "pool_allocated_bytes": pool_bytes,
               "installation_pool_plus_store_allocated_bytes": pool_bytes + staging_bytes}
    (root / "var/lib/airlock/host-store-install.json").write_text(json.dumps(receipt, sort_keys=True) + "\n")
    os.sync()


if __name__ == "__main__":
    install(*sys.argv[1:])
