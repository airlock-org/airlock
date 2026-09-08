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
