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
SCHEMA_RECOVERY = "nixome.closure-store.recovery.v1"
SCHEMA_STATUS = "nixome.closure-store.status.v1"
SCHEMA_BLOB_MAP = "nixome.closure-store.blob-map.v1"
SCHEMA_OBJECT_BLOB = "nixome.closure-store.object-tar-fragment.v1"
SCHEMA_RUNNING_REF = "nixome.closure-store.running-ref.v1"

VIEW_BACKEND_EXT4_IMAGE = "ext4-image"
VIEW_BACKEND_EROFS_BLOBMAP = "erofs-blobmap"
VIEW_BACKEND_EROFS_OBJECTMAP = "erofs-objectmap"
EROFS_FILTERED_BLOB_BACKENDS = {
    VIEW_BACKEND_EROFS_BLOBMAP,
    VIEW_BACKEND_EROFS_OBJECTMAP,
}
DEFAULT_VIEW_BACKEND = VIEW_BACKEND_EROFS_OBJECTMAP
DEFAULT_CHUNK_SIZE = 64 * 1024
OBJECT_BLOB_ALIGNMENT = 4096
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


def copy_or_link_tree(src, dst):
    src = Path(src)
    dst = Path(dst)
    if src.is_symlink():
        os.symlink(os.readlink(src), dst)
        return
    if src.is_file():
        try:
            os.link(src, dst)
        except OSError:
            shutil.copy2(src, dst, follow_symlinks=False)
        return
    if not src.is_dir():
        fail(f"unsupported path type while copying {src}")
    dst.mkdir()
    for child in sorted(src.iterdir()):
        copy_or_link_tree(child, dst / child.name)
    shutil.copystat(src, dst, follow_symlinks=False)


def copy_tree(src, dst):
    src = Path(src)
    dst = Path(dst)
    if src.is_symlink():
        os.symlink(os.readlink(src), dst)
        return
    if src.is_file():
        shutil.copy2(src, dst, follow_symlinks=False)
        return
    if not src.is_dir():
        fail(f"unsupported path type while copying {src}")
    dst.mkdir()
    for child in sorted(src.iterdir()):
        copy_tree(child, dst / child.name)
    shutil.copystat(src, dst, follow_symlinks=False)


def replace_tree(src, dst):
    dst = Path(dst)
    tmp = dst.with_name("." + dst.name + ".tmp-" + uuid.uuid4().hex)
    if tmp.exists():
        remove_path(tmp)
    copy_tree(src, tmp)
    chmod_readonly(tmp)
    if dst.exists():
        remove_path(dst)
    os.replace(tmp, dst)
    fsync_dir(dst.parent)


def ensure_pool(root):
    root = Path(root)
    for subdir in [
        "objects/store",
        "objects/metadata",
        "objects/blob-metadata",
        "chunks/metadata",
        "domains",
        "images/by-closure",
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
                    "data": "objects/data.blob",
                    "metadata": "objects/blob-metadata",
                    "object_key": "nix-store-basename",
                    "format": "posix-tar-fragment-v1",
                },
                "chunk_store": {
                    "schema": "nixome.closure-store.chunk-store.v1",
                    "data": "chunks/data.blob",
                    "metadata": "chunks/metadata",
                    "object_key": "sha256",
                },
            },
        )
    return root


def parse_size(value, name):
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        fail(f"{name} must be an integer: {value!r}")
    if parsed <= 0:
        fail(f"{name} must be positive: {value!r}")
    return parsed


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


def object_path(root, entry):
    return Path(root) / "objects/store" / entry


def object_blob_path(root):
    return Path(root) / "objects" / "data.blob"


def object_blob_meta_path(root, entry):
    return Path(root) / "objects/blob-metadata" / f"{entry}.json"


def tar_eof_meta_path(root):
    return Path(root) / "objects/blob-metadata" / ".tar-eof.json"


def ensure_object_blob(root):
    path = object_blob_path(root)
    if not path.exists():
        with path.open("wb") as handle:
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(path, 0o600)
        fsync_dir(path.parent)
    return path


def verify_blob_region(blob_path, offset, size, digest, label):
    with Path(blob_path).open("rb") as handle:
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


def append_file_to_open_blob(backing, source, blob, txid):
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
        "central_blob": str(blob),
        "offset": offset,
        "size": size,
        "sha256": digest,
        "transaction": txid,
    }


def append_file_to_blob(root, source, txid):
    root = Path(root)
    blob = ensure_object_blob(root)
    lock_path = root / "objects" / "data.blob.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)

    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        with blob.open("r+b") as backing:
            result = append_file_to_open_blob(backing, source, blob, txid)
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    return result


def ensure_tar_eof_extent(root, txid):
    root = Path(root)
    meta_path = tar_eof_meta_path(root)
    digest = hash_bytes(b"\0" * TAR_EOF_SIZE)
    if meta_path.exists():
        meta = read_json(meta_path)
        blob = meta["central_blob"]
        verify_blob_region(blob, int(meta["offset"]), int(meta["size"]), digest, "canonical tar EOF")
        return meta

    tmp = root / "tmp" / f"tar-eof-{txid}"
    tmp.write_bytes(b"\0" * TAR_EOF_SIZE)
    os.chmod(tmp, 0o400)
    extent = append_file_to_blob(root, tmp, txid)
    remove_path(tmp)
    payload = {
        "schema": "nixome.closure-store.tar-eof.v1",
        "central_blob": extent["central_blob"],
        "offset": extent["offset"],
        "size": extent["size"],
        "sha256": extent["sha256"],
        "admitted_at": utc_now(),
        "transaction": txid,
    }
    atomic_json(meta_path, payload)
    return payload


def admit_object_tar_fragment(root, logical_path, source, source_hash, txid):
    root = Path(root)
    entry = store_entry(logical_path)
    meta = object_blob_meta_path(root, entry)
    if meta.exists():
        current = read_json(meta)
        if current.get("tree_hash") != source_hash:
            fail(f"existing object hash mismatch for {logical_path}")
        verify_blob_region(
            current["central_blob"],
            int(current["offset"]),
            int(current["size"]),
            current["sha256"],
            f"canonical object {entry}",
        )
        return dict(current, admitted=False)

    tmp = root / "tmp" / f"object-tar-{entry}-{txid}"
    if tmp.exists():
        remove_path(tmp)
    create_tar_fragment(source, entry, tmp)

    blob = ensure_object_blob(root)
    lock_path = root / "objects" / "data.blob.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        if meta.exists():
            fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
            remove_path(tmp)
            current = read_json(meta)
            if current.get("tree_hash") != source_hash:
                fail(f"existing object hash mismatch for {logical_path}")
            verify_blob_region(
                current["central_blob"],
                int(current["offset"]),
                int(current["size"]),
                current["sha256"],
                f"canonical object {entry}",
            )
            return dict(current, admitted=False)
        with blob.open("r+b") as backing:
            extent = append_file_to_open_blob(backing, tmp, blob, txid)
        maybe_failpoint("object-after-append")
        payload = {
            "schema": SCHEMA_OBJECT_BLOB,
            "path": logical_path,
            "entry": entry,
            "tree_hash": source_hash,
            "format": "posix-tar-fragment-v1",
            "central_blob": extent["central_blob"],
            "offset": extent["offset"],
            "size": extent["size"],
            "sha256": extent["sha256"],
            "admitted_at": utc_now(),
            "transaction": txid,
        }
        atomic_json(meta, payload)
        maybe_failpoint("object-after-metadata")
        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)
    remove_path(tmp)
    payload["admitted"] = True
    return payload


def ensure_object_tree_copy(root, logical_path, physical_store_root):
    entry = store_entry(logical_path)
    source = physical_store_path(logical_path, physical_store_root)
    if not source.exists():
        fail(f"store object source is missing: {logical_path} ({source})")

    source_hash = tree_hash(source)
    final = object_path(root, entry)
    meta = object_meta_path(root, entry)
    if final.exists():
        current_hash = tree_hash(final)
        if current_hash != source_hash:
            fail(f"existing object hash mismatch for {logical_path}")
        return {
            "tree_hash": current_hash,
            "tree_admitted": False,
        }

    tmp = Path(root) / "tmp" / f"object-tree-{entry}-{uuid.uuid4().hex}"
    if tmp.exists():
        remove_path(tmp)
    copy_tree(source, tmp)
    chmod_readonly(tmp)
    tmp_hash = tree_hash(tmp)
    if tmp_hash != source_hash:
        fail(f"copied object hash mismatch for {logical_path}")
    if not tmp.is_symlink():
        os.chmod(tmp, stat.S_IMODE(tmp.lstat().st_mode) | 0o700)
    try:
        os.replace(tmp, final)
    except FileExistsError:
        remove_path(tmp)
        return ensure_object_tree_copy(root, logical_path, physical_store_root)
    fsync_dir(final.parent)
    chmod_readonly(final)
    return {
        "tree_hash": source_hash,
        "tree_admitted": True,
    }


def admit_object(root, logical_path, physical_store_root, txid, materialize_tree=False):
    entry = store_entry(logical_path)
    source = physical_store_path(logical_path, physical_store_root)
    if not source.exists() and not source.is_symlink():
        fail(f"store object source is missing: {logical_path} ({source})")

    source_hash = tree_hash(source)
    meta = object_meta_path(root, entry)
    existing_meta = read_json(meta) if meta.exists() else {}
    if existing_meta and existing_meta.get("tree_hash") != source_hash:
        fail(f"existing object hash mismatch for {logical_path}")

    tar_fragment = admit_object_tar_fragment(root, logical_path, source, source_hash, txid)
    tree_copy = None
    if materialize_tree:
        tree_copy = ensure_object_tree_copy(root, logical_path, physical_store_root)

    admitted = not existing_meta or tar_fragment.get("admitted", False)
    payload = {
        "schema": "nixome.closure-store.object.v2",
        "path": logical_path,
        "entry": entry,
        "tree_hash": source_hash,
        "tar_fragment": {
            "schema": SCHEMA_OBJECT_BLOB,
            "central_blob": tar_fragment["central_blob"],
            "offset": int(tar_fragment["offset"]),
            "size": int(tar_fragment["size"]),
            "sha256": tar_fragment["sha256"],
            "format": tar_fragment.get("format", "posix-tar-fragment-v1"),
        },
        "tree_materialized": bool(tree_copy),
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


def link_object_into_view(root, entry, destination):
    source = object_path(root, entry)
    if not source.exists():
        fail(f"admitted object missing while creating view: {entry}")
    copy_or_link_tree(source, destination)


def tree_sizing(path):
    apparent = 0
    allocated = 0
    inodes = 0
    for current_root, dirs, files in os.walk(path):
        inodes += 1
        try:
            allocated += os.lstat(current_root).st_blocks * 512
        except OSError:
            pass
        for name in dirs + files:
            full = Path(current_root) / name
            try:
                st = os.lstat(full)
            except OSError:
                continue
            inodes += 1
            allocated += st.st_blocks * 512
            if stat.S_ISREG(st.st_mode):
                apparent += st.st_size
    return apparent, allocated, inodes


def create_ext4_image(view_dir, image_path, additional_space_mib):
    apparent, allocated, inodes = tree_sizing(view_dir)
    requested_spare = int(additional_space_mib) * 1024 * 1024
    metadata_spare = allocated // 2 + 128 * 1024 * 1024 + max(inodes, 8192) * 512
    spare = max(requested_spare, metadata_spare)
    image_bytes = apparent + spare
    image_bytes = ((image_bytes + 1048575) // 1048576) * 1048576
    inode_count = inodes + max(inodes // 2, 8192)
    tmp = image_path.with_name("." + image_path.name + ".tmp-" + uuid.uuid4().hex)
    with tmp.open("wb") as handle:
        handle.truncate(image_bytes)
    subprocess.run(
        [
            "mkfs.ext4",
            "-q",
            "-F",
            "-m",
            "0",
            "-L",
            "nixstore",
            "-N",
            str(inode_count),
            "-d",
            str(view_dir),
            str(tmp),
        ],
        check=True,
    )
    os.chmod(tmp, 0o444)
    os.replace(tmp, image_path)
    fsync_dir(image_path.parent)
    return {
        "used_apparent_bytes": apparent,
        "used_allocated_bytes": allocated,
        "used_inodes": inodes,
        "spare_bytes": spare,
        "inode_count": inode_count,
        "image_bytes": image_bytes,
    }


def chunk_blob_path(root):
    return Path(root) / "chunks" / "data.blob"


def chunk_meta_path(root, digest):
    return Path(root) / "chunks" / "metadata" / f"{digest}.json"


def ensure_chunk_blob(root):
    path = chunk_blob_path(root)
    if not path.exists():
        with path.open("wb") as handle:
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(path, 0o600)
        fsync_dir(path.parent)
    return path


def hash_bytes(data):
    return hashlib.sha256(data).hexdigest()


def verify_blob_chunk(blob_path, offset, size, digest):
    with Path(blob_path).open("rb") as handle:
        handle.seek(offset)
        data = handle.read(size)
    if len(data) != size:
        fail(f"central chunk blob is truncated at offset {offset}")
    if hash_bytes(data) != digest:
        fail(f"central chunk blob hash mismatch at offset {offset}")


def closure_store_fsync_enabled():
    return parse_bool(os.environ.get("NIXOME_CLOSURE_STORE_FSYNC", "1"))


def merge_blob_extents(extents):
    merged = []
    for extent in extents:
        if (
            merged
            and merged[-1]["logical_offset"] + merged[-1]["length"] == extent["logical_offset"]
            and merged[-1]["central_offset"] + merged[-1]["length"] == extent["central_offset"]
        ):
            merged[-1]["length"] += extent["length"]
        else:
            merged.append(dict(extent))
    return merged


def admit_blob_chunks(root, temp_blob, chunk_size, txid):
    root = Path(root)
    temp_blob = Path(temp_blob)
    chunk_size = parse_size(chunk_size, "chunk size")
    if chunk_size % 512 != 0:
        fail(f"chunk size must be 512-byte aligned: {chunk_size}")

    blob_size = temp_blob.stat().st_size
    if blob_size % 512 != 0:
        fail(f"temporary EROFS blob is not 512-byte aligned: {temp_blob}")

    lock_path = root / "chunks" / "data.blob.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    admitted = 0
    reused = 0
    chunk_refs = []
    extents = []
    pending_metadata = []
    blob_path = ensure_chunk_blob(root)

    with lock_path.open("a+", encoding="utf-8") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        with temp_blob.open("rb") as source, blob_path.open("r+b") as backing:
            logical_offset = 0
            while True:
                chunk = source.read(chunk_size)
                if not chunk:
                    break
                if len(chunk) % 512 != 0:
                    fail(f"temporary EROFS blob chunk is not 512-byte aligned at {logical_offset}")

                digest = hash_bytes(chunk)
                meta_path = chunk_meta_path(root, digest)
                if meta_path.exists():
                    meta = read_json(meta_path)
                    central_offset = int(meta["offset"])
                    central_size = int(meta["size"])
                    if central_size != len(chunk):
                        fail(f"central chunk size mismatch for {digest}")
                    verify_blob_chunk(blob_path, central_offset, central_size, digest)
                    reused += 1
                else:
                    backing.seek(0, os.SEEK_END)
                    central_offset = align_up(backing.tell(), 4096)
                    padding = central_offset - backing.tell()
                    if padding:
                        backing.write(b"\0" * padding)
                    backing.write(chunk)
                    pending_metadata.append(
                        {
                            "path": meta_path,
                            "payload": {
                                "schema": "nixome.closure-store.chunk.v1",
                                "hash": digest,
                                "offset": central_offset,
                                "size": len(chunk),
                                "admitted_at": utc_now(),
                                "transaction": txid,
                            },
                        }
                    )
                    admitted += 1

                chunk_refs.append(digest)
                extents.append(
                    {
                        "logical_offset": logical_offset,
                        "central_offset": central_offset,
                        "length": len(chunk),
                    }
                )
                logical_offset += len(chunk)

            if pending_metadata:
                backing.flush()
                if closure_store_fsync_enabled():
                    os.fsync(backing.fileno())

        for metadata in pending_metadata:
            atomic_json(metadata["path"], metadata["payload"])
        if pending_metadata:
            fsync_dir(root / "chunks" / "metadata")

        fcntl.flock(lock.fileno(), fcntl.LOCK_UN)

    return {
        "blob": str(blob_path),
        "blob_logical_size": blob_size,
        "chunk_size": chunk_size,
        "chunk_count": len(chunk_refs),
        "chunks_admitted": admitted,
        "chunks_reused": reused,
        "chunk_refs": chunk_refs,
        "extents": merge_blob_extents(extents),
    }


def write_blob_map(path, payload):
    atomic_json(path, payload, mode=0o444)


def write_chunk_refs(path, refs):
    atomic_write(path, "".join(f"{ref}\n" for ref in refs), mode=0o444)


def write_object_refs(path, refs):
    atomic_write(path, "".join(f"{ref}\n" for ref in refs), mode=0o444)


def read_chunk_refs(path):
    if not path:
        return []
    return [
        line.strip()
        for line in Path(path).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def read_object_refs(path):
    if not path:
        return []
    return [
        line.strip()
        for line in Path(path).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def add_manifest_store_liveness(root, manifest, live_objects, live_images, live_views, live_chunks):
    store = manifest.get("store", {})
    for path in store.get("paths", []):
        live_objects.add(store_entry(path))
    if store.get("view_backend", VIEW_BACKEND_EXT4_IMAGE) == VIEW_BACKEND_EXT4_IMAGE:
        image = store.get("canonical_image")
        if image:
            live_images.add(image)
    view_root = canonical_view_root(store)
    if view_root:
        live_views.add(view_root)
    for chunk in read_chunk_refs(store.get("canonical_chunk_refs") or store.get("chunk_refs")):
        live_chunks.add(chunk)


def canonical_view_root(store):
    for key in ("canonical_metadata_image", "canonical_blob_map", "canonical_object_refs", "canonical_chunk_refs"):
        value = store.get(key)
        if value:
            return str(Path(value).parent)
    canonical_image = store.get("canonical_image")
    if canonical_image and "/views/by-closure/" in canonical_image:
        return str(Path(canonical_image).parent)
    return None


def create_erofs_metadata_image(view_dir, metadata_path, temp_blob_path, chunk_size):
    mkfs_erofs = require_tool("mkfs.erofs", "NIXOME_MKFS_EROFS")
    if temp_blob_path.exists():
        temp_blob_path.unlink()
    with temp_blob_path.open("wb") as handle:
        handle.truncate(0)
    run_checked(
        [
            mkfs_erofs,
            "--quiet",
            "--blobdev",
            str(temp_blob_path),
            "--chunksize",
            str(chunk_size),
            str(metadata_path),
            str(view_dir),
        ]
    )
    os.chmod(metadata_path, 0o444)
    os.chmod(temp_blob_path, 0o400)


def reconstruct_blob_view(blob_map, output_path):
    output_path = Path(output_path)
    with Path(blob_map["central_blob"]).open("rb") as source, output_path.open("wb") as target:
        for extent in blob_map["extents"]:
            source.seek(int(extent["central_offset"]))
            remaining = int(extent["length"])
            target.seek(int(extent["logical_offset"]))
            while remaining:
                data = source.read(min(1024 * 1024, remaining))
                if not data:
                    fail("central blob extent is truncated while reconstructing view")
                remaining -= len(data)
                target.write(data)
        target.truncate(int(blob_map["logical_size"]))
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


def dm_table_for_extents(blob_map, backing_device):
    lines = []
    expected_logical = 0
    for extent in blob_map.get("extents", []):
        logical = int(extent["logical_offset"])
        central = int(extent["central_offset"])
        length = int(extent["length"])
        if logical != expected_logical:
            fail(f"non-contiguous blob map at logical offset {logical}, expected {expected_logical}")
        if logical % 512 != 0 or central % 512 != 0 or length % 512 != 0:
            fail("blob map extent is not 512-byte aligned")
        lines.append(f"{logical // 512} {length // 512} linear {backing_device} {central // 512}")
        expected_logical += length
    logical_size = int(blob_map["logical_size"])
    if expected_logical != logical_size:
        fail(f"blob map size mismatch: {expected_logical} != {logical_size}")
    return "\n".join(lines) + "\n"


def create_dm_view_for_blob_map(root, name, blob_map):
    central_blob = blob_map["central_blob"]
    if not os.path.exists(central_blob):
        fail(f"central blob is missing: {central_blob}")
    loop = ensure_loop_for_blob(central_blob)
    dmsetup = require_tool("dmsetup", "NIXOME_DMSETUP")
    if not dm_exists(name):
        table = dm_table_for_extents(blob_map, loop)
        table_path = Path(root) / "runtime" / f"{name}.table"
        atomic_write(table_path, table, mode=0o400)
        maybe_failpoint("mapper-before-create")
        run_checked([dmsetup, "-r", "create", name], input_text=table)
        maybe_failpoint("mapper-after-create")
    run_checked([dmsetup, "mknodes", name])
    return {
        "loop": loop,
        "dm_name": name,
        "device": dm_path(name),
    }


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
    if store.get("view_backend") not in EROFS_FILTERED_BLOB_BACKENDS:
        fail(f"generation is not an EROFS filtered-blob view: {domain} generation {generation}")

    blob_map_path = store.get("canonical_blob_map") or store.get("blob_map")
    if not blob_map_path:
        fail(f"generation is missing blob map: {domain} generation {generation}")
    blob_map = read_json(blob_map_path)
    central_blob = blob_map["central_blob"]
    if not os.path.exists(central_blob):
        fail(f"central blob is missing: {central_blob}")

    name = blob_dm_name(domain, generation, store.get("closure_hash", ""))
    path = dm_path(name)
    dm_view = create_dm_view_for_blob_map(root, name, blob_map)

    return {
        "schema": "nixome.closure-store.attach.v1",
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "view_backend": store.get("view_backend"),
        "blob_map": blob_map_path,
        "central_blob": central_blob,
        "loop": dm_view["loop"],
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
    return {
        "schema": "nixome.closure-store.detach.v1",
        "status": "passed",
        "pool_root": str(root),
        "domain": domain,
        "generation": generation,
        "dm_name": name,
        "device": dm_path(name),
        "removed": removed,
    }


def materialize_ext4_view(root, generation_dir, view_dir, closure_id, additional_space_mib):
    image_dir = Path(root) / "images/by-closure" / closure_id
    image_path = image_dir / "store.img"
    image_manifest = image_dir / "manifest.json"

    image_dir.mkdir(parents=True, exist_ok=True)
    image_created = False
    if not image_path.exists():
        sizing = create_ext4_image(view_dir, image_path, additional_space_mib)
        atomic_json(
            image_manifest,
            {
                "schema": "nixome.closure-store.view-image.v1",
                "closure_hash": closure_id,
                "image": str(image_path),
                "mountpoint": "/nix/store",
                "read_only": True,
                "filesystem": "ext4",
                "view_backend": VIEW_BACKEND_EXT4_IMAGE,
                "sizing": sizing,
                "image_contains_materialized_store_copy": True,
            },
        )
        image_created = True
    else:
        sizing = read_json(image_manifest).get("sizing", {}) if image_manifest.exists() else {}

    generation_image = generation_dir / "store.img"
    if generation_image.exists() or generation_image.is_symlink():
        generation_image.unlink()
    os.symlink(os.path.relpath(image_path, generation_dir), generation_image)
    fsync_dir(generation_dir)
    return {
        "view_backend": VIEW_BACKEND_EXT4_IMAGE,
        "filesystem": "ext4",
        "view_dir": str(view_dir),
        "image": str(generation_image),
        "canonical_image": str(image_path),
        "metadata_image": str(generation_image),
        "canonical_metadata_image": str(image_path),
        "blob_map": None,
        "canonical_blob_map": None,
        "chunk_refs": None,
        "canonical_chunk_refs": None,
        "blob_device": None,
        "blob_volume": None,
        "blob_logical_size": None,
        "chunk_size": None,
        "chunk_count": 0,
        "chunks_admitted": 0,
        "chunks_reused": 0,
        "image_created": image_created,
        "sizing": sizing,
        "image_contains_materialized_store_copy": True,
    }


def materialize_erofs_blobmap_view(root, generation_dir, view_dir, closure_id, chunk_size, txid):
    view_root = Path(root) / "views/by-closure" / closure_id
    metadata_image = view_root / "store.erofs"
    blob_map = view_root / "store.blob-map.json"
    chunk_refs = view_root / "chunk-refs"
    view_manifest = view_root / "manifest.json"
    view_root.mkdir(parents=True, exist_ok=True)

    view_created = False
    if not metadata_image.exists() or not blob_map.exists() or not chunk_refs.exists():
        tmp_dir = Path(root) / "tmp" / f"erofs-view-{closure_id}-{txid}"
        if tmp_dir.exists():
            remove_path(tmp_dir)
        tmp_dir.mkdir(parents=True)
        tmp_metadata = tmp_dir / "store.erofs"
        tmp_blob = tmp_dir / "store.blob"
        create_erofs_metadata_image(view_dir, tmp_metadata, tmp_blob, chunk_size)
        chunk_map = admit_blob_chunks(root, tmp_blob, chunk_size, txid)

        final_tmp_metadata = view_root / f".store.erofs.tmp-{uuid.uuid4().hex}"
        shutil.copy2(tmp_metadata, final_tmp_metadata)
        os.chmod(final_tmp_metadata, 0o444)
        os.replace(final_tmp_metadata, metadata_image)

        write_blob_map(
            blob_map,
            {
                "schema": SCHEMA_BLOB_MAP,
                "closure_hash": closure_id,
                "filesystem": "erofs",
                "view_backend": VIEW_BACKEND_EROFS_BLOBMAP,
                "metadata_image": str(metadata_image),
                "central_blob": chunk_map["blob"],
                "logical_size": chunk_map["blob_logical_size"],
                "chunk_size": chunk_map["chunk_size"],
                "chunk_count": chunk_map["chunk_count"],
                "chunks_admitted": chunk_map["chunks_admitted"],
                "chunks_reused": chunk_map["chunks_reused"],
                "extents": chunk_map["extents"],
            },
        )
        write_chunk_refs(chunk_refs, chunk_map["chunk_refs"])
        atomic_json(
            view_manifest,
            {
                "schema": "nixome.closure-store.view-erofs-blobmap.v1",
                "closure_hash": closure_id,
                "filesystem": "erofs",
                "view_backend": VIEW_BACKEND_EROFS_BLOBMAP,
                "metadata_image": str(metadata_image),
                "blob_map": str(blob_map),
                "chunk_refs": str(chunk_refs),
                "read_only": True,
                "image_contains_materialized_store_copy": False,
            },
        )
        remove_path(tmp_dir)
        fsync_dir(view_root)
        view_created = True

    blob_map_payload = read_json(blob_map)
    generation_metadata = generation_dir / "store.erofs"
    generation_blob_map = generation_dir / "store.blob-map.json"
    generation_chunk_refs = generation_dir / "chunk-refs"
    for target, source in [
        (generation_metadata, metadata_image),
        (generation_blob_map, blob_map),
        (generation_chunk_refs, chunk_refs),
    ]:
        if target.exists() or target.is_symlink():
            target.unlink()
        os.symlink(os.path.relpath(source, generation_dir), target)
    fsync_dir(generation_dir)

    return {
        "view_backend": VIEW_BACKEND_EROFS_BLOBMAP,
        "filesystem": "erofs",
        "view_dir": str(view_dir),
        "image": str(generation_metadata),
        "canonical_image": str(metadata_image),
        "metadata_image": str(generation_metadata),
        "canonical_metadata_image": str(metadata_image),
        "blob_map": str(generation_blob_map),
        "canonical_blob_map": str(blob_map),
        "chunk_refs": str(generation_chunk_refs),
        "canonical_chunk_refs": str(chunk_refs),
        "blob_device": "/dev/xvdf",
        "blob_volume": "store-data",
        "blob_logical_size": blob_map_payload["logical_size"],
        "chunk_size": blob_map_payload["chunk_size"],
        "chunk_count": blob_map_payload["chunk_count"],
        "chunks_admitted": blob_map_payload.get("chunks_admitted", 0),
        "chunks_reused": blob_map_payload.get("chunks_reused", 0),
        "image_created": view_created,
        "sizing": {
            "metadata_image_bytes": os.path.getsize(metadata_image),
            "blob_logical_size": blob_map_payload["logical_size"],
            "chunk_size": blob_map_payload["chunk_size"],
            "chunk_count": blob_map_payload["chunk_count"],
            "extent_count": len(blob_map_payload.get("extents", [])),
        },
        "image_contains_materialized_store_copy": False,
    }


def materialize_erofs_objectmap_view(root, generation_dir, paths, closure_id, txid):
    root = Path(root)
    view_root = root / "views/by-closure" / closure_id
    metadata_image = view_root / "store.erofs"
    blob_map = view_root / "store.blob-map.json"
    object_refs = view_root / "object-refs"
    view_manifest = view_root / "manifest.json"
    view_root.mkdir(parents=True, exist_ok=True)

    view_created = False
    if not metadata_image.exists() or not blob_map.exists() or not object_refs.exists():
        object_blob = str(object_blob_path(root))
        extents = []
        object_entries = []
        logical_offset = 0
        total_object_data_bytes = 0
        for logical_path in sorted(paths):
            entry = store_entry(logical_path)
            meta_path = object_blob_meta_path(root, entry)
            if not meta_path.exists():
                fail(f"canonical object data is missing while creating view: {logical_path}")
            meta = read_json(meta_path)
            if meta.get("central_blob") != object_blob:
                fail(f"canonical object data is in an unexpected blob for {logical_path}")
            size = int(meta["size"])
            offset = int(meta["offset"])
            if size % TAR_BLOCK_SIZE != 0 or offset % TAR_BLOCK_SIZE != 0:
                fail(f"canonical object extent is not tar-block aligned: {logical_path}")
            verify_blob_region(object_blob, offset, size, meta["sha256"], f"canonical object {entry}")
            extents.append(
                {
                    "logical_offset": logical_offset,
                    "central_offset": offset,
                    "length": size,
                    "object": entry,
                    "object_sha256": meta["sha256"],
                }
            )
            object_entries.append(entry)
            logical_offset += size
            total_object_data_bytes += size

        eof = ensure_tar_eof_extent(root, txid)
        if eof.get("central_blob") != object_blob:
            fail("canonical tar EOF is in an unexpected blob")
        extents.append(
            {
                "logical_offset": logical_offset,
                "central_offset": int(eof["offset"]),
                "length": int(eof["size"]),
                "object": ".tar-eof",
                "object_sha256": eof["sha256"],
            }
        )
        logical_offset += int(eof["size"])

        blob_map_payload = {
            "schema": SCHEMA_BLOB_MAP,
            "closure_hash": closure_id,
            "filesystem": "erofs",
            "view_backend": VIEW_BACKEND_EROFS_OBJECTMAP,
            "external_data_format": "posix-tar-index",
            "metadata_image": str(metadata_image),
            "central_blob": object_blob,
            "logical_size": logical_offset,
            "object_count": len(object_entries),
            "object_data_bytes": total_object_data_bytes,
            "objects": object_entries,
            "extents": extents,
        }

        tmp_metadata = view_root / f".store.erofs.tmp-{uuid.uuid4().hex}"
        create_erofs_tar_index_image(root, blob_map_payload, tmp_metadata, closure_id, txid)
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
        "chunk_refs": None,
        "canonical_chunk_refs": None,
        "object_refs": str(generation_object_refs),
        "canonical_object_refs": str(object_refs),
        "blob_device": "/dev/xvdf",
        "blob_volume": "store-data",
        "blob_logical_size": blob_map_payload["logical_size"],
        "chunk_size": None,
        "chunk_count": 0,
        "chunks_admitted": 0,
        "chunks_reused": 0,
        "object_data_bytes": blob_map_payload.get("object_data_bytes", 0),
        "object_extent_count": len(blob_map_payload.get("extents", [])),
        "image_created": view_created,
        "sizing": {
            "metadata_image_bytes": os.path.getsize(metadata_image),
            "blob_logical_size": blob_map_payload["logical_size"],
            "object_data_bytes": blob_map_payload.get("object_data_bytes", 0),
            "object_count": blob_map_payload.get("object_count", 0),
            "extent_count": len(blob_map_payload.get("extents", [])),
        },
        "image_contains_materialized_store_copy": False,
    }


def materialize_view(
    root,
    domain,
    generation,
    paths,
    closure_id,
    additional_space_mib,
    view_backend,
    chunk_size,
    txid,
):
    generation_dir = Path(root) / "domains" / domain / "generations" / str(generation)
    if view_backend == VIEW_BACKEND_EROFS_OBJECTMAP:
        return materialize_erofs_objectmap_view(root, generation_dir, paths, closure_id, txid)

    view_dir = generation_dir / "view"
    store_dir = view_dir
    if view_dir.exists():
        remove_path(view_dir)
    view_dir.mkdir(parents=True)
    for logical_path in sorted(paths):
        link_object_into_view(root, store_entry(logical_path), store_dir / store_entry(logical_path))
    chmod_readonly(view_dir)
    fsync_dir(generation_dir)

    if view_backend == VIEW_BACKEND_EXT4_IMAGE:
        return materialize_ext4_view(root, generation_dir, view_dir, closure_id, additional_space_mib)
    if view_backend == VIEW_BACKEND_EROFS_BLOBMAP:
        return materialize_erofs_blobmap_view(root, generation_dir, view_dir, closure_id, chunk_size, txid)
    fail(f"unsupported closure view backend: {view_backend}")


def generation_manifest_path(root, domain, generation):
    return Path(root) / "domains" / domain / "generations" / str(generation) / "manifest.json"


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
    if manifest.get("schema") != SCHEMA_GENERATION or manifest.get("status") != "committed":
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

    verification = run_verify([args.system], args.verify_mode)
    materialize_tree = args.view_backend in (VIEW_BACKEND_EXT4_IMAGE, VIEW_BACKEND_EROFS_BLOBMAP)
    admitted = []
    reused = []
    for logical_path in paths:
        result = admit_object(root, logical_path, args.physical_store_root, txid, materialize_tree=materialize_tree)
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
        args.additional_space_mib,
        args.view_backend,
        args.chunk_size,
        txid,
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
            "chunk_refs": view["chunk_refs"],
            "canonical_chunk_refs": view["canonical_chunk_refs"],
            "object_refs": view.get("object_refs"),
            "canonical_object_refs": view.get("canonical_object_refs"),
            "blob_logical_size": view["blob_logical_size"],
            "chunk_size": view["chunk_size"],
            "chunk_count": view["chunk_count"],
            "object_data_bytes": view.get("object_data_bytes", 0),
            "object_extent_count": view.get("object_extent_count", 0),
            "image_contains_materialized_store_copy": view["image_contains_materialized_store_copy"],
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
        "chunks_total": view["chunk_count"],
        "chunks_admitted": view["chunks_admitted"],
        "chunks_reused": view["chunks_reused"],
        "admitted": admitted,
        "reused": reused,
        "view_created": view["image_created"],
        "view_image": view["image"],
        "canonical_view_image": view["canonical_image"],
        "metadata_image": view["metadata_image"],
        "canonical_metadata_image": view["canonical_metadata_image"],
        "blob_map": view["blob_map"],
        "canonical_blob_map": view["canonical_blob_map"],
        "chunk_refs": view["chunk_refs"],
        "canonical_chunk_refs": view["canonical_chunk_refs"],
        "object_refs": view.get("object_refs"),
        "canonical_object_refs": view.get("canonical_object_refs"),
        "blob_device": args.store_blob_device if view["blob_volume"] else None,
        "blob_volume": view["blob_volume"],
        "blob_logical_size": view["blob_logical_size"],
        "object_data_bytes": view.get("object_data_bytes", 0),
        "object_extent_count": view.get("object_extent_count", 0),
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
        "metadata_image": manifest["store"].get("metadata_image", manifest["store"]["image"]),
        "canonical_metadata_image": manifest["store"].get(
            "canonical_metadata_image", manifest["store"]["canonical_image"]
        ),
        "blob_map": manifest["store"].get("blob_map"),
        "canonical_blob_map": manifest["store"].get("canonical_blob_map"),
        "chunk_refs": manifest["store"].get("chunk_refs"),
        "canonical_chunk_refs": manifest["store"].get("canonical_chunk_refs"),
        "object_refs": manifest["store"].get("object_refs"),
        "canonical_object_refs": manifest["store"].get("canonical_object_refs"),
        "blob_device": manifest["store"].get("blob_device"),
        "blob_volume": manifest["store"].get("blob_volume"),
        "filesystem": manifest["store"].get("filesystem", "ext4"),
        "view_backend": manifest["store"].get("view_backend", VIEW_BACKEND_EXT4_IMAGE),
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
                    yield domain_dir.name, int(generation_dir.name), manifest_path, manifest


def recover(args):
    root = ensure_pool(args.pool_root)
    aborted = []
    finalized = []
    removed_incomplete_generations = []
    for tx in sorted((root / "transactions").glob("*.json")):
        data = read_json(tx)
        if data.get("status") == "committed":
            manifest_path = data.get("manifest")
            if manifest_path and Path(manifest_path).exists():
                manifest = read_json(manifest_path)
                if manifest.get("schema") == SCHEMA_GENERATION and manifest.get("status") == "prepared":
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
    return {
        "schema": SCHEMA_RECOVERY,
        "status": "passed",
        "pool_root": str(root),
        "aborted_transactions": aborted,
        "finalized_generations": finalized,
        "removed_incomplete_generations": removed_incomplete_generations,
    }


def gc(args):
    root = ensure_pool(args.pool_root)
    live_objects = set()
    live_images = set()
    live_views = set()
    live_chunks = set()
    active_transaction_ids = set()
    active_transaction_objects = set()
    active_transaction_chunks = set()
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

    for manifest_path in sorted((root / "domains").glob("*/generations/*/manifest.json")):
        try:
            manifest = read_json(manifest_path)
        except (OSError, json.JSONDecodeError):
            continue
        if manifest.get("schema") != SCHEMA_GENERATION or generation_is_committed(root, manifest):
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
        add_manifest_store_liveness(root, manifest, live_objects, live_images, live_views, live_chunks)

    if active_transaction_ids:
        for meta in sorted((root / "objects/blob-metadata").glob("*.json")):
            if meta.name == ".tar-eof.json":
                continue
            payload = read_json(meta)
            if payload.get("transaction") in active_transaction_ids:
                active_transaction_objects.add(payload.get("entry") or meta.name[: -len(".json")])
        for meta in sorted((root / "chunks/metadata").glob("*.json")):
            payload = read_json(meta)
            if payload.get("transaction") in active_transaction_ids:
                active_transaction_chunks.add(payload.get("hash") or meta.stem)

    live_objects.update(active_transaction_objects)
    live_chunks.update(active_transaction_chunks)

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
            if gen in keep or (domain, gen) in running_generations or manifest.get("retention", {}).get("retain", False):
                retained_generations.add((domain, gen))
                add_manifest_store_liveness(root, manifest, live_objects, live_images, live_views, live_chunks)
            else:
                generation_dir = manifest_path.parent
                remove_path(generation_dir)
                removed_generations.append({"domain": domain, "generation": gen})
                maybe_failpoint("gc-after-generation-remove")

    removed_objects = []
    for obj in sorted((root / "objects/store").iterdir()):
        if obj.name not in live_objects:
            if obj.is_dir():
                remove_path(obj)
            else:
                remove_path(obj)
            meta = object_meta_path(root, obj.name)
            if meta.exists():
                meta.unlink()
            removed_objects.append(obj.name)

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

    removed_images = []
    for image_dir in sorted((root / "images/by-closure").iterdir()):
        image = image_dir / "store.img"
        if str(image) not in live_images:
            remove_path(image_dir)
            removed_images.append(image_dir.name)

    removed_views = []
    for view_dir in sorted((root / "views/by-closure").iterdir()):
        if str(view_dir) not in live_views:
            remove_path(view_dir)
            removed_views.append(view_dir.name)

    removed_chunks = []
    for meta in sorted((root / "chunks/metadata").glob("*.json")):
        digest = meta.stem
        if digest not in live_chunks:
            meta.unlink()
            removed_chunks.append(digest)

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
        "in_progress_generations": in_progress_generations,
        "active_transaction_ids": sorted(active_transaction_ids),
        "active_transaction_objects": sorted(active_transaction_objects),
        "active_transaction_chunks": sorted(active_transaction_chunks),
        "removed_generations": removed_generations,
        "removed_objects": removed_objects,
        "removed_object_metadata": removed_object_metadata,
        "removed_object_blobs": removed_object_blobs,
        "removed_images": removed_images,
        "removed_views": removed_views,
        "removed_chunks": removed_chunks,
        "chunk_blob": str(chunk_blob_path(root)),
        "chunk_blob_append_only": True,
        "chunk_blob_compaction": "not implemented",
        "object_blob": str(object_blob_path(root)),
        "object_blob_append_only": True,
        "object_blob_compaction": "not implemented",
    }


def status(args):
    root = ensure_pool(args.pool_root)
    domains = {}
    for domain, generation, manifest_path, manifest in iter_committed_generations(root):
        domains.setdefault(domain, {"current": current_generation(root, domain), "generations": []})
        domains[domain]["generations"].append(
            {
                "generation": generation,
                "manifest": str(manifest_path),
                "system": manifest["system"],
                "closure_hash": manifest["store"]["closure_hash"],
                "object_count": manifest["store"]["object_count"],
                "filesystem": manifest["store"].get("filesystem", "ext4"),
                "view_backend": manifest["store"].get("view_backend", VIEW_BACKEND_EXT4_IMAGE),
                "chunk_count": manifest["store"].get("chunk_count", 0),
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
    chunk_count = len(list((root / "chunks/metadata").glob("*.json")))
    chunk_blob = chunk_blob_path(root)
    object_blob = object_blob_path(root)
    return {
        "schema": SCHEMA_STATUS,
        "status": "passed",
        "pool_root": str(root),
        "object_count": object_count,
        "object_blob_count": object_blob_count,
        "object_blob": str(object_blob),
        "object_blob_size": os.path.getsize(object_blob) if object_blob.exists() else 0,
        "chunk_count": chunk_count,
        "chunk_blob": str(chunk_blob),
        "chunk_blob_size": os.path.getsize(chunk_blob) if chunk_blob.exists() else 0,
        "domains": domains,
        "running": running,
    }


def add_common(parser):
    parser.add_argument("--pool-root", default=os.environ.get("NIXOME_CLOSURE_STORE_ROOT", "/var/lib/nixome/closure-store"))


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
    admit_parser.add_argument("--physical-store-root", default=default_physical_store_root())
    admit_parser.add_argument("--verify-mode", default=os.environ.get("NIXOME_CLOSURE_STORE_VERIFY", "nix-store"), choices=["none", "tree", "nix-store", "nix-store-trust"])
    admit_parser.add_argument("--additional-space-mib", type=int, default=256)
    admit_parser.add_argument(
        "--view-backend",
        default=os.environ.get("NIXOME_CLOSURE_STORE_VIEW_BACKEND", DEFAULT_VIEW_BACKEND),
        choices=[VIEW_BACKEND_EXT4_IMAGE, VIEW_BACKEND_EROFS_BLOBMAP, VIEW_BACKEND_EROFS_OBJECTMAP],
    )
    admit_parser.add_argument(
        "--chunk-size",
        type=int,
        default=int(os.environ.get("NIXOME_CLOSURE_STORE_CHUNK_SIZE", DEFAULT_CHUNK_SIZE)),
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
    attach_parser.set_defaults(func=lambda args: attach_blob_view(args.pool_root, args.domain, args.generation))

    detach_parser = sub.add_parser("detach-blob-view")
    add_common(detach_parser)
    detach_parser.add_argument("--domain", required=True)
    detach_parser.add_argument("--generation", required=True)
    detach_parser.set_defaults(func=lambda args: detach_blob_view(args.pool_root, args.domain, args.generation))

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
