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


from .core import DEFAULT_SEGMENT_TARGET_BYTES, SCHEMA_POOL, atomic_json, read_json, utc_now
from .objects import migrate_legacy_blob_maps, migrate_single_blob_to_segments

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
