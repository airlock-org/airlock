"""Complete an image's guest seed with host views, without a closure cycle."""
import json
from pathlib import Path
import sys

from airlock_qubes_storage._engine import core, host, prepared, segments
from airlock_qubes_storage._engine.core import reproducible_image_build


def copy_seed(seed, root):
    """Reopen coordination files in a private copy of an immutable Nix seed."""
    host.command("cp", "-a", "--sparse=always", str(seed / "pool"), str(root))
    root.chmod(0o755)
    for path in root.rglob("*"):
        if path.is_dir() and not path.is_symlink():
            path.chmod(path.stat().st_mode | 0o200)
    for path in (root / "admission.lock", segments.segment_lock_path(root)):
        if path.is_symlink():
            raise ValueError(f"seed coordination lock must not be a symlink: {path}")
        if path.exists():
            path.chmod(0o600)


@reproducible_image_build()
def prepare(seed, output, registration, system, choices):
    seed, output = Path(seed), Path(output)
    root = output / "pool"
    output.mkdir(parents=True)
    copy_seed(seed, root)
    # Existing indices are immutable. Seed segments stay sealed, so new host
    # admission cannot invalidate the descriptors referenced by host closures.
    for path in root.rglob("*.json"):
        if not path.is_symlink():
            core.atomic_json(path, prepared.relocate(core.read_json(path), host.POOL, str(root)))
    text = Path(registration).read_text()
    systems = list(dict.fromkeys([system] + list(choices.values())))
    host.publish(root, system, text, systems, incremental=False)
    for role in ("sys-ctrl", "sys-agent"):
        if (root / f"builders/{role}.json").exists():
            host.refresh_builder(root, role, text, incremental=False)
    core.atomic_json(root / "host/aliases.json", choices)
    for identifier in segments.segment_ids(root):
        segments.seal_segment_locked(root, identifier, "image-build", "image-seed")
    (root / "objects/current-segment").unlink(missing_ok=True)
    for path in root.rglob("*.json"):
        if not path.is_symlink():
            core.atomic_json(path, prepared.relocate(core.read_json(path), str(root), host.POOL))
    # Relocation changes the serialized blob map, so checksum its final bytes.
    for path in (root / "host/boots").glob("*.json"):
        record = core.read_json(path)
        blob = root / Path(record["blob_map"]).relative_to(host.POOL)
        record["blob_map_sha256"] = core.hash_file(blob)
        core.atomic_json(path, record)
    core.atomic_json(output / "pool-fingerprints.json", {
        str(path.relative_to(root)): {"size": path.stat().st_size, "sha256": core.hash_file(path)}
        for path in sorted(root.rglob("*"))
        if path.is_file() and not path.is_symlink() and path.name != "root.img"
    })
    core.atomic_json(output / "receipt.json", {
        "schema": "airlock.unified-seed.v1", "systems": systems,
        "pool_allocated_bytes": host.allocated(root),
        "registration_sha256": core.hash_file(registration),
    })


def verify(seed):
    from airlock_qubes_storage._engine.builder import builder_volume_configs
    from airlock_qubes_storage._engine.root_images import verify_generation_root
    seed = Path(seed)
    root = seed / "pool"
    prepared.verify_files(root, core.read_json(seed / "pool-fingerprints.json"))
    for path in (root / "domains").glob("*/generations/*/manifest.json"):
        verify_generation_root(root, core.read_json(path))
    for path in (root / "host/boots").glob("*.json"):
        host.validate_boot(root, core.read_json(path)["system"], logical_root=host.POOL)
    for role in ("sys-agent", "sys-ctrl"):
        builder_volume_configs(host.POOL, "airlock-closure", domain=role, physical_root=root)
    print(json.dumps({"schema": "airlock.unified-seed-check.v1", "status": "passed"}))


if __name__ == "__main__":
    if sys.argv[1] == "--check":
        verify(sys.argv[2])
    else:
        prepare(*sys.argv[1:5], json.loads(Path(sys.argv[5]).read_text()))
