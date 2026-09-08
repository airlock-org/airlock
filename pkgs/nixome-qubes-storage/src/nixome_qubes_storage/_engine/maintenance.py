import fcntl
import hashlib
import json
import os
import uuid
from contextlib import suppress
from pathlib import Path


from .core import (
    DEFAULT_SEGMENT_TARGET_BYTES,
    EngineError,
    SCHEMA_COMPACTION,
    SCHEMA_GC,
    SCHEMA_GENERATION,
    SCHEMA_RECOVERY,
    SCHEMA_STATUS,
    VIEW_BACKEND_EROFS_OBJECTMAP,
    add_manifest_store_liveness,
    atomic_json,
    closure_store_fsync_enabled,
    dm_exists,
    fail,
    maybe_failpoint,
    parse_bool,
    read_json,
    remove_path,
    utc_now,
)
from .objects import append_file_to_segment
from .pool import ensure_pool
from .segments import (
    configured_segment_target_bytes,
    current_location_from_meta,
    current_open_segment_locked,
    current_segment_ref_path,
    location_key,
    normalize_location,
    object_blob_meta_path,
    object_blob_meta_paths,
    object_extent_from_meta,
    object_meta_path,
    parse_positive_int,
    read_segment_meta,
    seal_segment_locked,
    segment_blob_path,
    segment_ids,
    segment_lock_path,
    segment_meta_path,
    segments_dir,
    tar_eof_meta_path,
    top_level_location_fields,
    validate_segment_id,
    verify_segment_region,
)
from .state import (
    current_generation,
    generation_is_committed,
    generation_manifest_path,
    iter_running_generation_refs,
    transaction_path,
    transaction_status,
)
from .views import (
    blob_map_physical_references,
    iter_runtime_view_refs,
    merge_segment_reference_details,
    runtime_segment_references,
)


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
