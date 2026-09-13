"""Build-time pool preparation and checked adoption without source packages."""

import shutil
import sys
from pathlib import Path

from .core import (
    atomic_json,
    closure_hash,
    fail,
    hash_file,
    read_json,
    read_store_paths,
)
from .generations import admit_generation
from .segments import seal_segment_locked, segment_ids
from .state import generation_is_committed, generation_manifest_path

SCHEMA_PREPARED = "airlock.closure-store.prepared.v1"


def relocate(value, old, new):
    if isinstance(value, str):
        return (
            new + value[len(old) :]
            if value == old or value.startswith(old + "/")
            else value
        )
    if isinstance(value, list):
        return [relocate(item, old, new) for item in value]
    if isinstance(value, dict):
        return {key: relocate(item, old, new) for key, item in value.items()}
    return value


def prepare_pool(root, descriptors, specs, runtime_root):
    """Only called in a build sandbox; outputs must be fresh directories."""
    from .cli import build_parser

    root = Path(root).absolute()
    descriptors = Path(descriptors).absolute()
    if root.exists() or descriptors.exists():
        fail("prepared store outputs must not already exist")
    root.mkdir(parents=True)
    descriptors.mkdir(parents=True)
    prepared = []
    for spec in specs:
        name = spec["name"]
        if not name or "/" in name or name in (".", ".."):
            fail("invalid prepared closure name")
        target = descriptors / name
        target.mkdir()
        for filename in ("store-paths", "manifest.json"):
            shutil.copyfile(Path(spec["closure_source"]) / filename, target / filename)
        args = build_parser().parse_args(
            [
                "admit",
                "--pool-root",
                str(root),
                "--domain",
                spec["domain"],
                "--system",
                spec["system"],
                "--closure-source",
                str(target),
                "--root-source",
                spec["root_source"],
                "--kernel-name",
                spec["kernel_name"],
                "--approval-source",
                str(target),
                "--approved-by",
                "image-build",
                "--verify-mode",
                "tree",
                "--activate",
                "false",
            ]
        )
        # Build-time sources use the sandbox's normal store, never the host's
        # AIRLOCK_NIX_ROOT setting (which may have been inherited by a caller).
        args.physical_store_root = spec.get("physical_store_root", "/nix/store")
        print(f"Preparing guest store: {name}", flush=True)
        admission = admit_generation(args)
        prepared.append((target, admission))

    # No runtime admission may append to the checksummed seed segments.
    for segment_id in segment_ids(root):
        seal_segment_locked(root, segment_id, "image-build", "image-seed")
    current = root / "objects/current-segment"
    if current.exists():
        current.unlink()

    # All embedded pool paths must name the installed pool, not a build output.
    # Relative generation symlinks already survive this relocation unchanged.
    for path in root.rglob("*.json"):
        if not path.is_symlink():
            atomic_json(path, relocate(read_json(path), str(root), runtime_root))

    fingerprints = {}
    for target, admission in prepared:
        manifest_path = Path(admission["manifest"])
        manifest = read_json(manifest_path)
        store = manifest["store"]
        relative_manifest = str(manifest_path.relative_to(root))
        files = {relative_manifest}
        for key in (
            "canonical_metadata_image",
            "canonical_blob_map",
            "canonical_object_refs",
        ):
            files.add(str(Path(store[key]).relative_to(runtime_root)))
        files.add(f"transactions/{manifest['transaction']}.json")
        blob_map = read_json(
            root / Path(store["canonical_blob_map"]).relative_to(runtime_root)
        )
        for segment in blob_map["segments"]:
            files.add(f"objects/segments/{segment['id']}.blob")
            files.add(f"objects/segments/{segment['id']}.json")
        for entry in blob_map["objects"] + [".tar-eof"]:
            files.add(f"objects/blob-metadata/{entry}.json")
            if entry != ".tar-eof":
                files.add(f"objects/metadata/{entry}.json")
        for relative in sorted(files):
            if relative not in fingerprints:
                path = root / relative
                fingerprints[relative] = {
                    "size": path.stat().st_size,
                    "sha256": hash_file(path),
                }
        atomic_json(
            target / "prepared.json",
            {
                "schema": SCHEMA_PREPARED,
                "pool_root": runtime_root,
                "domain": manifest["domain"],
                "generation": manifest["generation"],
                "system": manifest["system"],
                "root_source": manifest["root"]["source"],
                "kernel_name": manifest["kernel"]["name"],
                "closure_hash": store["closure_hash"],
                "files": {
                    relative: fingerprints[relative] for relative in sorted(files)
                },
            },
        )


def verify_prepared(args):
    """Verify the selected seed once before Qubes records its generation.

    A present but invalid descriptor fails closed; never silently repack from
    sources the prepared image intentionally omits. This descriptor is part of
    the administrator-selected store artifact, not supplied by a guest.
    """
    descriptor_path = Path(args.artifact) / "prepared.json"
    if not descriptor_path.exists():
        return None
    descriptor = read_json(descriptor_path)
    if descriptor.get("schema") != SCHEMA_PREPARED:
        fail("unsupported prepared store descriptor")
    root = Path(args.pool_root).absolute()
    for field, expected in {
        "pool_root": str(root),
        "domain": args.domain,
        "system": args.system,
        "root_source": args.root,
        "kernel_name": args.kernel,
    }.items():
        if descriptor.get(field) != expected:
            fail(f"prepared store {field} mismatch")
    generation = descriptor.get("generation")
    if type(generation) is not int or generation < 1:
        fail("invalid prepared store generation")
    paths = read_store_paths(args.artifact, args.system)
    expected_hash = closure_hash(paths, args.system, args.root, args.kernel)
    if descriptor.get("closure_hash") != expected_hash:
        fail("prepared store closure mismatch")
    files = descriptor.get("files", {})
    relative_manifest = f"domains/{args.domain}/generations/{generation}/manifest.json"
    if relative_manifest not in files:
        fail("prepared store manifest checksum missing")
    print(
        f"Checking prepared store for {args.domain} (sequential integrity read; no package copying)",
        file=sys.stderr,
        flush=True,
    )
    for relative, expected in files.items():
        path = root / relative
        if Path(relative).is_absolute() or ".." in Path(relative).parts:
            fail("prepared store checksum path escapes pool")
        if not path.resolve().is_relative_to(root.resolve()):
            fail("prepared store checksum symlink escapes pool")
        if (
            path.stat().st_size != expected["size"]
            or hash_file(path) != expected["sha256"]
        ):
            fail(f"prepared store integrity check failed: {relative}")
    manifest = read_json(generation_manifest_path(root, args.domain, generation))
    store = manifest.get("store", {})
    if not generation_is_committed(root, manifest):
        fail("prepared store generation is not committed")
    if (
        manifest.get("domain") != args.domain
        or manifest.get("system") != args.system
        or store.get("closure_hash") != expected_hash
        or store.get("paths") != paths
        or store.get("source") != args.source
    ):
        fail("prepared store generation identity mismatch")
    for field, expected in {
        "device": args.device,
        "blob_device": args.blob_device,
        "mountpoint": args.mountpoint,
        "view_backend": args.view_backend,
    }.items():
        if store.get(field) != expected:
            fail(f"prepared store {field} mismatch")
    return generation


if __name__ == "__main__":
    import sys

    prepare_pool(sys.argv[1], sys.argv[2], read_json(sys.argv[3]), sys.argv[4])
