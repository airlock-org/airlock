import fcntl
import os
from contextlib import suppress
from pathlib import Path


from .core import (
    DEFAULT_SEGMENT_TARGET_BYTES,
    OBJECT_BLOB_ALIGNMENT,
    SCHEMA_OBJECT_BLOB,
    SCHEMA_SEGMENT,
    TAR_BLOCK_SIZE,
    TAR_EOF_SIZE,
    align_up,
    atomic_json,
    closure_store_fsync_enabled,
    fail,
    fsync_dir,
    hash_file,
    maybe_failpoint,
    physical_store_path,
    read_json,
    remove_path,
    require_tool,
    run_checked,
    store_entry,
    tree_hash,
    utc_now,
)
from .segments import (
    create_segment_locked,
    current_open_segment_locked,
    file_size,
    legacy_object_blob_path,
    object_blob_meta_path,
    object_extent_from_meta,
    object_meta_path,
    read_segment_meta,
    seal_segment_locked,
    segment_blob_path,
    segment_file_size,
    segment_id_from_number,
    segment_lock_path,
    segment_meta_path,
    segments_dir,
    tar_eof_meta_path,
    validate_segment_id,
    write_segment_meta,
)


def migrate_single_blob_to_segments(root):
    root = Path(root)
    legacy_blob = legacy_object_blob_path(root)
    if not legacy_blob.exists():
        return
    segment_directory = segments_dir(root)
    segment_directory.mkdir(parents=True, exist_ok=True)
    lock_path = segment_lock_path(root)
    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if not legacy_blob.exists():
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
            return
        existing = [path for path in segment_directory.glob("*.blob")]
        if existing:
            fail("cannot migrate legacy object blob while segment blobs already exist")
        legacy_realpath = os.path.realpath(legacy_blob)
        for meta_path in sorted((root / "objects/blob-metadata").glob("*.json")):
            payload = read_json(meta_path)
            if (
                payload.get("central_blob")
                and os.path.realpath(payload["central_blob"]) != legacy_realpath
            ):
                fail(
                    f"legacy object metadata points outside the legacy object blob: {meta_path}"
                )
        segment_id = segment_id_from_number(1)
        new_blob = segment_blob_path(root, segment_id)
        os.replace(legacy_blob, new_blob)
        os.chmod(new_blob, 0o400)
        objects = []
        for meta_path in sorted((root / "objects/blob-metadata").glob("*.json")):
            payload = read_json(meta_path)
            if "offset" in payload:
                payload["segment_offset"] = int(payload.pop("offset"))
            payload["segment_id"] = segment_id
            payload["segment_blob"] = str(new_blob)
            payload.pop("central_blob", None)
            payload["locations"] = [
                {
                    "segment_id": segment_id,
                    "segment_offset": int(payload["segment_offset"]),
                    "size": int(payload["size"]),
                    "sha256": payload["sha256"],
                    "status": "current",
                    "created_at": utc_now(),
                    "transaction": payload.get("transaction"),
                }
            ]
            atomic_json(meta_path, payload, mode=0o444)
            if meta_path.name != ".tar-eof.json":
                objects.append(
                    {
                        "entry": payload.get("entry")
                        or meta_path.name[: -len(".json")],
                        "offset": int(payload["segment_offset"]),
                        "size": int(payload["size"]),
                        "sha256": payload["sha256"],
                    }
                )
        for meta_path in sorted((root / "objects/metadata").glob("*.json")):
            payload = read_json(meta_path)
            fragment = payload.get("tar_fragment")
            if not fragment:
                continue
            if "offset" in fragment:
                fragment["segment_offset"] = int(fragment.pop("offset"))
            fragment["segment_id"] = segment_id
            fragment["segment_blob"] = str(new_blob)
            fragment.pop("central_blob", None)
            fragment["locations"] = [
                {
                    "segment_id": segment_id,
                    "segment_offset": int(fragment["segment_offset"]),
                    "size": int(fragment["size"]),
                    "sha256": fragment["sha256"],
                    "status": "current",
                    "created_at": utc_now(),
                    "transaction": fragment.get("transaction"),
                }
            ]
            atomic_json(meta_path, payload)
        atomic_json(
            segment_meta_path(root, segment_id),
            {
                "schema": SCHEMA_SEGMENT,
                "id": segment_id,
                "blob": str(new_blob),
                "status": "sealed",
                "sealed": True,
                "created_at": utc_now(),
                "sealed_at": utc_now(),
                "seal_reason": "legacy-single-blob-migration",
                "target_bytes": max(
                    DEFAULT_SEGMENT_TARGET_BYTES, os.path.getsize(new_blob)
                ),
                "used_bytes": os.path.getsize(new_blob),
                "object_count": len(objects),
                "objects": objects,
                "oversized": os.path.getsize(new_blob) > DEFAULT_SEGMENT_TARGET_BYTES,
                "migrated_from": "objects/data.blob",
                "append_only_while_open": True,
                "immutable_after_seal": True,
            },
            mode=0o444,
        )
        with suppress(FileNotFoundError):
            remove_path(root / "objects/data.blob.lock")
        fsync_dir(segment_directory)
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)


def migrate_legacy_blob_map(root, path, segment_id):
    root = Path(root)
    path = Path(path)
    if not path.exists() or path.is_symlink():
        return False
    payload = read_json(path)
    if "central_blob" not in payload and all(
        "segment_id" in extent for extent in payload.get("extents", [])
    ):
        return False
    segment_id = validate_segment_id(segment_id)
    segment_blob = str(segment_blob_path(root, segment_id))
    segments = {
        segment_id: {
            "id": segment_id,
            "blob": segment_blob,
            "size": (
                os.path.getsize(segment_blob) if os.path.exists(segment_blob) else 0
            ),
            "status": "sealed",
        }
    }
    for extent in payload.get("extents", []):
        if "segment_id" not in extent:
            extent["segment_id"] = segment_id
        if "segment_offset" not in extent and "central_offset" in extent:
            extent["segment_offset"] = int(extent.pop("central_offset"))
        extent["segment_blob"] = segment_blob
        segments.setdefault(
            extent["segment_id"],
            {
                "id": extent["segment_id"],
                "blob": extent["segment_blob"],
                "size": (
                    os.path.getsize(extent["segment_blob"])
                    if os.path.exists(extent["segment_blob"])
                    else 0
                ),
                "status": "sealed",
            },
        )
    payload.pop("central_blob", None)
    payload["segments"] = [
        segments[key] for key in sorted(segments, key=lambda value: int(value))
    ]
    payload["segment_count"] = len(payload["segments"])
    atomic_json(path, payload, mode=0o444)
    return True


def migrate_legacy_blob_maps(root):
    root = Path(root)
    first_segment = segment_blob_path(root, "000001")
    if not first_segment.exists():
        return
    for blob_map in sorted((root / "views/by-closure").glob("*/store.blob-map.json")):
        migrate_legacy_blob_map(root, blob_map, "000001")
    for blob_map in sorted(
        (root / "domains").glob("*/generations/*/store.blob-map.json")
    ):
        migrate_legacy_blob_map(root, blob_map, "000001")


def create_tar_fragment(source, entry, target):
    source = Path(source)
    target = Path(target)
    full_tar = target.with_suffix(".full.tar")
    if target.exists():
        target.unlink()
    if full_tar.exists():
        full_tar.unlink()

    tar = require_tool("tar")
    run_checked(
        [
            tar,
            "--format=posix",
            # POSIX/PAX stores atime and ctime even with --mtime. These change
            # when Nix restores/deduplicates a package, breaking cached seed
            # descriptors. Keep long-path support without host inode metadata
            # or process-dependent extended-header names.
            "--pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime",
            "--hard-dereference",
            "--sort=name",
            "--numeric-owner",
            "--owner=0",
            "--group=0",
            "--mtime=@1",
            "--blocking-factor=1",
            "-cf",
            str(full_tar),
            "-C",
            str(source.parent),
            "--",
            source.name,
        ]
    )

    size = file_size(full_tar)
    if size < TAR_EOF_SIZE:
        fail(f"tar fragment is too small for {entry}")
    with full_tar.open("r+b") as handle:
        handle.seek(size - TAR_EOF_SIZE)
        trailer = handle.read(TAR_EOF_SIZE)
        if trailer != b"\0" * TAR_EOF_SIZE:
            fail(f"tar fragment does not end with a standard EOF marker for {entry}")
        handle.truncate(size - TAR_EOF_SIZE)
    if file_size(full_tar) % TAR_BLOCK_SIZE != 0:
        fail(f"tar fragment is not tar-block aligned for {entry}")
    os.replace(full_tar, target)
    os.chmod(target, 0o400)


def append_file_to_open_segment_blob(backing, source, segment_id, txid):
    source = Path(source)
    digest = hash_file(source)
    size = file_size(source)
    if size % TAR_BLOCK_SIZE != 0:
        fail(f"object data is not tar-block aligned: {source}")

    with source.open("rb") as data:
        backing.seek(0, os.SEEK_END)
        offset = align_up(backing.tell(), OBJECT_BLOB_ALIGNMENT)
        padding = offset - backing.tell()
        if padding:
            backing.write(b"\0" * padding)
        for chunk in iter(lambda: data.read(1024 * 1024), b""):
            backing.write(chunk)
        backing.flush()
        if closure_store_fsync_enabled():
            os.fsync(backing.fileno())

    return {
        "segment_id": segment_id,
        "segment_offset": offset,
        "size": size,
        "sha256": digest,
        "transaction": txid,
    }


def append_file_to_segment(root, source, entry, txid, target_bytes):
    root = Path(root)
    source = Path(source)
    size = file_size(source)
    lock_path = segment_lock_path(root)
    lock_path.parent.mkdir(parents=True, exist_ok=True)

    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        oversized = size > target_bytes
        if oversized:
            segment_id, meta = create_segment_locked(
                root, target_bytes, txid, oversized=True, make_current=False
            )
        else:
            current = current_open_segment_locked(root)
            if current is None:
                segment_id, meta = create_segment_locked(root, target_bytes, txid)
            else:
                segment_id, meta = current
            blob_size = segment_file_size(root, segment_id)
            offset = align_up(blob_size, OBJECT_BLOB_ALIGNMENT)
            if int(meta.get("object_count", 0)) > 0 and offset + size > target_bytes:
                seal_segment_locked(root, segment_id, txid, "target-size-rollover")
                segment_id, meta = create_segment_locked(root, target_bytes, txid)

        blob_path = segment_blob_path(root, segment_id)
        with blob_path.open("r+b") as backing:
            result = append_file_to_open_segment_blob(backing, source, segment_id, txid)
        result["segment_blob"] = str(blob_path)

        meta = read_segment_meta(root, segment_id)
        used_bytes = int(result["segment_offset"]) + int(result["size"])
        objects = list(meta.get("objects", []))
        objects.append(
            {
                "entry": entry,
                "offset": int(result["segment_offset"]),
                "size": int(result["size"]),
                "sha256": result["sha256"],
            }
        )
        meta.update(
            {
                "updated_at": utc_now(),
                "used_bytes": max(int(meta.get("used_bytes", 0)), used_bytes),
                "object_count": int(meta.get("object_count", 0)) + 1,
                "objects": objects,
            }
        )
        write_segment_meta(root, segment_id, meta)
        if oversized:
            seal_segment_locked(root, segment_id, txid, "oversized-object")
        elif int(meta["used_bytes"]) >= target_bytes:
            seal_segment_locked(root, segment_id, txid, "target-size-reached")
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    return result


def ensure_tar_eof_extent(root, txid, target_bytes):
    root = Path(root)
    meta_path = tar_eof_meta_path(root)
    if meta_path.exists():
        meta = read_json(meta_path)
        object_extent_from_meta(root, meta, "canonical tar EOF")
        return meta

    tmp = root / "tmp" / f"tar-eof-{txid}"
    tmp.write_bytes(b"\0" * TAR_EOF_SIZE)
    os.chmod(tmp, 0o400)
    extent = append_file_to_segment(root, tmp, ".tar-eof", txid, target_bytes)
    remove_path(tmp)
    payload = {
        "schema": "airlock.closure-store.tar-eof.v1",
        "segment_id": extent["segment_id"],
        "segment_blob": extent["segment_blob"],
        "segment_offset": extent["segment_offset"],
        "size": extent["size"],
        "sha256": extent["sha256"],
        "admitted_at": utc_now(),
        "transaction": txid,
    }
    atomic_json(meta_path, payload)
    return payload


def admit_object_tar_fragment(
    root, logical_path, source, source_hash, txid, target_bytes
):
    root = Path(root)
    entry = store_entry(logical_path)
    meta = object_blob_meta_path(root, entry)
    if meta.exists():
        current = read_json(meta)
        if current.get("tree_hash") != source_hash:
            fail(f"existing object hash mismatch for {logical_path}")
        object_extent_from_meta(root, current, f"canonical object {entry}")
        return dict(current, admitted=False)

    tmp = root / "tmp" / f"object-tar-{entry}-{txid}"
    if tmp.exists():
        remove_path(tmp)
    create_tar_fragment(source, entry, tmp)

    lock_path = segment_lock_path(root)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if meta.exists():
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
            remove_path(tmp)
            current = read_json(meta)
            if current.get("tree_hash") != source_hash:
                fail(f"existing object hash mismatch for {logical_path}")
            object_extent_from_meta(root, current, f"canonical object {entry}")
            return dict(current, admitted=False)
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    extent = append_file_to_segment(root, tmp, entry, txid, target_bytes)
    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if meta.exists():
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
            remove_path(tmp)
            current = read_json(meta)
            if current.get("tree_hash") != source_hash:
                fail(f"existing object hash mismatch for {logical_path}")
            object_extent_from_meta(root, current, f"canonical object {entry}")
            return dict(current, admitted=False)
        maybe_failpoint("object-after-append")
        payload = {
            "schema": SCHEMA_OBJECT_BLOB,
            "path": logical_path,
            "entry": entry,
            "tree_hash": source_hash,
            "format": "posix-tar-fragment-v1",
            "segment_id": extent["segment_id"],
            "segment_blob": extent["segment_blob"],
            "segment_offset": extent["segment_offset"],
            "size": extent["size"],
            "sha256": extent["sha256"],
            "admitted_at": utc_now(),
            "transaction": txid,
            "locations": [
                {
                    "segment_id": extent["segment_id"],
                    "segment_offset": int(extent["segment_offset"]),
                    "size": int(extent["size"]),
                    "sha256": extent["sha256"],
                    "status": "current",
                    "created_at": utc_now(),
                    "transaction": txid,
                }
            ],
        }
        atomic_json(meta, payload)
        maybe_failpoint("object-after-metadata")
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    remove_path(tmp)
    payload["admitted"] = True
    return payload


def admit_object(root, logical_path, physical_store_root, txid, target_bytes):
    entry = store_entry(logical_path)
    source = physical_store_path(logical_path, physical_store_root)
    if not source.exists() and not source.is_symlink():
        fail(f"store object source is missing: {logical_path} ({source})")

    source_hash = tree_hash(source)
    meta = object_meta_path(root, entry)
    existing_meta = read_json(meta) if meta.exists() else {}
    if existing_meta and existing_meta.get("tree_hash") != source_hash:
        fail(f"existing object hash mismatch for {logical_path}")

    tar_fragment = admit_object_tar_fragment(
        root, logical_path, source, source_hash, txid, target_bytes
    )
    admitted = not existing_meta or tar_fragment.get("admitted", False)
    payload = {
        "schema": "airlock.closure-store.object.v2",
        "path": logical_path,
        "entry": entry,
        "tree_hash": source_hash,
        "tar_fragment": {
            "schema": SCHEMA_OBJECT_BLOB,
            "segment_id": tar_fragment["segment_id"],
            "segment_blob": tar_fragment["segment_blob"],
            "segment_offset": int(tar_fragment["segment_offset"]),
            "size": int(tar_fragment["size"]),
            "sha256": tar_fragment["sha256"],
            "format": tar_fragment.get("format", "posix-tar-fragment-v1"),
            "locations": tar_fragment.get(
                "locations",
                [
                    {
                        "segment_id": tar_fragment["segment_id"],
                        "segment_offset": int(tar_fragment["segment_offset"]),
                        "size": int(tar_fragment["size"]),
                        "sha256": tar_fragment["sha256"],
                        "status": "current",
                        "created_at": tar_fragment.get("admitted_at", utc_now()),
                        "transaction": tar_fragment.get("transaction"),
                    }
                ],
            ),
        },
        "tree_materialized": False,
        "admitted_at": existing_meta.get("admitted_at", utc_now()),
    }
    atomic_json(meta, payload)

    return {
        "path": logical_path,
        "entry": entry,
        "tree_hash": source_hash,
        "tar_fragment": payload["tar_fragment"],
        "admitted": admitted,
    }
