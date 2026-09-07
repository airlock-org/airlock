import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import time
import uuid
from pathlib import Path


SCHEMA_POOL = "nixome.closure-store.v1"
SCHEMA_ADMISSION = "nixome.closure-store.admission.v1"
SCHEMA_GENERATION = "nixome.closure-store.generation.v1"
SCHEMA_SWITCH = "nixome.closure-store.switch.v1"
SCHEMA_ROLLBACK = "nixome.closure-store.rollback.v1"
SCHEMA_GC = "nixome.closure-store.gc.v1"
SCHEMA_RECOVERY = "nixome.closure-store.recovery.v1"
SCHEMA_STATUS = "nixome.closure-store.status.v1"


class EngineError(RuntimeError):
    pass


def utc_now():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def fail(message):
    raise EngineError(message)


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
        "domains",
        "images/by-closure",
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
            },
        )
    return root


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


def admit_object(root, logical_path, physical_store_root, txid):
    entry = store_entry(logical_path)
    source = physical_store_path(logical_path, physical_store_root)
    if not source.exists():
        fail(f"store object source is missing: {logical_path} ({source})")

    source_hash = tree_hash(source)
    final = object_path(root, entry)
    meta = object_meta_path(root, entry)
    if final.exists():
        current_meta = read_json(meta) if meta.exists() else {}
        current_hash = current_meta.get("tree_hash") or tree_hash(final)
        if current_hash != source_hash:
            fail(f"existing object hash mismatch for {logical_path}")
        return {
            "path": logical_path,
            "entry": entry,
            "tree_hash": current_hash,
            "admitted": False,
        }

    tmp = Path(root) / "tmp" / f"object-{entry}-{txid}"
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
        return admit_object(root, logical_path, physical_store_root, txid)
    fsync_dir(final.parent)
    chmod_readonly(final)
    atomic_json(
        meta,
        {
            "schema": "nixome.closure-store.object.v1",
            "path": logical_path,
            "entry": entry,
            "tree_hash": source_hash,
            "admitted_at": utc_now(),
        },
    )
    return {
        "path": logical_path,
        "entry": entry,
        "tree_hash": source_hash,
        "admitted": True,
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


def materialize_view(root, domain, generation, paths, closure_id, additional_space_mib):
    generation_dir = Path(root) / "domains" / domain / "generations" / str(generation)
    view_dir = generation_dir / "view"
    store_dir = view_dir
    image_dir = Path(root) / "images/by-closure" / closure_id
    image_path = image_dir / "store.img"
    image_manifest = image_dir / "manifest.json"

    if view_dir.exists():
        remove_path(view_dir)
    view_dir.mkdir(parents=True)
    for logical_path in sorted(paths):
        link_object_into_view(root, store_entry(logical_path), store_dir / store_entry(logical_path))
    chmod_readonly(view_dir)
    fsync_dir(generation_dir)

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
        "view_dir": str(view_dir),
        "image": str(generation_image),
        "canonical_image": str(image_path),
        "image_created": image_created,
        "sizing": sizing,
    }


def generation_manifest_path(root, domain, generation):
    return Path(root) / "domains" / domain / "generations" / str(generation) / "manifest.json"


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
    if not (target / "manifest.json").exists():
        fail(f"generation does not exist for {domain}: {generation}")
    tmp = domain_dir / f".current.tmp-{uuid.uuid4().hex}"
    os.symlink(os.path.relpath(target, domain_dir), tmp)
    os.replace(tmp, domain_dir / "current")
    fsync_dir(domain_dir)


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

    paths = read_store_paths(args.closure_source, args.system)
    paths = sorted(set(paths))
    if args.system not in paths:
        paths.append(args.system)
        paths = sorted(set(paths))

    verification = run_verify([args.system], args.verify_mode)
    admitted = []
    reused = []
    for logical_path in paths:
        result = admit_object(root, logical_path, args.physical_store_root, txid)
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
    )
    manifest_path = generation_dir / "manifest.json"
    manifest = {
        "schema": SCHEMA_GENERATION,
        "status": "committed",
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
            "read_only": True,
            "paths": paths,
            "object_count": len(paths),
            "view_dir": view["view_dir"],
            "image": view["image"],
            "canonical_image": view["canonical_image"],
            "image_contains_materialized_store_copy": True,
        },
        "verification": verification,
        "retention": {
            "retain": parse_bool(args.retain),
        },
    }
    atomic_json(manifest_path, manifest)
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
        "view_size": os.path.getsize(view["canonical_image"]),
        "read_only": True,
    }


def rollback(args):
    result = switch(args)
    result["schema"] = SCHEMA_ROLLBACK
    return result


def switch(args):
    root = ensure_pool(args.pool_root)
    domain = safe_domain_name(args.domain)
    generation = int(args.generation)
    manifest = read_json(generation_manifest_path(root, domain, generation))
    if manifest.get("schema") != SCHEMA_GENERATION or manifest.get("status") != "committed":
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
        "view_image": manifest["store"]["image"],
        "canonical_view_image": manifest["store"]["canonical_image"],
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
                if manifest.get("schema") == SCHEMA_GENERATION and manifest.get("status") == "committed":
                    yield domain_dir.name, int(generation_dir.name), manifest_path, manifest


def recover(args):
    root = ensure_pool(args.pool_root)
    recovered = []
    for tx in sorted((root / "transactions").glob("*.json")):
        data = read_json(tx)
        if data.get("status") == "committed":
            continue
        data["status"] = "aborted"
        data["aborted_at"] = utc_now()
        atomic_json(tx, data)
        recovered.append(str(tx))
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
    return {
        "schema": SCHEMA_RECOVERY,
        "status": "passed",
        "pool_root": str(root),
        "aborted_transactions": recovered,
    }


def gc(args):
    root = ensure_pool(args.pool_root)
    live_objects = set()
    live_images = set()
    retained_generations = set()
    removed_generations = []

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
            if gen in keep or manifest.get("retention", {}).get("retain", False):
                retained_generations.add((domain, gen))
                for path in manifest["store"]["paths"]:
                    live_objects.add(store_entry(path))
                live_images.add(manifest["store"]["canonical_image"])
            else:
                generation_dir = manifest_path.parent
                remove_path(generation_dir)
                removed_generations.append({"domain": domain, "generation": gen})

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

    removed_images = []
    for image_dir in sorted((root / "images/by-closure").iterdir()):
        image = image_dir / "store.img"
        if str(image) not in live_images:
            remove_path(image_dir)
            removed_images.append(image_dir.name)

    return {
        "schema": SCHEMA_GC,
        "status": "passed",
        "pool_root": str(root),
        "retained_generations": [
            {"domain": domain, "generation": generation}
            for domain, generation in sorted(retained_generations)
        ],
        "removed_generations": removed_generations,
        "removed_objects": removed_objects,
        "removed_images": removed_images,
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
            }
        )
    object_count = len(list((root / "objects/store").iterdir()))
    return {
        "schema": SCHEMA_STATUS,
        "status": "passed",
        "pool_root": str(root),
        "object_count": object_count,
        "domains": domains,
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
    admit_parser.add_argument("--store-mountpoint", default="/nix/store")
    admit_parser.add_argument("--physical-store-root", default=default_physical_store_root())
    admit_parser.add_argument("--verify-mode", default=os.environ.get("NIXOME_CLOSURE_STORE_VERIFY", "nix-store"), choices=["none", "tree", "nix-store", "nix-store-trust"])
    admit_parser.add_argument("--additional-space-mib", type=int, default=256)
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

    return parser


def run(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)
