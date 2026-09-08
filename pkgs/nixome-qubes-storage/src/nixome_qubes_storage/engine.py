import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import time
import uuid
from contextlib import suppress
from pathlib import Path

SCHEMA_POOL = "nixome.closure-store.v1"
SCHEMA_ADMISSION = "nixome.closure-store.admission.v1"
SCHEMA_GENERATION = "nixome.closure-store.generation.v1"
SCHEMA_SWITCH = "nixome.closure-store.switch.v1"
SCHEMA_ROLLBACK = "nixome.closure-store.rollback.v1"
SCHEMA_RETENTION = "nixome.closure-store.retention.v1"
SCHEMA_GC = "nixome.closure-store.gc.v1"
SCHEMA_COMPACTION = "nixome.closure-store.compaction.v1"
SCHEMA_RECOVERY = "nixome.closure-store.recovery.v1"
SCHEMA_STATUS = "nixome.closure-store.status.v1"
SCHEMA_BLOB_MAP = "nixome.closure-store.blob-map.v1"
SCHEMA_OBJECT_BLOB = "nixome.closure-store.object-tar-fragment.v1"
SCHEMA_SEGMENT = "nixome.closure-store.segment.v1"
SCHEMA_RUNNING_REF = "nixome.closure-store.running-ref.v1"
SCHEMA_RUNTIME_VIEW_REF = "nixome.closure-store.runtime-view-ref.v1"

VIEW_BACKEND_EROFS_OBJECTMAP = "erofs-objectmap"
DEFAULT_VIEW_BACKEND = VIEW_BACKEND_EROFS_OBJECTMAP
OBJECT_BLOB_ALIGNMENT = 4096
DEFAULT_SEGMENT_TARGET_BYTES = 2 * 1024 * 1024 * 1024
DEFAULT_COMPACTION_THRESHOLD_PERCENT = 50
DEFAULT_COMPACTION_MAX_SEGMENTS = 1
TAR_BLOCK_SIZE = 512
TAR_EOF_SIZE = 1024


class EngineError(RuntimeError):
    pass


def utc_now():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def fail(message):
    raise EngineError(message)


def maybe_failpoint(name):
    if os.environ.get("NIXOME_CLOSURE_STORE_FAILPOINT") == name:
        fail(f"simulated failure at {name}")


def parse_bool(value):
    if isinstance(value, bool):
        return value
    if value in ("1", "true", "yes", "on"):
        return True
    if value in ("0", "false", "no", "off"):
        return False
    fail(f"invalid boolean value: {value!r}")


def fsync_dir(path):
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def atomic_write(path, data, mode=0o644):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name("." + path.name + ".tmp-" + uuid.uuid4().hex)
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.chmod(tmp, mode)
    os.replace(tmp, path)
    fsync_dir(path.parent)


def atomic_json(path, payload, mode=0o644):
    atomic_write(path, json.dumps(payload, indent=2, sort_keys=True) + "\n", mode)


def read_json(path):
    with Path(path).open("r", encoding="utf-8") as handle:
        return json.load(handle)


def safe_domain_name(name):
    if not name or "/" in name or name in (".", ".."):
        fail(f"invalid domain name: {name!r}")
    return name


def store_entry(path):
    prefix = "/nix/store/"
    if not path.startswith(prefix):
        fail(f"not a Nix store path: {path}")
    entry = path[len(prefix) :]
    if not entry or "/" in entry or entry in (".", ".."):
        fail(f"not a store path root: {path}")
    return entry


def logical_store_root():
    return "/nix/store"


def default_physical_store_root():
    nix_root = os.environ.get("NIXOME_NIX_ROOT")
    if nix_root and nix_root != "/":
        return str(Path(nix_root) / "nix/store")
    return "/nix/store"


def physical_store_path(logical_path, physical_store_root):
    entry = store_entry(logical_path)
    return Path(physical_store_root) / entry


def read_store_paths(source, system=None):
    source_path = Path(source)
    candidates = []
    if source_path.is_dir():
        candidates.extend(
            [
                source_path / "store-paths",
                source_path / "closure/store-paths",
            ]
        )
    else:
        candidates.append(source_path)

    for candidate in candidates:
        if candidate.is_file():
            paths = [
                line.strip()
                for line in candidate.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
            if system and system not in paths:
                paths.append(system)
            return sorted(set(paths))

    if system:
        return [system]
    fail(f"closure source does not contain store-paths: {source}")


def hash_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_hash(path):
    path = Path(path)
    digest = hashlib.sha256()
    if path.is_symlink():
        digest.update(b"L\0")
        digest.update(os.readlink(path).encode())
        return digest.hexdigest()
    if path.is_file():
        st_mode = stat.S_IMODE(path.lstat().st_mode)
        digest.update(f"F\0{st_mode:o}\0".encode())
        digest.update(hash_file(path).encode())
        return digest.hexdigest()
    if not path.is_dir():
        fail(f"unsupported store object file type: {path}")
    digest.update(b"D\0")
    for child in sorted(path.rglob("*")):
        relative = child.relative_to(path).as_posix()
        st = child.lstat()
        if child.is_symlink():
            digest.update(f"L\0{relative}\0{os.readlink(child)}\0".encode())
        elif child.is_dir():
            digest.update(f"D\0{relative}\0{stat.S_IMODE(st.st_mode):o}\0".encode())
        elif child.is_file():
            digest.update(f"F\0{relative}\0{stat.S_IMODE(st.st_mode):o}\0".encode())
            digest.update(hash_file(child).encode())
            digest.update(b"\0")
        else:
            fail(f"unsupported store object file type: {child}")
    return digest.hexdigest()


def chmod_readonly(path):
    path = Path(path)
    if path.is_symlink():
        return
    for root, dirs, files in os.walk(path):
        for dirname in dirs:
            full = Path(root) / dirname
            if not full.is_symlink():
                os.chmod(full, stat.S_IMODE(full.lstat().st_mode) & ~0o222)
        for filename in files:
            full = Path(root) / filename
            if not full.is_symlink():
                os.chmod(full, stat.S_IMODE(full.lstat().st_mode) & ~0o222)
    if not path.is_symlink():
        os.chmod(path, stat.S_IMODE(path.lstat().st_mode) & ~0o222)


def chmod_owner_writable(path):
    path = Path(path)
    if path.is_symlink():
        return
    if path.is_file():
        os.chmod(path, stat.S_IMODE(path.lstat().st_mode) | 0o200)
        return
    if path.is_dir():
        os.chmod(path, stat.S_IMODE(path.lstat().st_mode) | 0o700)
        for root, dirs, files in os.walk(path):
            for dirname in dirs:
                full = Path(root) / dirname
                if not full.is_symlink():
                    os.chmod(full, stat.S_IMODE(full.lstat().st_mode) | 0o700)
            for filename in files:
                full = Path(root) / filename
                if not full.is_symlink():
                    os.chmod(full, stat.S_IMODE(full.lstat().st_mode) | 0o200)


def remove_path(path):
    path = Path(path)
    if not path.exists() and not path.is_symlink():
        return
    if path.is_dir() and not path.is_symlink():
        chmod_owner_writable(path)
        shutil.rmtree(path)
    else:
        chmod_owner_writable(path)
        path.unlink()


def ensure_pool(root):
    root = Path(root)
    for subdir in [
        "objects/metadata",
        "objects/blob-metadata",
        "objects/segments",
        "domains",
        "views/by-closure",
        "runtime",
        "running",
        "transactions",
        "tmp",
    ]:
        (root / subdir).mkdir(parents=True, exist_ok=True)
    pool_manifest = root / "MANIFEST.json"
    if not pool_manifest.exists():
        atomic_json(
            pool_manifest,
            {
                "schema": SCHEMA_POOL,
                "created_at": utc_now(),
                "object_key": "nix-store-basename",
                "object_store": {
                    "schema": "nixome.closure-store.object-store.v1",
                    "segments": "objects/segments",
                    "metadata": "objects/blob-metadata",
                    "object_key": "nix-store-basename",
                    "format": "posix-tar-fragment-v1",
                    "segment_target_bytes": DEFAULT_SEGMENT_TARGET_BYTES,
                },
            },
        )
    else:
        manifest = read_json(pool_manifest)
        object_store = manifest.setdefault("object_store", {})
        if (
            object_store.get("data") == "objects/data.blob"
            or "segments" not in object_store
        ):
            object_store.pop("data", None)
            object_store["segments"] = "objects/segments"
            object_store["segment_target_bytes"] = object_store.get(
                "segment_target_bytes", DEFAULT_SEGMENT_TARGET_BYTES
            )
            atomic_json(pool_manifest, manifest)
    migrate_single_blob_to_segments(root)
    migrate_legacy_blob_maps(root)
    return root


def require_tool(name, envvar=None):
    override = os.environ.get(envvar) if envvar else None
    if override:
        if not os.path.exists(override):
            fail(f"{envvar} points to a missing executable: {override}")
        return override
    found = shutil.which(name)
    if found:
        return found
    fail(f"required executable is missing from PATH: {name}")


def run_checked(command, *, input_text=None):
    run_kwargs = {}
    if input_text is None:
        run_kwargs["stdin"] = subprocess.DEVNULL
    else:
        run_kwargs["input"] = input_text

    result = subprocess.run(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
        **run_kwargs,
    )
    if result.returncode != 0:
        fail(
            "command failed with exit "
            f"{result.returncode}: {' '.join(command)}\n"
            f"{result.stderr.strip() or result.stdout.strip()}"
        )
    return result


def align_up(value, alignment):
    return ((value + alignment - 1) // alignment) * alignment


def run_verify(paths, mode):
    if mode == "none":
        return {
            "mode": mode,
            "status": "skipped",
            "reason": "verification disabled",
        }
    if mode == "tree":
        return {
            "mode": mode,
            "status": "passed",
            "reason": "deterministic tree hashes computed during admission",
        }
    if mode not in ("nix-store", "nix-store-trust"):
        fail(f"unsupported verification mode: {mode}")

    maybe_failpoint("verification-before-command")
    command = [
        "nix",
        "--extra-experimental-features",
        "nix-command",
        "store",
        "verify",
        "--recursive",
    ]
    if mode == "nix-store":
        command.append("--no-trust")
    command.extend(paths)
    env = os.environ.copy()
    result = subprocess.run(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        fail(
            "nix store verification failed with exit "
            f"{result.returncode}: {result.stderr.strip() or result.stdout.strip()}"
        )
    maybe_failpoint("verification-after-command")
    return {
        "mode": mode,
        "status": "passed",
        "command": command,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "env_store": env.get("NIX_CONFIG", ""),
    }


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
        value = os.environ.get("NIXOME_CLOSURE_STORE_SEGMENT_TARGET_BYTES")
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


def file_size(path):
    return Path(path).stat().st_size


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
        "schema": "nixome.closure-store.tar-eof.v1",
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
        "schema": "nixome.closure-store.object.v2",
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


def closure_hash(paths, system, root_source, kernel_name):
    digest = hashlib.sha256()
    for value in [
        SCHEMA_GENERATION,
        f"system={system}",
        f"root={root_source or ''}",
        f"kernel={kernel_name or ''}",
    ]:
        digest.update(value.encode())
        digest.update(b"\0")
    for path in sorted(paths):
        digest.update(path.encode())
        digest.update(b"\0")
    return digest.hexdigest()


def next_generation(domain_dir):
    generations = domain_dir / "generations"
    generations.mkdir(parents=True, exist_ok=True)
    numbers = []
    for child in generations.iterdir():
        if child.is_dir() and child.name.isdigit():
            numbers.append(int(child.name))
    return max(numbers, default=0) + 1


def hash_bytes(data):
    return hashlib.sha256(data).hexdigest()


def closure_store_fsync_enabled():
    return parse_bool(os.environ.get("NIXOME_CLOSURE_STORE_FSYNC", "1"))


def write_blob_map(path, payload):
    atomic_json(path, payload, mode=0o444)


def write_object_refs(path, refs):
    atomic_write(path, "".join(f"{ref}\n" for ref in refs), mode=0o444)


def read_object_refs(path):
    if not path:
        return []
    return [
        line.strip()
        for line in Path(path).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def add_manifest_store_liveness(root, manifest, live_objects, live_views):
    store = manifest.get("store", {})
    for path in store.get("paths", []):
        live_objects.add(store_entry(path))
    view_root = canonical_view_root(store)
    if view_root:
        live_views.add(view_root)


def canonical_view_root(store):
    for key in (
        "canonical_metadata_image",
        "canonical_blob_map",
        "canonical_object_refs",
    ):
        value = store.get(key)
        if value:
            return str(Path(value).parent)
    return None


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
    mkfs_erofs = require_tool("mkfs.erofs", "NIXOME_MKFS_EROFS")
    root = Path(root)
    metadata_path = Path(metadata_path)
    if metadata_path.exists():
        metadata_path.unlink()

    source_mode = os.environ.get("NIXOME_CLOSURE_STORE_INDEX_SOURCE", "auto")
    dm_name = f"nixome-build-{closure_id[:16]}-{txid[:8]}"
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
        run_checked(
            [
                mkfs_erofs,
                "--quiet",
                "--tar=i",
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


def safe_dm_component(value):
    cleaned = re.sub(r"[^A-Za-z0-9_.+-]", "-", value)
    return cleaned[:48] if cleaned else "domain"


def blob_dm_name(domain, generation, closure_hash):
    short_hash = closure_hash[:16] if closure_hash else "unknown"
    return f"nixome-{safe_dm_component(domain)}-{generation}-{short_hash}"


def dm_path(name):
    return f"/dev/mapper/{name}"


def dm_exists(name):
    dmsetup = require_tool("dmsetup", "NIXOME_DMSETUP")
    result = subprocess.run(
        [dmsetup, "info", name],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def ensure_loop_for_blob(blob_path):
    losetup = require_tool("losetup", "NIXOME_LOSETUP")
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
    dmsetup = require_tool("dmsetup", "NIXOME_DMSETUP")
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
    for ref_path in sorted(runtime_dir.glob("nixome-*.json")):
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
    dmsetup = require_tool("dmsetup", "NIXOME_DMSETUP")
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
        "schema": "nixome.closure-store.attach.v1",
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
        "schema": "nixome.closure-store.detach.v1",
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
                "schema": "nixome.closure-store.view-erofs-objectmap.v1",
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


def generation_manifest_path(root, domain, generation):
    return (
        Path(root)
        / "domains"
        / domain
        / "generations"
        / str(generation)
        / "manifest.json"
    )


def transaction_path(root, txid):
    return Path(root) / "transactions" / f"{txid}.json"


def transaction_status(root, txid):
    if not txid:
        return None
    path = transaction_path(root, txid)
    if not path.exists():
        return None
    try:
        return read_json(path).get("status")
    except (OSError, json.JSONDecodeError):
        return None


def generation_is_committed(root, manifest):
    if (
        manifest.get("schema") != SCHEMA_GENERATION
        or manifest.get("status") != "committed"
    ):
        return False
    txid = manifest.get("transaction")
    if not txid:
        return True
    return transaction_status(root, txid) == "committed"


def running_ref_path(root, domain, generation, volume):
    return Path(root) / "running" / domain / str(generation) / f"{volume}.json"


def current_generation(root, domain):
    link = Path(root) / "domains" / domain / "current"
    if not link.is_symlink():
        return None
    target = os.readlink(link)
    name = Path(target).name
    return int(name) if name.isdigit() else None


def switch_current(root, domain, generation):
    domain_dir = Path(root) / "domains" / domain
    target = domain_dir / "generations" / str(generation)
    manifest_path = target / "manifest.json"
    if not manifest_path.exists():
        fail(f"generation does not exist for {domain}: {generation}")
    manifest = read_json(manifest_path)
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    tmp = domain_dir / f".current.tmp-{uuid.uuid4().hex}"
    os.symlink(os.path.relpath(target, domain_dir), tmp)
    maybe_failpoint("switch-before-current-replace")
    os.replace(tmp, domain_dir / "current")
    fsync_dir(domain_dir)
    maybe_failpoint("switch-after-current-replace")


def mark_running_ref(root, domain, generation, volume):
    root = ensure_pool(root)
    domain = safe_domain_name(domain)
    generation = int(generation)
    if not volume or "/" in volume or volume in (".", ".."):
        fail(f"invalid running volume name: {volume!r}")
    manifest_path = generation_manifest_path(root, domain, generation)
    manifest = read_json(manifest_path)
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    store = manifest.get("store", {})
    physical_refs = []
    blob_map_path = store.get("canonical_blob_map") or store.get("blob_map")
    if blob_map_path and Path(blob_map_path).exists():
        blob_map = read_json(blob_map_path)
        physical_refs = [
            ref
            for refs in blob_map_physical_references(
                blob_map, f"running:{domain}:{generation}:{volume}"
            ).values()
            for ref in refs
        ]
    path = running_ref_path(root, domain, generation, volume)
    atomic_json(
        path,
        {
            "schema": SCHEMA_RUNNING_REF,
            "status": "running",
            "domain": domain,
            "generation": generation,
            "volume": volume,
            "manifest": str(manifest_path),
            "closure_hash": store.get("closure_hash", ""),
            "blob_map": str(blob_map_path) if blob_map_path else "",
            "physical_references": physical_refs,
            "started_at": utc_now(),
        },
    )
    return {
        "schema": SCHEMA_RUNNING_REF,
        "status": "running",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "volume": volume,
        "manifest": str(manifest_path),
    }


def clear_running_ref(root, domain, generation, volume):
    root = ensure_pool(root)
    domain = safe_domain_name(domain)
    generation = int(generation)
    path = running_ref_path(root, domain, generation, volume)
    removed = False
    if path.exists() or path.is_symlink():
        remove_path(path)
        removed = True
        gen_dir = path.parent
        domain_dir = gen_dir.parent
        with suppress(OSError):
            gen_dir.rmdir()
        with suppress(OSError):
            domain_dir.rmdir()
    return {
        "schema": SCHEMA_RUNNING_REF,
        "status": "cleared",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "volume": volume,
        "removed": removed,
    }


def mark_running(args):
    return mark_running_ref(args.pool_root, args.domain, args.generation, args.volume)


def clear_running(args):
    return clear_running_ref(args.pool_root, args.domain, args.generation, args.volume)


def iter_running_generation_refs(root):
    running_dir = Path(root) / "running"
    if not running_dir.exists():
        return
    for ref_path in sorted(running_dir.glob("*/*/*.json")):
        try:
            ref = read_json(ref_path)
        except (OSError, json.JSONDecodeError):
            continue
        if ref.get("schema") != SCHEMA_RUNNING_REF or ref.get("status") != "running":
            continue
        domain = ref.get("domain")
        generation = ref.get("generation")
        if not domain or generation is None:
            continue
        yield safe_domain_name(domain), int(generation), ref_path, ref


def admit_generation(args):
    root = ensure_pool(args.pool_root)
    domain = safe_domain_name(args.domain)
    domain_dir = root / "domains" / domain
    generations_dir = domain_dir / "generations"
    generations_dir.mkdir(parents=True, exist_ok=True)
    txid = uuid.uuid4().hex
    tx_path = root / "transactions" / f"{txid}.json"
    previous = current_generation(root, domain)
    generation = next_generation(domain_dir)
    generation_dir = generations_dir / str(generation)
    generation_dir.mkdir(parents=True, exist_ok=False)

    atomic_json(
        tx_path,
        {
            "schema": "nixome.closure-store.transaction.v1",
            "id": txid,
            "domain": domain,
            "generation": generation,
            "status": "preparing",
            "created_at": utc_now(),
        },
    )
    maybe_failpoint("admit-after-tx-create")

    paths = read_store_paths(args.closure_source, args.system)
    paths = sorted(set(paths))
    if args.system not in paths:
        paths.append(args.system)
        paths = sorted(set(paths))
    target_bytes = configured_segment_target_bytes(args.segment_target_bytes)

    verification = run_verify([args.system], args.verify_mode)
    admitted = []
    reused = []
    for logical_path in paths:
        result = admit_object(
            root, logical_path, args.physical_store_root, txid, target_bytes
        )
        if result["admitted"]:
            admitted.append(result)
        else:
            reused.append(result)

    closure_id = closure_hash(paths, args.system, args.root_source, args.kernel_name)
    view = materialize_view(
        root,
        domain,
        generation,
        paths,
        closure_id,
        args.view_backend,
        txid,
        target_bytes,
    )
    manifest_path = generation_dir / "manifest.json"
    manifest = {
        "schema": SCHEMA_GENERATION,
        "status": "prepared",
        "domain": domain,
        "generation": generation,
        "previous_generation": previous,
        "created_at": utc_now(),
        "transaction": txid,
        "approval": {
            "approved_by": args.approved_by,
            "source": args.approval_source,
        },
        "system": args.system,
        "root": {
            "source": args.root_source,
        },
        "kernel": {
            "name": args.kernel_name,
        },
        "store": {
            "mode": "shared-closure",
            "closure_hash": closure_id,
            "source": args.closure_source,
            "mountpoint": args.store_mountpoint,
            "device": args.store_device,
            "blob_device": args.store_blob_device if view["blob_volume"] else None,
            "blob_volume": view["blob_volume"],
            "read_only": True,
            "filesystem": view["filesystem"],
            "view_backend": view["view_backend"],
            "paths": paths,
            "object_count": len(paths),
            "view_dir": view["view_dir"],
            "image": view["image"],
            "canonical_image": view["canonical_image"],
            "metadata_image": view["metadata_image"],
            "canonical_metadata_image": view["canonical_metadata_image"],
            "blob_map": view["blob_map"],
            "canonical_blob_map": view["canonical_blob_map"],
            "object_refs": view.get("object_refs"),
            "canonical_object_refs": view.get("canonical_object_refs"),
            "blob_logical_size": view["blob_logical_size"],
            "object_data_bytes": view.get("object_data_bytes", 0),
            "object_extent_count": view.get("object_extent_count", 0),
            "segment_count": view.get("segment_count", 0),
            "segments": view.get("segments", []),
            "image_contains_materialized_store_copy": view[
                "image_contains_materialized_store_copy"
            ],
            "segment_target_bytes": target_bytes,
        },
        "verification": verification,
        "retention": {
            "retain": parse_bool(args.retain),
        },
    }
    maybe_failpoint("generation-before-manifest")
    atomic_json(manifest_path, manifest)
    maybe_failpoint("generation-after-manifest")
    atomic_json(
        tx_path,
        {
            "schema": "nixome.closure-store.transaction.v1",
            "id": txid,
            "domain": domain,
            "generation": generation,
            "status": "committed",
            "activated": parse_bool(args.activate),
            "committed_at": utc_now(),
            "manifest": str(manifest_path),
        },
    )
    maybe_failpoint("generation-after-tx-commit")
    manifest["status"] = "committed"
    atomic_json(manifest_path, manifest)
    maybe_failpoint("generation-after-manifest-commit")
    if parse_bool(args.activate):
        switch_current(root, domain, generation)

    return {
        "schema": SCHEMA_ADMISSION,
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "previous_generation": previous,
        "current_generation": current_generation(root, domain),
        "activated": parse_bool(args.activate),
        "manifest": str(manifest_path),
        "system": args.system,
        "closure_hash": closure_id,
        "verification": verification,
        "store": manifest["store"],
        "objects_total": len(paths),
        "objects_admitted": len(admitted),
        "objects_reused": len(reused),
        "admitted": admitted,
        "reused": reused,
        "view_created": view["image_created"],
        "view_image": view["image"],
        "canonical_view_image": view["canonical_image"],
        "metadata_image": view["metadata_image"],
        "canonical_metadata_image": view["canonical_metadata_image"],
        "blob_map": view["blob_map"],
        "canonical_blob_map": view["canonical_blob_map"],
        "object_refs": view.get("object_refs"),
        "canonical_object_refs": view.get("canonical_object_refs"),
        "blob_device": args.store_blob_device if view["blob_volume"] else None,
        "blob_volume": view["blob_volume"],
        "blob_logical_size": view["blob_logical_size"],
        "object_data_bytes": view.get("object_data_bytes", 0),
        "object_extent_count": view.get("object_extent_count", 0),
        "segment_count": view.get("segment_count", 0),
        "segments": view.get("segments", []),
        "filesystem": view["filesystem"],
        "view_backend": view["view_backend"],
        "view_size": os.path.getsize(view["canonical_image"]),
        "read_only": True,
    }


def rollback(args):
    result = switch(args)
    result["schema"] = SCHEMA_ROLLBACK
    return result


def set_retention(args):
    root = ensure_pool(args.pool_root)
    domain = safe_domain_name(args.domain)
    generation = int(args.generation)
    manifest_path = generation_manifest_path(root, domain, generation)
    manifest = read_json(manifest_path)
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    retain = parse_bool(args.retain)
    retention = manifest.setdefault("retention", {})
    previous = bool(retention.get("retain", False))
    retention["retain"] = retain
    retention["updated_at"] = utc_now()
    atomic_json(manifest_path, manifest)
    return {
        "schema": SCHEMA_RETENTION,
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "previous_retain": previous,
        "retain": retain,
        "manifest": str(manifest_path),
    }


def switch(args):
    root = ensure_pool(args.pool_root)
    domain = safe_domain_name(args.domain)
    generation = int(args.generation)
    manifest = read_json(generation_manifest_path(root, domain, generation))
    if not generation_is_committed(root, manifest):
        fail(f"generation is not committed for {domain}: {generation}")
    previous = current_generation(root, domain)
    switch_current(root, domain, generation)
    return {
        "schema": SCHEMA_SWITCH,
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "previous_generation": previous,
        "current_generation": generation,
        "manifest": str(generation_manifest_path(root, domain, generation)),
        "system": manifest["system"],
        "root_source": manifest.get("root", {}).get("source", ""),
        "kernel_name": manifest.get("kernel", {}).get("name", ""),
        "store": manifest["store"],
        "view_image": manifest["store"]["image"],
        "canonical_view_image": manifest["store"]["canonical_image"],
        "metadata_image": manifest["store"].get(
            "metadata_image", manifest["store"]["image"]
        ),
        "canonical_metadata_image": manifest["store"].get(
            "canonical_metadata_image", manifest["store"]["canonical_image"]
        ),
        "blob_map": manifest["store"].get("blob_map"),
        "canonical_blob_map": manifest["store"].get("canonical_blob_map"),
        "object_refs": manifest["store"].get("object_refs"),
        "canonical_object_refs": manifest["store"].get("canonical_object_refs"),
        "blob_device": manifest["store"].get("blob_device"),
        "blob_volume": manifest["store"].get("blob_volume"),
        "segment_count": manifest["store"].get("segment_count", 0),
        "segments": manifest["store"].get("segments", []),
        "filesystem": manifest["store"].get("filesystem", "erofs"),
        "view_backend": manifest["store"].get(
            "view_backend", VIEW_BACKEND_EROFS_OBJECTMAP
        ),
        "read_only": True,
    }


def iter_committed_generations(root):
    domains_dir = Path(root) / "domains"
    if not domains_dir.exists():
        return
    for domain_dir in sorted(domains_dir.iterdir()):
        generations_dir = domain_dir / "generations"
        if not generations_dir.is_dir():
            continue
        for generation_dir in sorted(generations_dir.iterdir()):
            manifest_path = generation_dir / "manifest.json"
            if manifest_path.exists():
                manifest = read_json(manifest_path)
                if generation_is_committed(root, manifest):
                    yield domain_dir.name, int(
                        generation_dir.name
                    ), manifest_path, manifest


def live_segment_references(root, live_objects, live_views):
    references = {}
    for meta_path in object_blob_meta_paths(root):
        entry = meta_path.name[: -len(".json")]
        if entry not in live_objects:
            continue
        payload = read_json(meta_path)
        extent = object_extent_from_meta(root, payload, f"canonical object {entry}")
        references.setdefault(extent["segment_id"], set()).add(entry)

    eof_path = tar_eof_meta_path(root)
    if live_views and eof_path.exists():
        payload = read_json(eof_path)
        extent = object_extent_from_meta(root, payload, "canonical tar EOF")
        references.setdefault(extent["segment_id"], set()).add(".tar-eof")
    return references


def add_running_segment_references(root, reference_details):
    for domain, generation, _ref_path, ref in iter_running_generation_refs(root):
        source = f"running:{domain}:{generation}:{ref.get('volume', '')}"
        for physical_ref in ref.get("physical_references", []):
            segment_id = validate_segment_id(physical_ref["segment_id"])
            reference_details.setdefault(segment_id, []).append(
                {
                    "segment_id": segment_id,
                    "source": source,
                    "entry": physical_ref.get("entry", "<unknown>"),
                    "offset": int(physical_ref.get("offset", 0)),
                    "length": int(physical_ref.get("length", 0)),
                    "logical_offset": int(physical_ref.get("logical_offset", 0)),
                }
            )


def add_live_view_segment_references(live_views, reference_details):
    for view in sorted(live_views):
        blob_map = Path(view) / "store.blob-map.json"
        if not blob_map.exists():
            continue
        with suppress(OSError, json.JSONDecodeError, EngineError):
            merge_segment_reference_details(
                reference_details,
                blob_map_physical_references(read_json(blob_map), f"view:{view}"),
            )


def physical_segment_reference_details(root, live_objects, live_views):
    details = {}
    for segment_id, entries in live_segment_references(
        root, live_objects, live_views
    ).items():
        for entry in entries:
            details.setdefault(segment_id, []).append(
                {
                    "segment_id": segment_id,
                    "source": "canonical-current",
                    "entry": entry,
                    "offset": 0,
                    "length": 0,
                    "logical_offset": 0,
                }
            )
    add_live_view_segment_references(live_views, details)
    add_running_segment_references(root, details)
    runtime_refs, _stale = runtime_segment_references(root)
    merge_segment_reference_details(details, runtime_refs)
    return details


def collect_segments(root):
    result = []
    for segment_id in segment_ids(root):
        blob_path = segment_blob_path(root, segment_id)
        meta_path = segment_meta_path(root, segment_id)
        meta = read_json(meta_path) if meta_path.exists() else {}
        result.append(
            {
                "id": segment_id,
                "blob": str(blob_path),
                "metadata": str(meta_path),
                "status": meta.get(
                    "status",
                    "missing-metadata" if not meta_path.exists() else "unknown",
                ),
                "sealed": bool(meta.get("sealed", False)),
                "oversized": bool(meta.get("oversized", False)),
                "target_bytes": int(
                    meta.get("target_bytes", DEFAULT_SEGMENT_TARGET_BYTES)
                ),
                "used_bytes": os.path.getsize(blob_path) if blob_path.exists() else 0,
                "object_count": int(meta.get("object_count", 0)),
            }
        )
    return result


def gc_segments(root, live_segment_refs, reference_details=None):
    reference_details = reference_details or {
        segment_id: [
            {"entry": entry, "source": "canonical-current"} for entry in entries
        ]
        for segment_id, entries in live_segment_refs.items()
    }
    current = None
    ref = current_segment_ref_path(root)
    if ref.exists():
        current = ref.read_text(encoding="utf-8").strip() or None

    removed = []
    retained = []
    partial = []
    blocked = []
    for segment in collect_segments(root):
        segment_id = segment["id"]
        details = reference_details.get(segment_id, [])
        live_entries = sorted(
            {
                ref.get("entry", "<unknown>")
                for ref in details
                if ref.get("entry") != "<unknown>"
            }
        )
        if segment["sealed"] and segment_id != current and not details:
            maybe_failpoint("gc-before-segment-delete")
            for path in [segment["blob"], segment["metadata"]]:
                remove_path(path)
            removed.append(segment_id)
            maybe_failpoint("gc-after-segment-delete")
            continue
        result = dict(segment)
        result["live_entries"] = live_entries
        result["reference_count"] = len(details)
        result["references"] = details[:50]
        retained.append(result)
        if live_entries and len(live_entries) < int(segment.get("object_count", 0)):
            partial.append(segment_id)
        if (
            segment["sealed"]
            and segment_id != current
            and details
            and not live_segment_refs.get(segment_id)
        ):
            blocked.append(segment_id)
    return removed, retained, partial, blocked


def compute_liveness(root, keep_generations):
    live_objects = set()
    live_views = set()
    active_transaction_ids = set()
    running_generations = set()
    retained_generations = set()
    transaction_states = {}
    in_progress_generations = []
    stale_running_refs = []

    for tx in sorted((Path(root) / "transactions").glob("*.json")):
        data = read_json(tx)
        txid = data.get("id") or tx.stem
        transaction_states[txid] = data.get("status")
        if data.get("status") not in ("committed", "aborted"):
            active_transaction_ids.add(txid)

    for manifest_path in sorted(
        (Path(root) / "domains").glob("*/generations/*/manifest.json")
    ):
        try:
            manifest = read_json(manifest_path)
        except (OSError, json.JSONDecodeError):
            continue
        if manifest.get("schema") != SCHEMA_GENERATION or generation_is_committed(
            root, manifest
        ):
            continue
        txid = manifest.get("transaction")
        tx_status = transaction_states.get(txid) or transaction_status(root, txid)
        if not txid or tx_status in (None, "aborted"):
            continue
        active_transaction_ids.add(txid)
        domain = manifest.get("domain", manifest_path.parents[2].name)
        generation = int(manifest.get("generation", manifest_path.parent.name))
        in_progress_generations.append(
            {
                "domain": domain,
                "generation": generation,
                "transaction": txid,
                "transaction_status": tx_status,
                "manifest": str(manifest_path),
            }
        )
        add_manifest_store_liveness(root, manifest, live_objects, live_views)

    for domain, generation, ref_path, _ref in iter_running_generation_refs(root):
        manifest_path = generation_manifest_path(root, domain, generation)
        if manifest_path.exists():
            manifest = read_json(manifest_path)
            if generation_is_committed(root, manifest):
                running_generations.add((domain, generation))
                continue
        stale_running_refs.append(str(ref_path))

    by_domain = {}
    for domain, generation, manifest_path, manifest in iter_committed_generations(root):
        by_domain.setdefault(domain, []).append((generation, manifest_path, manifest))

    for domain, generations in by_domain.items():
        current = current_generation(root, domain)
        ordered = sorted(generations, key=lambda item: item[0], reverse=True)
        keep = {gen for gen, _, _ in ordered[:keep_generations]}
        if current is not None:
            keep.add(current)
        for gen, _manifest_path, manifest in generations:
            if (
                gen in keep
                or (domain, gen) in running_generations
                or manifest.get("retention", {}).get("retain", False)
            ):
                retained_generations.add((domain, gen))
                add_manifest_store_liveness(root, manifest, live_objects, live_views)

    if active_transaction_ids:
        for meta in object_blob_meta_paths(root):
            payload = read_json(meta)
            if payload.get("transaction") in active_transaction_ids:
                live_objects.add(payload.get("entry") or meta.name[: -len(".json")])

    return {
        "live_objects": live_objects,
        "live_views": live_views,
        "active_transaction_ids": active_transaction_ids,
        "running_generations": running_generations,
        "retained_generations": retained_generations,
        "in_progress_generations": in_progress_generations,
        "stale_running_refs": stale_running_refs,
    }


def current_live_locations(root, live_objects, live_views):
    locations = {}
    for meta_path in object_blob_meta_paths(root):
        entry = meta_path.name[: -len(".json")]
        if entry not in live_objects:
            continue
        payload = read_json(meta_path)
        extent = object_extent_from_meta(root, payload, f"canonical object {entry}")
        locations[entry] = {
            "entry": entry,
            "meta_path": str(meta_path),
            "payload": payload,
            "extent": extent,
        }
    if live_views and tar_eof_meta_path(root).exists():
        payload = read_json(tar_eof_meta_path(root))
        extent = object_extent_from_meta(root, payload, "canonical tar EOF")
        locations[".tar-eof"] = {
            "entry": ".tar-eof",
            "meta_path": str(tar_eof_meta_path(root)),
            "payload": payload,
            "extent": extent,
        }
    return locations


def segment_utilization(root, live_locations, reference_details):
    by_segment = {segment["id"]: dict(segment) for segment in collect_segments(root)}
    for segment in by_segment.values():
        segment["current_live_bytes"] = 0
        segment["current_live_entries"] = []
        segment["physical_reference_count"] = len(
            reference_details.get(segment["id"], [])
        )
        segment["blocked_by_physical_refs"] = bool(
            reference_details.get(segment["id"], [])
        )
    for entry, info in live_locations.items():
        extent = info["extent"]
        segment = by_segment.get(extent["segment_id"])
        if not segment:
            continue
        segment["current_live_bytes"] += int(extent["size"])
        segment["current_live_entries"].append(entry)
    for segment in by_segment.values():
        used = int(segment.get("used_bytes", 0))
        live = int(segment.get("current_live_bytes", 0))
        segment["dead_bytes"] = max(used - live, 0)
        segment["utilization_percent"] = (
            100.0 if used == 0 else round((live * 100.0) / used, 2)
        )
        segment["current_live_entries"] = sorted(segment["current_live_entries"])
    return [by_segment[key] for key in sorted(by_segment, key=lambda value: int(value))]


def compact_candidates(segments, threshold_percent, max_segments):
    current = None
    candidates = []
    for segment in segments:
        if not segment.get("sealed"):
            continue
        if segment.get("oversized"):
            continue
        if current and segment["id"] == current:
            continue
        live = int(segment.get("current_live_bytes", 0))
        used = int(segment.get("used_bytes", 0))
        if live <= 0 or used <= 0:
            continue
        if float(segment.get("utilization_percent", 100.0)) > threshold_percent:
            continue
        candidates.append(segment)
    candidates.sort(
        key=lambda item: (
            float(item.get("utilization_percent", 100.0)),
            int(item["id"]),
        )
    )
    return candidates[:max_segments]


def copy_location_to_tmp(root, entry, location, txid):
    root = Path(root)
    tmp = root / "tmp" / f"compact-{entry}-{txid}"
    if tmp.exists():
        remove_path(tmp)
    segment_id = validate_segment_id(location["segment_id"])
    remaining = int(location["size"])
    digest = hashlib.sha256()
    with (
        segment_blob_path(root, segment_id).open("rb") as source,
        tmp.open("xb") as target,
    ):
        source.seek(int(location["segment_offset"]))
        while remaining:
            data = source.read(min(1024 * 1024, remaining))
            if not data:
                fail(f"cannot compact {entry}: source segment is truncated")
            remaining -= len(data)
            digest.update(data)
            target.write(data)
        target.flush()
        if closure_store_fsync_enabled():
            os.fsync(target.fileno())
    if digest.hexdigest() != location["sha256"]:
        remove_path(tmp)
        fail(f"cannot compact {entry}: source hash mismatch")
    os.chmod(tmp, 0o400)
    return tmp


def existing_valid_alternate_location(root, payload, excluded_segments, label):
    for raw_location in payload.get("locations", []):
        location = normalize_location(raw_location, label)
        if location.get("status") != "current":
            continue
        if location["segment_id"] in excluded_segments:
            continue
        with suppress(EngineError):
            verify_segment_region(
                root,
                location["segment_id"],
                location["segment_offset"],
                location["size"],
                location["sha256"],
                label,
            )
            return location
    return None


def publish_blob_location(root, entry, new_location, txid, reason):
    root = Path(root)
    meta_path = (
        tar_eof_meta_path(root)
        if entry == ".tar-eof"
        else object_blob_meta_path(root, entry)
    )
    payload = read_json(meta_path)
    old_current = current_location_from_meta(payload, f"canonical object {entry}")
    new_location = normalize_location(
        dict(new_location, status="current", transaction=txid),
        f"canonical object {entry}",
    )
    if location_key(old_current) == location_key(new_location):
        return False

    locations = []
    seen = set()
    for raw_location in payload.get("locations", [old_current]):
        location = normalize_location(raw_location, f"canonical object {entry}")
        key = location_key(location)
        if key in seen:
            continue
        seen.add(key)
        if location.get("status") == "current":
            location["status"] = "retired"
            location["retired_at"] = utc_now()
            location["retired_by_transaction"] = txid
        locations.append(location)
    if location_key(new_location) not in seen:
        new_location["created_at"] = utc_now()
        new_location["reason"] = reason
        locations.append(new_location)
    else:
        locations = [
            (
                dict(location, status="current", transaction=txid, reason=reason)
                if location_key(location) == location_key(new_location)
                else location
            )
            for location in locations
        ]

    payload.update(top_level_location_fields(root, new_location))
    payload["locations"] = locations
    payload["relocated_at"] = utc_now()
    payload["relocated_by_transaction"] = txid
    payload["transaction"] = txid
    atomic_json(meta_path, payload, mode=0o444)

    if entry != ".tar-eof":
        object_meta = object_meta_path(root, entry)
        if object_meta.exists():
            object_payload = read_json(object_meta)
            fragment = object_payload.setdefault("tar_fragment", {})
            fragment.update(top_level_location_fields(root, new_location))
            fragment["locations"] = locations
            fragment["relocated_at"] = payload["relocated_at"]
            object_payload["tree_materialized"] = False
            atomic_json(object_meta, object_payload)
    return True


def refresh_blob_map_locations(root, blob_map_path):
    root = Path(root)
    blob_map_path = Path(blob_map_path)
    if not blob_map_path.exists() or blob_map_path.is_symlink():
        return False
    payload = read_json(blob_map_path)
    if payload.get("view_backend") != VIEW_BACKEND_EROFS_OBJECTMAP:
        return False
    changed = False
    segments = {}
    for extent in payload.get("extents", []):
        entry = extent.get("object")
        if not entry:
            continue
        if entry == ".tar-eof":
            if not tar_eof_meta_path(root).exists():
                continue
            meta = read_json(tar_eof_meta_path(root))
            current = object_extent_from_meta(root, meta, "canonical tar EOF")
        else:
            meta_path = object_blob_meta_path(root, entry)
            if not meta_path.exists():
                continue
            current = object_extent_from_meta(
                root, read_json(meta_path), f"canonical object {entry}"
            )
        if int(extent["length"]) != int(current["size"]):
            fail(
                f"cannot refresh blob map {blob_map_path}: object size changed for {entry}"
            )
        if extent.get("object_sha256") != current["sha256"]:
            fail(
                f"cannot refresh blob map {blob_map_path}: object hash changed for {entry}"
            )
        for key in ["segment_id", "segment_blob", "segment_offset"]:
            if extent.get(key) != current[key]:
                changed = True
        extent["segment_id"] = current["segment_id"]
        extent["segment_blob"] = current["segment_blob"]
        extent["segment_offset"] = int(current["segment_offset"])
        segments[current["segment_id"]] = {
            "id": current["segment_id"],
            "blob": current["segment_blob"],
            "size": os.path.getsize(current["segment_blob"]),
            "status": read_segment_meta(root, current["segment_id"]).get(
                "status", "unknown"
            ),
        }
    new_segments = [
        segments[key] for key in sorted(segments, key=lambda value: int(value))
    ]
    if payload.get("segments") != new_segments or payload.get("segment_count") != len(
        new_segments
    ):
        changed = True
    payload["segments"] = new_segments
    payload["segment_count"] = len(new_segments)
    if changed:
        payload["refreshed_at"] = utc_now()
        atomic_json(blob_map_path, payload, mode=0o444)
    return changed


def refresh_all_blob_maps(root):
    refreshed = []
    for blob_map in sorted(
        (Path(root) / "views/by-closure").glob("*/store.blob-map.json")
    ):
        if refresh_blob_map_locations(root, blob_map):
            refreshed.append(str(blob_map))
    for blob_map in sorted(
        (Path(root) / "domains").glob("*/generations/*/store.blob-map.json")
    ):
        if blob_map.is_symlink():
            continue
        if refresh_blob_map_locations(root, blob_map):
            refreshed.append(str(blob_map))
    return refreshed


def retire_unreferenced_segments(root, live_objects, live_views):
    live_segment_refs = live_segment_references(root, live_objects, live_views)
    physical_refs = physical_segment_reference_details(root, live_objects, live_views)
    before = {
        segment["id"]: int(segment["used_bytes"]) for segment in collect_segments(root)
    }
    removed, retained, partial, blocked = gc_segments(
        root, live_segment_refs, physical_refs
    )
    return {
        "removed_segments": removed,
        "retained_segments": retained,
        "partially_live_segments": partial,
        "blocked_segments": blocked,
        "bytes_reclaimed": sum(before.get(segment_id, 0) for segment_id in removed),
    }


def compact(args):
    root = ensure_pool(args.pool_root)
    threshold = parse_positive_int(
        args.threshold_percent, "compaction threshold percent"
    )
    if threshold > 100:
        fail("compaction threshold percent must be <= 100")
    max_segments = parse_positive_int(args.max_segments, "compaction max segments")
    target_bytes = configured_segment_target_bytes(args.segment_target_bytes)
    dry_run = parse_bool(args.dry_run)
    keep_generations = int(args.keep_generations)

    liveness = compute_liveness(root, keep_generations)
    live_locations = current_live_locations(
        root, liveness["live_objects"], liveness["live_views"]
    )
    physical_refs = physical_segment_reference_details(
        root, liveness["live_objects"], liveness["live_views"]
    )
    segments = segment_utilization(root, live_locations, physical_refs)
    candidates = compact_candidates(segments, threshold, max_segments)
    zero_ref_retire = [
        segment
        for segment in segments
        if segment.get("sealed")
        and int(segment.get("current_live_bytes", 0)) == 0
        and not physical_refs.get(segment["id"])
    ]

    if dry_run:
        return {
            "schema": SCHEMA_COMPACTION,
            "status": "passed",
            "dry_run": True,
            "pool_root": str(root),
            "threshold_percent": threshold,
            "max_segments": max_segments,
            "segment_target_bytes": target_bytes,
            "segment_count": len(segments),
            "segments": segments,
            "candidate_segments": candidates,
            "directly_reclaimable_segments": zero_ref_retire,
            "objects_to_move": sum(
                len(segment.get("current_live_entries", [])) for segment in candidates
            ),
            "bytes_to_copy": sum(
                int(segment.get("current_live_bytes", 0)) for segment in candidates
            ),
            "bytes_reclaimable_now": sum(
                int(segment.get("used_bytes", 0)) for segment in zero_ref_retire
            ),
        }

    txid = uuid.uuid4().hex
    tx_path = transaction_path(root, txid)
    atomic_json(
        tx_path,
        {
            "schema": "nixome.closure-store.transaction.v1",
            "id": txid,
            "type": "compaction",
            "status": "preparing",
            "created_at": utc_now(),
            "candidate_segments": [segment["id"] for segment in candidates],
        },
    )
    maybe_failpoint("compact-after-tx-create")

    with segment_lock_path(root).open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        current = current_open_segment_locked(root)
        if current is not None:
            current_segment_id, current_meta = current
            if int(current_meta.get("object_count", 0)) > 0:
                seal_segment_locked(
                    root, current_segment_id, txid, "pre-compaction-open-segment"
                )
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    moved = []
    reused = []
    new_segments = set()
    bytes_copied = 0
    excluded_segments = {segment["id"] for segment in candidates}
    for segment in candidates:
        for entry in segment.get("current_live_entries", []):
            info = live_locations.get(entry)
            if not info:
                continue
            old_extent = info["extent"]
            if old_extent["segment_id"] != segment["id"]:
                continue
            alternate = existing_valid_alternate_location(
                root, info["payload"], excluded_segments, f"canonical object {entry}"
            )
            if alternate:
                maybe_failpoint("compact-before-location-publish")
                if publish_blob_location(
                    root, entry, alternate, txid, "compact-reuse-existing-location"
                ):
                    reused.append(
                        {
                            "entry": entry,
                            "segment_id": alternate["segment_id"],
                            "segment_offset": alternate["segment_offset"],
                        }
                    )
                maybe_failpoint("compact-after-location-publish")
                continue
            tmp = copy_location_to_tmp(root, entry, old_extent, txid)
            maybe_failpoint("compact-after-object-copy")
            try:
                new_extent = append_file_to_segment(
                    root, tmp, entry, txid, target_bytes
                )
                verify_segment_region(
                    root,
                    new_extent["segment_id"],
                    new_extent["segment_offset"],
                    new_extent["size"],
                    new_extent["sha256"],
                    f"compacted object {entry}",
                )
                maybe_failpoint("compact-before-location-publish")
                publish_blob_location(root, entry, new_extent, txid, "compact-copy")
                maybe_failpoint("compact-after-location-publish")
                new_segments.add(new_extent["segment_id"])
                bytes_copied += int(new_extent["size"])
                moved.append(
                    {
                        "entry": entry,
                        "old_segment_id": old_extent["segment_id"],
                        "old_segment_offset": old_extent["segment_offset"],
                        "new_segment_id": new_extent["segment_id"],
                        "new_segment_offset": new_extent["segment_offset"],
                        "size": int(new_extent["size"]),
                        "sha256": new_extent["sha256"],
                    }
                )
            finally:
                with suppress(Exception):
                    remove_path(tmp)

    for segment_id in sorted(new_segments, key=lambda value: int(value)):
        with segment_lock_path(root).open("a+", encoding="utf-8") as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
            meta = read_segment_meta(root, segment_id)
            if meta.get("status") == "open" and not meta.get("sealed"):
                maybe_failpoint("compact-before-replacement-seal")
                seal_segment_locked(root, segment_id, txid, "compaction-replacement")
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    refreshed = refresh_all_blob_maps(root)
    maybe_failpoint("compact-after-view-refresh")
    atomic_json(
        tx_path,
        {
            "schema": "nixome.closure-store.transaction.v1",
            "id": txid,
            "type": "compaction",
            "status": "committed",
            "committed_at": utc_now(),
            "candidate_segments": [segment["id"] for segment in candidates],
            "objects_moved": len(moved),
            "bytes_copied": bytes_copied,
        },
    )
    maybe_failpoint("compact-after-tx-commit")

    liveness_after = compute_liveness(root, keep_generations)
    retirement = retire_unreferenced_segments(
        root,
        liveness_after["live_objects"],
        liveness_after["live_views"],
    )
    maybe_failpoint("compact-after-retirement")

    write_amplification = (
        0.0
        if bytes_copied == 0
        else round(
            bytes_copied
            / max(sum(int(segment.get("dead_bytes", 0)) for segment in candidates), 1),
            4,
        )
    )
    return {
        "schema": SCHEMA_COMPACTION,
        "status": "passed",
        "dry_run": False,
        "pool_root": str(root),
        "transaction": txid,
        "threshold_percent": threshold,
        "max_segments": max_segments,
        "segment_target_bytes": target_bytes,
        "candidate_segments": candidates,
        "objects_moved": moved,
        "objects_reused": reused,
        "bytes_copied": bytes_copied,
        "bytes_reclaimed": retirement["bytes_reclaimed"],
        "segments_retired": retirement["removed_segments"],
        "segments_blocked_from_deletion": retirement["blocked_segments"],
        "views_refreshed": refreshed,
        "write_amplification": write_amplification,
        "segment_compaction": "copy-and-switch",
    }


def recover(args):
    root = ensure_pool(args.pool_root)
    aborted = []
    finalized = []
    removed_incomplete_generations = []
    removed_runtime_refs = []
    sealed_aborted_segments = []
    refreshed_blob_maps = []
    for tx in sorted((root / "transactions").glob("*.json")):
        data = read_json(tx)
        if data.get("status") == "committed":
            manifest_path = data.get("manifest")
            if manifest_path and Path(manifest_path).exists():
                manifest = read_json(manifest_path)
                if (
                    manifest.get("schema") == SCHEMA_GENERATION
                    and manifest.get("status") == "prepared"
                ):
                    manifest["status"] = "committed"
                    manifest["recovered_at"] = utc_now()
                    atomic_json(manifest_path, manifest)
                    finalized.append(str(manifest_path))
            continue
        data["status"] = "aborted"
        data["aborted_at"] = utc_now()
        atomic_json(tx, data)
        aborted.append(str(tx))
    for tmp in sorted((root / "tmp").iterdir()):
        if tmp.is_dir():
            remove_path(tmp)
        else:
            remove_path(tmp)
    for ref_path, ref in list(iter_runtime_view_refs(root)):
        dm_name = ref.get("dm_name")
        if not dm_name:
            remove_path(ref_path)
            removed_runtime_refs.append(str(ref_path))
            continue
        dm_present = True
        with suppress(EngineError):
            dm_present = dm_exists(dm_name)
        if not dm_present:
            remove_path(ref_path)
            removed_runtime_refs.append(str(ref_path))
    aborted_ids = {Path(path).stem for path in aborted}
    for segment in collect_segments(root):
        if segment.get("sealed"):
            continue
        meta = read_segment_meta(root, segment["id"])
        if meta.get("created_by_transaction") not in aborted_ids:
            continue
        with segment_lock_path(root).open("a+", encoding="utf-8") as lock:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
            meta = read_segment_meta(root, segment["id"])
            if not meta.get("sealed"):
                seal_segment_locked(
                    root,
                    segment["id"],
                    meta.get("created_by_transaction"),
                    "recovery-after-aborted-transaction",
                )
                sealed_aborted_segments.append(segment["id"])
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    for domain_dir in sorted((root / "domains").iterdir()):
        if not domain_dir.is_dir():
            continue
        for stale_link in domain_dir.glob(".current.tmp-*"):
            if stale_link.is_dir() and not stale_link.is_symlink():
                remove_path(stale_link)
            else:
                remove_path(stale_link)
        generations_dir = domain_dir / "generations"
        if not generations_dir.is_dir():
            continue
        for generation_dir in sorted(generations_dir.iterdir()):
            if not generation_dir.is_dir():
                continue
            manifest_path = generation_dir / "manifest.json"
            if not manifest_path.exists():
                remove_path(generation_dir)
                removed_incomplete_generations.append(str(generation_dir))
                continue
            manifest = read_json(manifest_path)
            if not generation_is_committed(root, manifest):
                remove_path(generation_dir)
                removed_incomplete_generations.append(str(generation_dir))
    refreshed_blob_maps = refresh_all_blob_maps(root)
    return {
        "schema": SCHEMA_RECOVERY,
        "status": "passed",
        "pool_root": str(root),
        "aborted_transactions": aborted,
        "finalized_generations": finalized,
        "removed_incomplete_generations": removed_incomplete_generations,
        "removed_runtime_refs": removed_runtime_refs,
        "sealed_aborted_segments": sealed_aborted_segments,
        "refreshed_blob_maps": refreshed_blob_maps,
    }


def gc(args):
    root = ensure_pool(args.pool_root)
    live_objects = set()
    live_views = set()
    active_transaction_ids = set()
    active_transaction_objects = set()
    in_progress_generations = []
    running_generations = set()
    stale_running_refs = []
    retained_generations = set()
    removed_generations = []
    transaction_states = {}

    for tx in sorted((root / "transactions").glob("*.json")):
        data = read_json(tx)
        txid = data.get("id") or tx.stem
        transaction_states[txid] = data.get("status")
        if data.get("status") not in ("committed", "aborted"):
            active_transaction_ids.add(txid)

    for manifest_path in sorted(
        (root / "domains").glob("*/generations/*/manifest.json")
    ):
        try:
            manifest = read_json(manifest_path)
        except (OSError, json.JSONDecodeError):
            continue
        if manifest.get("schema") != SCHEMA_GENERATION or generation_is_committed(
            root, manifest
        ):
            continue
        txid = manifest.get("transaction")
        tx_status = transaction_states.get(txid) or transaction_status(root, txid)
        if not txid or tx_status in (None, "aborted"):
            continue
        active_transaction_ids.add(txid)
        domain = manifest.get("domain", manifest_path.parents[2].name)
        generation = int(manifest.get("generation", manifest_path.parent.name))
        in_progress_generations.append(
            {
                "domain": domain,
                "generation": generation,
                "transaction": txid,
                "transaction_status": tx_status,
                "manifest": str(manifest_path),
            }
        )
        add_manifest_store_liveness(root, manifest, live_objects, live_views)

    if active_transaction_ids:
        for meta in sorted((root / "objects/blob-metadata").glob("*.json")):
            if meta.name == ".tar-eof.json":
                continue
            payload = read_json(meta)
            if payload.get("transaction") in active_transaction_ids:
                active_transaction_objects.add(
                    payload.get("entry") or meta.name[: -len(".json")]
                )

    live_objects.update(active_transaction_objects)

    for domain, generation, ref_path, _ref in iter_running_generation_refs(root):
        manifest_path = generation_manifest_path(root, domain, generation)
        if manifest_path.exists():
            manifest = read_json(manifest_path)
            if generation_is_committed(root, manifest):
                running_generations.add((domain, generation))
                continue
        stale_running_refs.append(str(ref_path))

    by_domain = {}
    for domain, generation, manifest_path, manifest in iter_committed_generations(root):
        by_domain.setdefault(domain, []).append((generation, manifest_path, manifest))

    for domain, generations in by_domain.items():
        current = current_generation(root, domain)
        ordered = sorted(generations, key=lambda item: item[0], reverse=True)
        keep = {gen for gen, _, _ in ordered[: args.keep_generations]}
        if current is not None:
            keep.add(current)
        for gen, manifest_path, manifest in generations:
            if (
                gen in keep
                or (domain, gen) in running_generations
                or manifest.get("retention", {}).get("retain", False)
            ):
                retained_generations.add((domain, gen))
                add_manifest_store_liveness(root, manifest, live_objects, live_views)
            else:
                generation_dir = manifest_path.parent
                remove_path(generation_dir)
                removed_generations.append({"domain": domain, "generation": gen})
                maybe_failpoint("gc-after-generation-remove")

    removed_object_metadata = []
    for meta in sorted((root / "objects/metadata").glob("*.json")):
        entry = meta.name[: -len(".json")]
        if entry not in live_objects:
            meta.unlink()
            removed_object_metadata.append(entry)

    removed_object_blobs = []
    for meta in sorted((root / "objects/blob-metadata").glob("*.json")):
        if meta.name == ".tar-eof.json":
            continue
        entry = meta.name[: -len(".json")]
        if entry not in live_objects:
            meta.unlink()
            removed_object_blobs.append(entry)
            maybe_failpoint("gc-after-object-metadata-remove")

    removed_special_extents = []
    if not live_views and tar_eof_meta_path(root).exists():
        remove_path(tar_eof_meta_path(root))
        removed_special_extents.append(".tar-eof")

    removed_views = []
    for view_dir in sorted((root / "views/by-closure").iterdir()):
        if str(view_dir) not in live_views:
            remove_path(view_dir)
            removed_views.append(view_dir.name)

    live_segment_refs = live_segment_references(root, live_objects, live_views)
    physical_refs = physical_segment_reference_details(root, live_objects, live_views)
    runtime_refs, stale_runtime_view_refs = runtime_segment_references(root)
    removed_segments, retained_segments, partially_live_segments, blocked_segments = (
        gc_segments(root, live_segment_refs, physical_refs)
    )

    return {
        "schema": SCHEMA_GC,
        "status": "passed",
        "pool_root": str(root),
        "retained_generations": [
            {"domain": domain, "generation": generation}
            for domain, generation in sorted(retained_generations)
        ],
        "running_generations": [
            {"domain": domain, "generation": generation}
            for domain, generation in sorted(running_generations)
        ],
        "stale_running_refs": stale_running_refs,
        "stale_runtime_view_refs": stale_runtime_view_refs,
        "in_progress_generations": in_progress_generations,
        "active_transaction_ids": sorted(active_transaction_ids),
        "active_transaction_objects": sorted(active_transaction_objects),
        "removed_generations": removed_generations,
        "removed_object_metadata": removed_object_metadata,
        "removed_object_blobs": removed_object_blobs,
        "removed_special_extents": removed_special_extents,
        "removed_views": removed_views,
        "segments_dir": str(segments_dir(root)),
        "removed_segments": removed_segments,
        "retained_segments": retained_segments,
        "partially_live_segments": partially_live_segments,
        "blocked_segments": blocked_segments,
        "runtime_segment_reference_count": sum(
            len(refs) for refs in runtime_refs.values()
        ),
        "segment_store_append_only": True,
        "segment_compaction": "available",
    }


def status(args):
    root = ensure_pool(args.pool_root)
    liveness = compute_liveness(root, 2)
    physical_refs = physical_segment_reference_details(
        root, liveness["live_objects"], liveness["live_views"]
    )
    live_locations = current_live_locations(
        root, liveness["live_objects"], liveness["live_views"]
    )
    domains = {}
    for domain, generation, manifest_path, manifest in iter_committed_generations(root):
        domains.setdefault(
            domain, {"current": current_generation(root, domain), "generations": []}
        )
        domains[domain]["generations"].append(
            {
                "generation": generation,
                "manifest": str(manifest_path),
                "system": manifest["system"],
                "closure_hash": manifest["store"]["closure_hash"],
                "object_count": manifest["store"]["object_count"],
                "filesystem": manifest["store"].get("filesystem", "erofs"),
                "view_backend": manifest["store"].get(
                    "view_backend", VIEW_BACKEND_EROFS_OBJECTMAP
                ),
            }
        )
    running = [
        {
            "domain": domain,
            "generation": generation,
            "volume": ref.get("volume"),
            "manifest": ref.get("manifest"),
            "closure_hash": ref.get("closure_hash", ""),
        }
        for domain, generation, _ref_path, ref in iter_running_generation_refs(root)
    ]
    object_count = len(
        [
            path
            for path in (root / "objects/metadata").glob("*.json")
            if path.name != ".tar-eof.json"
        ]
    )
    object_blob_count = len(
        [
            path
            for path in (root / "objects/blob-metadata").glob("*.json")
            if path.name != ".tar-eof.json"
        ]
    )
    segments = segment_utilization(root, live_locations, physical_refs)
    runtime_refs, stale_runtime_view_refs = runtime_segment_references(root)
    return {
        "schema": SCHEMA_STATUS,
        "status": "passed",
        "pool_root": str(root),
        "object_count": object_count,
        "object_blob_count": object_blob_count,
        "object_segment_count": object_blob_count,
        "segments_dir": str(segments_dir(root)),
        "segment_count": len(segments),
        "sealed_segment_count": len(
            [segment for segment in segments if segment["sealed"]]
        ),
        "open_segment_count": len(
            [segment for segment in segments if not segment["sealed"]]
        ),
        "segment_bytes_total": sum(int(segment["used_bytes"]) for segment in segments),
        "segments": segments,
        "domains": domains,
        "running": running,
        "runtime_view_reference_count": sum(
            len(refs) for refs in runtime_refs.values()
        ),
        "stale_runtime_view_refs": stale_runtime_view_refs,
        "segment_compaction": "available",
    }


def add_common(parser):
    parser.add_argument(
        "--pool-root",
        default=os.environ.get(
            "NIXOME_CLOSURE_STORE_ROOT", "/var/lib/nixome/closure-store"
        ),
    )


def build_parser():
    parser = argparse.ArgumentParser(prog="nixome-closure-store")
    sub = parser.add_subparsers(dest="command", required=True)

    admit_parser = sub.add_parser("admit")
    add_common(admit_parser)
    admit_parser.add_argument("--domain", required=True)
    admit_parser.add_argument("--system", required=True)
    admit_parser.add_argument("--closure-source", required=True)
    admit_parser.add_argument("--root-source", default="")
    admit_parser.add_argument("--kernel-name", default="")
    admit_parser.add_argument("--store-device", default="/dev/xvde")
    admit_parser.add_argument("--store-blob-device", default="/dev/xvdf")
    admit_parser.add_argument("--store-mountpoint", default="/nix/store")
    admit_parser.add_argument(
        "--physical-store-root", default=default_physical_store_root()
    )
    admit_parser.add_argument(
        "--segment-target-bytes",
        default=os.environ.get("NIXOME_CLOSURE_STORE_SEGMENT_TARGET_BYTES", ""),
        help="target maximum size for open immutable object segments",
    )
    admit_parser.add_argument(
        "--verify-mode",
        default=os.environ.get("NIXOME_CLOSURE_STORE_VERIFY", "nix-store"),
        choices=["none", "tree", "nix-store", "nix-store-trust"],
    )
    admit_parser.add_argument(
        "--view-backend",
        default=os.environ.get(
            "NIXOME_CLOSURE_STORE_VIEW_BACKEND", DEFAULT_VIEW_BACKEND
        ),
        choices=[VIEW_BACKEND_EROFS_OBJECTMAP],
    )
    admit_parser.add_argument("--approved-by", default="dom0")
    admit_parser.add_argument("--approval-source", default="")
    admit_parser.add_argument(
        "--activate",
        default="true",
        choices=["true", "false"],
        help="whether to atomically select the new generation after admission",
    )
    admit_parser.add_argument(
        "--retain",
        default="true",
        choices=["true", "false"],
        help="whether GC must retain this generation even after it stops being current",
    )
    admit_parser.set_defaults(func=admit_generation)

    rollback_parser = sub.add_parser("rollback")
    add_common(rollback_parser)
    rollback_parser.add_argument("--domain", required=True)
    rollback_parser.add_argument("--generation", required=True)
    rollback_parser.set_defaults(func=rollback)

    retention_parser = sub.add_parser("set-retention")
    add_common(retention_parser)
    retention_parser.add_argument("--domain", required=True)
    retention_parser.add_argument("--generation", required=True)
    retention_parser.add_argument("--retain", required=True, choices=["true", "false"])
    retention_parser.set_defaults(func=set_retention)

    switch_parser = sub.add_parser("switch")
    add_common(switch_parser)
    switch_parser.add_argument("--domain", required=True)
    switch_parser.add_argument("--generation", required=True)
    switch_parser.set_defaults(func=switch)

    gc_parser = sub.add_parser("gc")
    add_common(gc_parser)
    gc_parser.add_argument("--keep-generations", type=int, default=2)
    gc_parser.set_defaults(func=gc)

    compact_parser = sub.add_parser("compact")
    add_common(compact_parser)
    compact_parser.add_argument(
        "--dry-run",
        default="false",
        choices=["true", "false"],
        help="report compaction candidates without moving objects",
    )
    compact_parser.add_argument(
        "--threshold-percent",
        default=str(DEFAULT_COMPACTION_THRESHOLD_PERCENT),
        help="compact sealed non-oversized segments at or below this live-byte utilization",
    )
    compact_parser.add_argument(
        "--max-segments",
        default=str(DEFAULT_COMPACTION_MAX_SEGMENTS),
        help="maximum candidate segments to compact in one bounded run",
    )
    compact_parser.add_argument(
        "--keep-generations",
        type=int,
        default=2,
        help="latest generations per domain to treat as live for compaction planning",
    )
    compact_parser.add_argument(
        "--segment-target-bytes",
        default=os.environ.get("NIXOME_CLOSURE_STORE_SEGMENT_TARGET_BYTES", ""),
        help="target maximum size for replacement object segments",
    )
    compact_parser.set_defaults(func=compact)

    recover_parser = sub.add_parser("recover")
    add_common(recover_parser)
    recover_parser.set_defaults(func=recover)

    status_parser = sub.add_parser("status")
    add_common(status_parser)
    status_parser.set_defaults(func=status)

    attach_parser = sub.add_parser("attach-blob-view")
    add_common(attach_parser)
    attach_parser.add_argument("--domain", required=True)
    attach_parser.add_argument("--generation", required=True)
    attach_parser.set_defaults(
        func=lambda args: attach_blob_view(args.pool_root, args.domain, args.generation)
    )

    detach_parser = sub.add_parser("detach-blob-view")
    add_common(detach_parser)
    detach_parser.add_argument("--domain", required=True)
    detach_parser.add_argument("--generation", required=True)
    detach_parser.set_defaults(
        func=lambda args: detach_blob_view(args.pool_root, args.domain, args.generation)
    )

    mark_running_parser = sub.add_parser("mark-running")
    add_common(mark_running_parser)
    mark_running_parser.add_argument("--domain", required=True)
    mark_running_parser.add_argument("--generation", required=True)
    mark_running_parser.add_argument("--volume", required=True)
    mark_running_parser.set_defaults(func=mark_running)

    clear_running_parser = sub.add_parser("clear-running")
    add_common(clear_running_parser)
    clear_running_parser.add_argument("--domain", required=True)
    clear_running_parser.add_argument("--generation", required=True)
    clear_running_parser.add_argument("--volume", required=True)
    clear_running_parser.set_defaults(func=clear_running)

    return parser


def run(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)
