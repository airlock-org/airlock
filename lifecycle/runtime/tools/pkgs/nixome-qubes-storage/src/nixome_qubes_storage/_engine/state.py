import json
import os
import uuid
from contextlib import suppress
from pathlib import Path


from .core import (
    SCHEMA_GENERATION,
    SCHEMA_RUNNING_REF,
    atomic_json,
    fail,
    fsync_dir,
    maybe_failpoint,
    read_json,
    remove_path,
    safe_domain_name,
    utc_now,
)
from .pool import ensure_pool


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
        from .views import blob_map_physical_references

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
