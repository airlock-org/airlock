import hashlib
import os
import re
from pathlib import Path


from .core import (
    DEFAULT_SEGMENT_TARGET_BYTES,
    SCHEMA_SEGMENT,
    TAR_BLOCK_SIZE,
    atomic_json,
    atomic_write,
    closure_store_fsync_enabled,
    fail,
    fsync_dir,
    maybe_failpoint,
    read_json,
    remove_path,
    utc_now,
)


def object_meta_path(root, entry):
    return Path(root) / "objects/metadata" / f"{entry}.json"


def object_blob_meta_path(root, entry):
    return Path(root) / "objects/blob-metadata" / f"{entry}.json"


def tar_eof_meta_path(root):
    return Path(root) / "objects/blob-metadata" / ".tar-eof.json"


def legacy_object_blob_path(root):
    return Path(root) / "objects" / "data.blob"


def segments_dir(root):
    return Path(root) / "objects/segments"


def segment_lock_path(root):
    return Path(root) / "objects/segments.lock"


def current_segment_ref_path(root):
    return Path(root) / "objects/current-segment"


def parse_positive_int(value, label):
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        fail(f"{label} must be an integer: {value!r}")
    if parsed <= 0:
        fail(f"{label} must be positive: {value!r}")
    return parsed


def configured_segment_target_bytes(value=None):
    if value is None or value == "":
        value = os.environ.get("AIRLOCK_CLOSURE_STORE_SEGMENT_TARGET_BYTES")
    if value is None or value == "":
        return DEFAULT_SEGMENT_TARGET_BYTES
    parsed = parse_positive_int(value, "segment target bytes")
    if parsed < TAR_BLOCK_SIZE:
        fail(f"segment target bytes must be at least {TAR_BLOCK_SIZE}")
    return parsed


def segment_id_from_number(number):
    return f"{int(number):06d}"


def validate_segment_id(segment_id):
    segment_id = str(segment_id)
    if not re.fullmatch(r"[0-9]{6,}", segment_id):
        fail(f"invalid segment id: {segment_id!r}")
    return segment_id


def segment_blob_path(root, segment_id):
    return segments_dir(root) / f"{validate_segment_id(segment_id)}.blob"


def segment_meta_path(root, segment_id):
    return segments_dir(root) / f"{validate_segment_id(segment_id)}.json"


def segment_ids(root):
    ids = set()
    directory = segments_dir(root)
    if not directory.exists():
        return []
    for child in directory.iterdir():
        if child.suffix not in (".blob", ".json"):
            continue
        if re.fullmatch(r"[0-9]{6,}", child.stem):
            ids.add(child.stem)
    return sorted(ids, key=lambda value: int(value))


def next_segment_id(root):
    ids = segment_ids(root)
    if not ids:
        return segment_id_from_number(1)
    return segment_id_from_number(max(int(segment_id) for segment_id in ids) + 1)


def read_segment_meta(root, segment_id):
    path = segment_meta_path(root, segment_id)
    if not path.exists():
        fail(f"segment metadata is missing: {segment_id}")
    return read_json(path)


def write_segment_meta(root, segment_id, payload):
    mode = (
        0o444 if payload.get("sealed") or payload.get("status") == "sealed" else 0o600
    )
    atomic_json(segment_meta_path(root, segment_id), payload, mode=mode)


def segment_file_size(root, segment_id):
    path = segment_blob_path(root, segment_id)
    return os.path.getsize(path) if path.exists() else 0


def current_open_segment_locked(root):
    ref = current_segment_ref_path(root)
    if not ref.exists():
        return None
    segment_id = ref.read_text(encoding="utf-8").strip()
    if not segment_id:
        remove_path(ref)
        return None
    meta_path = segment_meta_path(root, segment_id)
    blob_path = segment_blob_path(root, segment_id)
    if not meta_path.exists() or not blob_path.exists():
        remove_path(ref)
        return None
    meta = read_json(meta_path)
    if meta.get("status") != "open" or meta.get("sealed") or meta.get("oversized"):
        remove_path(ref)
        return None
    return segment_id, meta


def create_segment_locked(
    root, target_bytes, txid, *, oversized=False, make_current=True
):
    root = Path(root)
    segment_id = next_segment_id(root)
    blob_path = segment_blob_path(root, segment_id)
    meta_path = segment_meta_path(root, segment_id)
    if blob_path.exists() or meta_path.exists():
        fail(f"segment already exists: {segment_id}")
    with blob_path.open("xb") as handle:
        handle.flush()
        if closure_store_fsync_enabled():
            os.fsync(handle.fileno())
    os.chmod(blob_path, 0o600)
    fsync_dir(blob_path.parent)

    payload = {
        "schema": SCHEMA_SEGMENT,
        "id": segment_id,
        "blob": str(blob_path),
        "status": "open",
        "sealed": False,
        "created_at": utc_now(),
        "created_by_transaction": txid,
        "target_bytes": int(target_bytes),
        "used_bytes": 0,
        "object_count": 0,
        "objects": [],
        "oversized": bool(oversized),
        "append_only_while_open": True,
        "immutable_after_seal": True,
    }
    write_segment_meta(root, segment_id, payload)
    maybe_failpoint("segment-after-create")
    if make_current and not oversized:
        atomic_write(current_segment_ref_path(root), f"{segment_id}\n", mode=0o644)
    return segment_id, payload


def seal_segment_locked(root, segment_id, txid, reason):
    segment_id = validate_segment_id(segment_id)
    meta = read_segment_meta(root, segment_id)
    if meta.get("status") == "sealed" or meta.get("sealed"):
        return meta
    maybe_failpoint("segment-before-seal")
    blob_path = segment_blob_path(root, segment_id)
    if not blob_path.exists():
        fail(f"segment blob is missing while sealing: {segment_id}")
    used_bytes = os.path.getsize(blob_path)
    os.chmod(blob_path, 0o400)
    meta.update(
        {
            "status": "sealed",
            "sealed": True,
            "sealed_at": utc_now(),
            "sealed_by_transaction": txid,
            "seal_reason": reason,
            "used_bytes": used_bytes,
        }
    )
    write_segment_meta(root, segment_id, meta)
    ref = current_segment_ref_path(root)
    if ref.exists() and ref.read_text(encoding="utf-8").strip() == segment_id:
        remove_path(ref)
    maybe_failpoint("segment-after-seal")
    return meta


def verify_segment_region(root, segment_id, offset, size, digest, label):
    blob_path = segment_blob_path(root, segment_id)
    with blob_path.open("rb") as handle:
        handle.seek(offset)
        remaining = size
        current = hashlib.sha256()
        while remaining:
            data = handle.read(min(1024 * 1024, remaining))
            if not data:
                fail(f"{label} is truncated at offset {offset}")
            remaining -= len(data)
            current.update(data)
    if current.hexdigest() != digest:
        fail(f"{label} hash mismatch at offset {offset}")


def object_extent_from_meta(root, meta, label):
    location = current_location_from_meta(meta, label)
    segment_id = validate_segment_id(location["segment_id"])
    offset = int(location["segment_offset"])
    size = int(location["size"])
    digest = location["sha256"]
    verify_segment_region(root, segment_id, offset, size, digest, label)
    return {
        "segment_id": segment_id,
        "segment_blob": str(segment_blob_path(root, segment_id)),
        "segment_offset": offset,
        "size": size,
        "sha256": digest,
    }


def current_location_from_meta(meta, label):
    for location in meta.get("locations", []):
        if location.get("status") == "current":
            return normalize_location(location, label)
    if "segment_id" not in meta or "segment_offset" not in meta:
        fail(f"{label} uses obsolete single-blob metadata")
    return normalize_location(meta, label)


def normalize_location(location, label):
    missing = [
        key
        for key in ["segment_id", "segment_offset", "size", "sha256"]
        if key not in location
    ]
    if missing:
        fail(f"{label} location is missing: {', '.join(missing)}")
    return {
        "segment_id": validate_segment_id(location["segment_id"]),
        "segment_offset": int(location["segment_offset"]),
        "size": int(location["size"]),
        "sha256": location["sha256"],
        "status": location.get("status", "current"),
        "transaction": location.get("transaction"),
        "created_at": location.get("created_at") or location.get("admitted_at"),
        "retired_at": location.get("retired_at"),
    }


def location_from_extent(extent, *, status="current", txid=None):
    payload = {
        "segment_id": validate_segment_id(extent["segment_id"]),
        "segment_offset": int(extent["segment_offset"]),
        "size": int(extent["size"]),
        "sha256": extent["sha256"],
        "status": status,
    }
    if txid:
        payload["transaction"] = txid
    return payload


def top_level_location_fields(root, location):
    return {
        "segment_id": validate_segment_id(location["segment_id"]),
        "segment_blob": str(segment_blob_path(root, location["segment_id"])),
        "segment_offset": int(location["segment_offset"]),
        "size": int(location["size"]),
        "sha256": location["sha256"],
    }


def location_key(location):
    location = normalize_location(location, "object")
    return (
        location["segment_id"],
        int(location["segment_offset"]),
        int(location["size"]),
        location["sha256"],
    )


def object_blob_meta_paths(root):
    for meta in sorted((Path(root) / "objects/blob-metadata").glob("*.json")):
        if meta.name == ".tar-eof.json":
            continue
        yield meta


def all_object_blob_meta_paths(root, include_tar_eof=False):
    yield from object_blob_meta_paths(root)
    if include_tar_eof and tar_eof_meta_path(root).exists():
        yield tar_eof_meta_path(root)


def file_size(path):
    return Path(path).stat().st_size
