import json
import os
import subprocess
import uuid
from contextlib import suppress
from pathlib import Path


from .core import (
    EngineError,
    SCHEMA_BLOB_MAP,
    SCHEMA_RUNTIME_VIEW_REF,
    TAR_BLOCK_SIZE,
    VIEW_BACKEND_EROFS_OBJECTMAP,
    atomic_json,
    atomic_write,
    blob_dm_name,
    dm_exists,
    dm_path,
    fail,
    fsync_dir,
    maybe_failpoint,
    read_json,
    remove_path,
    require_tool,
    run_checked,
    safe_domain_name,
    store_entry,
    utc_now,
    write_blob_map,
    write_object_refs,
)
from .objects import ensure_tar_eof_extent
from .pool import ensure_pool
from .segments import (
    object_blob_meta_path,
    object_extent_from_meta,
    read_segment_meta,
    validate_segment_id,
)
from .state import generation_is_committed, generation_manifest_path


def segment_path_map_from_blob_map(blob_map):
    paths = {}
    for segment in blob_map.get("segments", []):
        segment_id = validate_segment_id(segment["id"])
        paths[segment_id] = segment["blob"]
    for extent in blob_map.get("extents", []):
        if "segment_id" not in extent:
            fail("blob map uses obsolete single-blob extent metadata")
        segment_id = validate_segment_id(extent["segment_id"])
        if "segment_blob" in extent:
            paths.setdefault(segment_id, extent["segment_blob"])
    if not paths and blob_map.get("logical_size", 0):
        fail("blob map does not define any segment backings")
    return paths


def blob_map_physical_references(blob_map, source):
    references = {}
    for extent in blob_map.get("extents", []):
        segment_id = validate_segment_id(extent["segment_id"])
        entry = extent.get("object") or "<unknown>"
        references.setdefault(segment_id, []).append(
            {
                "segment_id": segment_id,
                "source": source,
                "entry": entry,
                "offset": int(extent["segment_offset"]),
                "length": int(extent["length"]),
                "logical_offset": int(extent["logical_offset"]),
            }
        )
    return references


def merge_segment_reference_details(target, source):
    for segment_id, refs in source.items():
        target.setdefault(validate_segment_id(segment_id), []).extend(refs)


def segment_reference_entries(reference_details):
    return {
        segment_id: {ref.get("entry", "<unknown>") for ref in refs}
        for segment_id, refs in reference_details.items()
    }


def reconstruct_blob_view(blob_map, output_path):
    output_path = Path(output_path)
    segment_paths = segment_path_map_from_blob_map(blob_map)
    handles = {}
    try:
        with output_path.open("wb") as target:
            for extent in blob_map["extents"]:
                segment_id = validate_segment_id(extent["segment_id"])
                if segment_id not in handles:
                    handles[segment_id] = Path(segment_paths[segment_id]).open("rb")
                source = handles[segment_id]
                source.seek(int(extent["segment_offset"]))
                remaining = int(extent["length"])
                target.seek(int(extent["logical_offset"]))
                while remaining:
                    data = source.read(min(1024 * 1024, remaining))
                    if not data:
                        fail("segment extent is truncated while reconstructing view")
                    remaining -= len(data)
                    target.write(data)
            target.truncate(int(blob_map["logical_size"]))
    finally:
        for handle in handles.values():
            handle.close()
    os.chmod(output_path, 0o400)


def create_erofs_tar_index_image(root, blob_map, metadata_path, closure_id, txid):
    from .core import image_building

    mkfs_erofs = require_tool("mkfs.erofs", "AIRLOCK_MKFS_EROFS")
    root = Path(root)
    metadata_path = Path(metadata_path)
    if metadata_path.exists():
        metadata_path.unlink()

    source_mode = os.environ.get("AIRLOCK_CLOSURE_STORE_INDEX_SOURCE", "auto")
    dm_name = f"airlock-build-{closure_id[:16]}-{txid[:8]}"
    source_path = None
    use_dm = source_mode == "dm" or (source_mode == "auto" and os.geteuid() == 0)

    if use_dm:
        try:
            source_path = create_dm_view_for_blob_map(root, dm_name, blob_map)["device"]
        except EngineError:
            if source_mode == "dm":
                raise
            source_path = None

    if source_path is None:
        tmp_source = root / "tmp" / f"erofs-index-source-{closure_id}-{txid}.tar"
        reconstruct_blob_view(blob_map, tmp_source)
        source_path = str(tmp_source)
    else:
        tmp_source = None

    try:
        seed_options = (
            ["-T", "1", "-U", str(uuid.UUID(closure_id[:32]))]
            if image_building()
            else []
        )
        run_checked(
            [
                mkfs_erofs,
                "--quiet",
                "--tar=i",
                *seed_options,
                str(metadata_path),
                source_path,
            ]
        )
    finally:
        if use_dm and source_path and source_path.startswith("/dev/mapper/"):
            detach_dm_view(dm_name)
        if tmp_source is not None:
            remove_path(tmp_source)

    os.chmod(metadata_path, 0o444)


def ensure_loop_for_blob(blob_path):
    losetup = require_tool("losetup", "AIRLOCK_LOSETUP")
    blob_path = os.path.abspath(blob_path)
    result = subprocess.run(
        [losetup, "-j", blob_path, "--output", "NAME", "--noheadings"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        loops = [line.strip() for line in result.stdout.splitlines() if line.strip()]
        if loops:
            run_checked([losetup, "-c", loops[0]])
            return loops[0]

    created = run_checked([losetup, "--find", "--show", "--read-only", blob_path])
    loop = created.stdout.strip()
    if not loop:
        fail(f"losetup did not report a loop device for {blob_path}")
    return loop


def dm_table_for_extents(blob_map, backing_devices):
    lines = []
    expected_logical = 0
    for extent in blob_map.get("extents", []):
        logical = int(extent["logical_offset"])
        segment_offset = int(extent["segment_offset"])
        length = int(extent["length"])
        if logical != expected_logical:
            fail(
                f"non-contiguous blob map at logical offset {logical}, expected {expected_logical}"
            )
        if logical % 512 != 0 or segment_offset % 512 != 0 or length % 512 != 0:
            fail("blob map extent is not 512-byte aligned")
        segment_id = validate_segment_id(extent["segment_id"])
        backing_device = backing_devices.get(segment_id)
        if not backing_device:
            fail(f"missing backing device for segment {segment_id}")
        lines.append(
            f"{logical // 512} {length // 512} linear {backing_device} {segment_offset // 512}"
        )
        expected_logical += length
    logical_size = int(blob_map["logical_size"])
    if expected_logical != logical_size:
        fail(f"blob map size mismatch: {expected_logical} != {logical_size}")
    return "\n".join(lines) + "\n"


def create_dm_view_for_blob_map(root, name, blob_map):
    segment_paths = segment_path_map_from_blob_map(blob_map)
    backing_devices = {}
    segment_results = []
    for segment_id, path in sorted(
        segment_paths.items(), key=lambda item: int(item[0])
    ):
        if not os.path.exists(path):
            fail(f"segment blob is missing: {path}")
        loop = ensure_loop_for_blob(path)
        backing_devices[segment_id] = loop
        segment_results.append({"id": segment_id, "blob": path, "loop": loop})
    dmsetup = require_tool("dmsetup", "AIRLOCK_DMSETUP")
    if not dm_exists(name):
        table = dm_table_for_extents(blob_map, backing_devices)
        table_path = Path(root) / "runtime" / f"{name}.table"
        atomic_write(table_path, table, mode=0o400)
        maybe_failpoint("mapper-before-create")
        run_checked([dmsetup, "-r", "create", name], input_text=table)
        maybe_failpoint("mapper-after-create")
    run_checked([dmsetup, "mknodes", name])
    return {
        "loops": [segment["loop"] for segment in segment_results],
        "segments": segment_results,
        "backing_paths": [segment["blob"] for segment in segment_results],
        "dm_name": name,
        "device": dm_path(name),
    }


def runtime_view_ref_path(root, name):
    return Path(root) / "runtime" / f"{name}.json"


def write_runtime_view_ref(
    root, name, domain, generation, blob_map_path, blob_map, status
):
    root = Path(root)
    path = runtime_view_ref_path(root, name)
    atomic_json(
        path,
        {
            "schema": SCHEMA_RUNTIME_VIEW_REF,
            "status": status,
            "domain": safe_domain_name(domain),
            "generation": int(generation),
            "dm_name": name,
            "device": dm_path(name),
            "blob_map": str(blob_map_path),
            "closure_hash": blob_map.get("closure_hash", ""),
            "segments": blob_map.get("segments", []),
            "physical_references": [
                ref
                for refs in blob_map_physical_references(
                    blob_map, f"runtime:{name}"
                ).values()
                for ref in refs
            ],
            "updated_at": utc_now(),
        },
    )
    return path


def remove_runtime_view_ref(root, name):
    path = runtime_view_ref_path(root, name)
    if path.exists() or path.is_symlink():
        remove_path(path)
        return True
    return False


def iter_runtime_view_refs(root):
    runtime_dir = Path(root) / "runtime"
    if not runtime_dir.exists():
        return
    for ref_path in sorted(runtime_dir.glob("airlock-*.json")):
        try:
            ref = read_json(ref_path)
        except (OSError, json.JSONDecodeError):
            continue
        if ref.get("schema") != SCHEMA_RUNTIME_VIEW_REF:
            continue
        yield ref_path, ref


def runtime_segment_references(root):
    references = {}
    stale = []
    for ref_path, ref in iter_runtime_view_refs(root):
        dm_name = ref.get("dm_name")
        if not dm_name:
            stale.append(str(ref_path))
            continue
        dm_present = True
        with suppress(EngineError):
            dm_present = dm_exists(dm_name)
        if not dm_present and ref.get("status") != "preparing":
            stale.append(str(ref_path))
            continue
        for physical_ref in ref.get("physical_references", []):
            segment_id = validate_segment_id(physical_ref["segment_id"])
            references.setdefault(segment_id, []).append(
                {
                    "source": f"runtime:{dm_name}",
                    "entry": physical_ref.get("entry", "<unknown>"),
                    "offset": int(physical_ref.get("offset", 0)),
                    "length": int(physical_ref.get("length", 0)),
                    "logical_offset": int(physical_ref.get("logical_offset", 0)),
                }
            )
    return references, stale


def detach_dm_view(name, deferred=False):
    if not dm_exists(name):
        return False
    dmsetup = require_tool("dmsetup", "AIRLOCK_DMSETUP")
    command = [dmsetup, "remove"]
    if deferred:
        command.append("--deferred")
    command.append(name)
    result = subprocess.run(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        fail(f"cannot remove device-mapper view {name}: {result.stderr.strip()}")
    return True


def attach_blob_view(root, domain, generation):
    root = ensure_pool(root)
    domain = safe_domain_name(domain)
    generation = int(generation)
    manifest = read_json(generation_manifest_path(root, domain, generation))
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    store = manifest.get("store", {})
    if store.get("view_backend") != VIEW_BACKEND_EROFS_OBJECTMAP:
        fail(
            f"generation is not an erofs-objectmap view: {domain} generation {generation}"
        )

    blob_map_path = store.get("canonical_blob_map") or store.get("blob_map")
    if not blob_map_path:
        fail(f"generation is missing blob map: {domain} generation {generation}")
    blob_map = read_json(blob_map_path)

    name = blob_dm_name(domain, generation, store.get("closure_hash", ""))
    path = dm_path(name)
    write_runtime_view_ref(
        root, name, domain, generation, blob_map_path, blob_map, "preparing"
    )
    dm_view = create_dm_view_for_blob_map(root, name, blob_map)
    write_runtime_view_ref(
        root, name, domain, generation, blob_map_path, blob_map, "active"
    )

    return {
        "schema": "airlock.closure-store.attach.v1",
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "view_backend": store.get("view_backend"),
        "blob_map": blob_map_path,
        "segments": dm_view["segments"],
        "backing_paths": dm_view["backing_paths"],
        "loops": dm_view["loops"],
        "dm_name": dm_view["dm_name"],
        "device": path,
        "read_only": True,
    }


def detach_blob_view(root, domain, generation):
    root = ensure_pool(root)
    domain = safe_domain_name(domain)
    generation = int(generation)
    manifest = read_json(generation_manifest_path(root, domain, generation))
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    store = manifest.get("store", {})
    name = blob_dm_name(domain, generation, store.get("closure_hash", ""))
    removed = detach_dm_view(name, deferred=True)
    runtime_ref_removed = False
    if removed:
        if dm_exists(name):
            blob_map_path = store.get("canonical_blob_map") or store.get("blob_map")
            if blob_map_path and Path(blob_map_path).exists():
                write_runtime_view_ref(
                    root,
                    name,
                    domain,
                    generation,
                    blob_map_path,
                    read_json(blob_map_path),
                    "retiring",
                )
        else:
            runtime_ref_removed = remove_runtime_view_ref(root, name)
    else:
        runtime_ref_removed = remove_runtime_view_ref(root, name)
    return {
        "schema": "airlock.closure-store.detach.v1",
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "dm_name": name,
        "device": dm_path(name),
        "removed": removed,
        "runtime_ref_removed": runtime_ref_removed,
    }


def materialize_erofs_objectmap_view(
    root, generation_dir, paths, closure_id, txid, target_bytes
):
    root = Path(root)
    view_root = root / "views/by-closure" / closure_id
    metadata_image = view_root / "store.erofs"
    blob_map = view_root / "store.blob-map.json"
    object_refs = view_root / "object-refs"
    view_manifest = view_root / "manifest.json"
    view_root.mkdir(parents=True, exist_ok=True)

    view_created = False
    reusable_blob_map = False
    if blob_map.exists():
        with suppress(OSError, json.JSONDecodeError, EngineError):
            existing_blob_map = read_json(blob_map)
            reusable_blob_map = (
                "segments" in existing_blob_map
                and "central_blob" not in existing_blob_map
                and all(
                    "segment_id" in extent
                    for extent in existing_blob_map.get("extents", [])
                )
            )
    if (
        not metadata_image.exists()
        or not blob_map.exists()
        or not object_refs.exists()
        or not reusable_blob_map
    ):
        extents = []
        segments = {}
        object_entries = []
        logical_offset = 0
        total_object_data_bytes = 0
        for logical_path in sorted(paths):
            entry = store_entry(logical_path)
            meta_path = object_blob_meta_path(root, entry)
            if not meta_path.exists():
                fail(
                    f"canonical object data is missing while creating view: {logical_path}"
                )
            meta = read_json(meta_path)
            extent = object_extent_from_meta(root, meta, f"canonical object {entry}")
            size = int(extent["size"])
            offset = int(extent["segment_offset"])
            if size % TAR_BLOCK_SIZE != 0 or offset % TAR_BLOCK_SIZE != 0:
                fail(
                    f"canonical object extent is not tar-block aligned: {logical_path}"
                )
            segment_id = extent["segment_id"]
            segments[segment_id] = {
                "id": segment_id,
                "blob": extent["segment_blob"],
                "size": os.path.getsize(extent["segment_blob"]),
                "status": read_segment_meta(root, segment_id).get("status", "unknown"),
            }
            extents.append(
                {
                    "logical_offset": logical_offset,
                    "segment_id": segment_id,
                    "segment_blob": extent["segment_blob"],
                    "segment_offset": offset,
                    "length": size,
                    "object": entry,
                    "object_sha256": extent["sha256"],
                }
            )
            object_entries.append(entry)
            logical_offset += size
            total_object_data_bytes += size

        eof = ensure_tar_eof_extent(root, txid, target_bytes)
        eof_extent = object_extent_from_meta(root, eof, "canonical tar EOF")
        eof_segment_id = eof_extent["segment_id"]
        segments[eof_segment_id] = {
            "id": eof_segment_id,
            "blob": eof_extent["segment_blob"],
            "size": os.path.getsize(eof_extent["segment_blob"]),
            "status": read_segment_meta(root, eof_segment_id).get("status", "unknown"),
        }
        extents.append(
            {
                "logical_offset": logical_offset,
                "segment_id": eof_segment_id,
                "segment_blob": eof_extent["segment_blob"],
                "segment_offset": int(eof_extent["segment_offset"]),
                "length": int(eof_extent["size"]),
                "object": ".tar-eof",
                "object_sha256": eof_extent["sha256"],
            }
        )
        logical_offset += int(eof_extent["size"])

        blob_map_payload = {
            "schema": SCHEMA_BLOB_MAP,
            "closure_hash": closure_id,
            "filesystem": "erofs",
            "view_backend": VIEW_BACKEND_EROFS_OBJECTMAP,
            "external_data_format": "posix-tar-index",
            "metadata_image": str(metadata_image),
            "segments": [
                segments[key] for key in sorted(segments, key=lambda value: int(value))
            ],
            "segment_count": len(segments),
            "logical_size": logical_offset,
            "object_count": len(object_entries),
            "object_data_bytes": total_object_data_bytes,
            "objects": object_entries,
            "extents": extents,
        }

        tmp_metadata = view_root / f".store.erofs.tmp-{uuid.uuid4().hex}"
        create_erofs_tar_index_image(
            root, blob_map_payload, tmp_metadata, closure_id, txid
        )
        maybe_failpoint("view-after-metadata-build")
        os.replace(tmp_metadata, metadata_image)
        os.chmod(metadata_image, 0o444)
        write_blob_map(blob_map, blob_map_payload)
        write_object_refs(object_refs, object_entries)
        maybe_failpoint("view-after-metadata-publish")
        atomic_json(
            view_manifest,
            {
                "schema": "airlock.closure-store.view-erofs-objectmap.v1",
                "closure_hash": closure_id,
                "filesystem": "erofs",
                "view_backend": VIEW_BACKEND_EROFS_OBJECTMAP,
                "external_data_format": "posix-tar-index",
                "metadata_image": str(metadata_image),
                "blob_map": str(blob_map),
                "object_refs": str(object_refs),
                "object_count": len(object_entries),
                "object_data_bytes": total_object_data_bytes,
                "read_only": True,
                "image_contains_materialized_store_copy": False,
            },
        )
        fsync_dir(view_root)
        view_created = True

    blob_map_payload = read_json(blob_map)
    generation_metadata = generation_dir / "store.erofs"
    generation_blob_map = generation_dir / "store.blob-map.json"
    generation_object_refs = generation_dir / "object-refs"
    for target, source in [
        (generation_metadata, metadata_image),
        (generation_blob_map, blob_map),
        (generation_object_refs, object_refs),
    ]:
        if target.exists() or target.is_symlink():
            target.unlink()
        os.symlink(os.path.relpath(source, generation_dir), target)
    fsync_dir(generation_dir)

    return {
        "view_backend": VIEW_BACKEND_EROFS_OBJECTMAP,
        "filesystem": "erofs",
        "view_dir": str(view_root),
        "image": str(generation_metadata),
        "canonical_image": str(metadata_image),
        "metadata_image": str(generation_metadata),
        "canonical_metadata_image": str(metadata_image),
        "blob_map": str(generation_blob_map),
        "canonical_blob_map": str(blob_map),
        "object_refs": str(generation_object_refs),
        "canonical_object_refs": str(object_refs),
        "blob_device": "/dev/xvdf",
        "blob_volume": "store-data",
        "blob_logical_size": blob_map_payload["logical_size"],
        "object_data_bytes": blob_map_payload.get("object_data_bytes", 0),
        "object_extent_count": len(blob_map_payload.get("extents", [])),
        "segment_count": blob_map_payload.get("segment_count", 0),
        "segments": blob_map_payload.get("segments", []),
        "image_created": view_created,
        "sizing": {
            "metadata_image_bytes": os.path.getsize(metadata_image),
            "blob_logical_size": blob_map_payload["logical_size"],
            "object_data_bytes": blob_map_payload.get("object_data_bytes", 0),
            "object_count": blob_map_payload.get("object_count", 0),
            "extent_count": len(blob_map_payload.get("extents", [])),
            "segment_count": blob_map_payload.get("segment_count", 0),
        },
        "image_contains_materialized_store_copy": False,
    }


def materialize_view(
    root,
    domain,
    generation,
    paths,
    closure_id,
    view_backend,
    txid,
    target_bytes,
):
    generation_dir = Path(root) / "domains" / domain / "generations" / str(generation)
    if view_backend == VIEW_BACKEND_EROFS_OBJECTMAP:
        return materialize_erofs_objectmap_view(
            root, generation_dir, paths, closure_id, txid, target_bytes
        )
    fail(f"unsupported closure view backend: {view_backend}")
