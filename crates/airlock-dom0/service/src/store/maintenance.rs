//! Storage reclamation traces logical generations and physical pins before deleting bytes.
//! Relocation publishes verified replacement extents before refreshing unpinned views.

use super::{fs::*, records, root, segments, views};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

/// Objects and generations protected by selection, retention, running guests or unfinished work.
struct Liveness {
    // Nix basenames with at least one retained logical reference.
    objects: BTreeSet<String>,
    // Canonical view directories retained by protected generation manifests.
    views: BTreeSet<PathBuf>,
    // Committed domain/generation pairs which must survive maintenance.
    generations: BTreeSet<(String, u64)>,
    // Unfinished transactions whose newly appended objects remain protected.
    transactions: BTreeSet<String>,
    // Committed manifests indexed by their domain and generation.
    manifests: BTreeMap<(String, u64), (PathBuf, Value)>,
    // Incomplete generation records retained for recovery rather than activation.
    incomplete: Vec<Value>,
    // Running references lacking a committed generation, reported without discarding pins.
    stale_running: Vec<PathBuf>,
}

fn add_manifest(live: &mut Liveness, manifest: &Value) -> Result<()> {
    if let Some(paths) = manifest["store"]["paths"].as_array() {
        for path in paths {
            live.objects
                .insert(entry(path.as_str().context("invalid live path")?)?.to_owned());
        }
    }
    if let Some(path) = manifest["store"]["canonical_blob_map"].as_str() {
        live.views.insert(
            Path::new(path)
                .parent()
                .context("missing view parent")?
                .to_path_buf(),
        );
    }
    Ok(())
}

fn liveness(root: &Path, keep: usize) -> Result<Liveness> {
    let mut live = Liveness {
        objects: BTreeSet::new(),
        views: BTreeSet::new(),
        generations: BTreeSet::new(),
        transactions: BTreeSet::new(),
        manifests: BTreeMap::new(),
        incomplete: Vec::new(),
        stale_running: Vec::new(),
    };
    for path in children(&root.join("transactions"))? {
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let value: Value = read(&path)?;
        if value["status"] != "committed" && value["status"] != "aborted" {
            live.transactions.insert(text(&value, "id")?);
        }
    }

    let mut by_domain: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for path in records::manifests(root)? {
        let value: Value = read(&path)?;
        let domain = text(&value, "domain")?;
        component(&domain)?;
        let generation = number(&value, "generation")?;
        if records::committed(root, &value)? {
            by_domain
                .entry(domain.clone())
                .or_default()
                .push(generation);
            live.manifests.insert((domain, generation), (path, value));
        } else if let Some(id) = value["transaction"].as_str() {
            let tx_path = root
                .join("transactions")
                .join(format!("{}.json", component(id)?));
            if tx_path.try_exists()? && read::<Value>(&tx_path)?["status"] != "aborted" {
                live.transactions.insert(id.to_owned());
                add_manifest(&mut live, &value)?;
                live.incomplete.push(json!({"domain":domain,"generation":generation,"transaction":id,"manifest":path}));
            }
        }
    }

    for (domain, mut generations) in by_domain {
        generations.sort_unstable_by(|a, b| b.cmp(a));
        for generation in generations
            .into_iter()
            .take(keep)
            .chain(records::current(root, &domain)?)
        {
            live.generations.insert((domain.clone(), generation));
        }
    }

    for (path, value) in records::running_refs(root)? {
        let key = (text(&value, "domain")?, number(&value, "generation")?);
        if live.manifests.contains_key(&key) {
            live.generations.insert(key);
        } else {
            live.stale_running.push(path);
        }
    }
    for (key, (_, value)) in &live.manifests {
        if value["retention"]["retain"] == true {
            live.generations.insert(key.clone());
        }
    }

    for key in live.generations.clone() {
        let (_, value) = live
            .manifests
            .get(&key)
            .context("current or running generation missing")?;
        let value = value.clone();
        add_manifest(&mut live, &value)?;
    }

    for path in children(&root.join("objects/blob-metadata"))? {
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let value: Value = read(&path)?;
        if value["transaction"]
            .as_str()
            .is_some_and(|id| live.transactions.contains(id))
            && let Some(name) = value["entry"].as_str()
        {
            live.objects.insert(component(name)?.to_owned());
        }
    }
    Ok(live)
}

fn locations(root: &Path, live: &Liveness) -> Result<BTreeMap<String, (Value, segments::Extent)>> {
    let mut output = BTreeMap::new();
    for name in live
        .objects
        .iter()
        .map(String::as_str)
        .chain((!live.views.is_empty()).then_some(".tar-eof"))
    {
        let value: Value = read(&segments::object_metadata(root, name)?)?;
        let extent = segments::extent(root, &value, true)?;
        output.insert(name.to_owned(), (value, extent));
    }
    Ok(output)
}

struct PhysicalReferences {
    // Physical extents protected by retained views and live or preparing consumers.
    by_segment: BTreeMap<String, Vec<Value>>,
    // Stale kernel mappings reported for recovery without releasing live pins.
    stale_runtime: Vec<PathBuf>,
}

fn physical_refs(
    root: &Path,
    live: &Liveness,
    locations: &BTreeMap<String, (Value, segments::Extent)>,
) -> Result<PhysicalReferences> {
    let mut references: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut stale = Vec::new();
    for (entry, (_, extent)) in locations {
        references.entry(extent.segment_id.clone()).or_default().push(json!({"segment_id":extent.segment_id,"source":"canonical-current","entry":entry,"offset":extent.segment_offset,"length":extent.size}));
    }
    for directory in &live.views {
        let path = directory.join("store.blob-map.json");
        pool_path(root, &path)?;
        for value in views::references(&read(&path)?, &format!("view:{}", directory.display()))? {
            references
                .entry(text(&value, "segment_id")?)
                .or_default()
                .push(value);
        }
    }
    for (_, record) in records::running_refs(root)? {
        for value in record["physical_references"]
            .as_array()
            .context("running reference lacks physical pins")?
        {
            references
                .entry(text(value, "segment_id")?)
                .or_default()
                .push(value.clone());
        }
    }
    for path in children(&root.join("runtime"))? {
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let value: Value = read(&path)?;
        ensure!(
            value["schema"] == "airlock.closure-store.runtime-view-ref.v1",
            "invalid runtime reference"
        );
        let name = text(&value, "dm_name")?;
        let present = views::dm_exists(&name).unwrap_or(true);
        if !present && value["status"] != "preparing" {
            stale.push(path);
            continue;
        }
        for reference in value["physical_references"]
            .as_array()
            .context("runtime reference lacks physical pins")?
        {
            references
                .entry(text(reference, "segment_id")?)
                .or_default()
                .push(reference.clone());
        }
    }
    Ok(PhysicalReferences {
        by_segment: references,
        stale_runtime: stale,
    })
}

fn inventory(
    root: &Path,
    locations: &BTreeMap<String, (Value, segments::Extent)>,
    references: &BTreeMap<String, Vec<Value>>,
) -> Result<Vec<Value>> {
    let mut output = Vec::new();
    for id in segments::ids(root)? {
        let path = segments::blob(root, &id)?;
        let metadata = segments::metadata(root, &id)?;
        let value: Value = read(&metadata)?;
        let used = fs::metadata(&path)?.len();
        let current: Vec<_> = locations
            .iter()
            .filter(|(_, (_, extent))| extent.segment_id == id)
            .collect();
        let live: u64 = current.iter().map(|(_, (_, extent))| extent.size).sum();
        let refs = references.get(&id).cloned().unwrap_or_default();
        output.push(json!({
            "id": id,
            "blob": path,
            "metadata": metadata,
            "status": value["status"],
            "sealed": value["sealed"],
            "oversized": value["oversized"],
            "target_bytes": value["target_bytes"],
            "used_bytes": used,
            "object_count": value["object_count"],
            "current_live_bytes": live,
            "current_live_entries": current.iter().map(|(name,_)|name).collect::<Vec<_>>(),
            "dead_bytes": used.saturating_sub(live),
            "utilization_percent": if used == 0 {100.0} else {live as f64 * 100.0 / used as f64},
            "physical_reference_count": refs.len(),
            "blocked_by_physical_refs": !refs.is_empty(),
            "references": refs.iter().take(50).collect::<Vec<_>>(),
            "reference_count": refs.len(),
        }));
    }
    Ok(output)
}

fn delete_segments(root: &Path, inventory: &[Value]) -> Result<Value> {
    let current = root.join("objects/current-segment");
    let current = if current.try_exists()? {
        fs::read_to_string(current)?.trim().to_owned()
    } else {
        String::new()
    };

    let mut removed = Vec::new();
    let mut retained = Vec::new();
    let mut partial = Vec::new();
    let mut blocked = Vec::new();
    let mut reclaimed = 0;
    for value in inventory {
        let id = text(value, "id")?;
        if value["sealed"] == true
            && id != current
            && number(value, "physical_reference_count")? == 0
        {
            failpoint("gc-before-segment-delete")?;
            remove(&segments::blob(root, &id)?)?;
            remove(&segments::metadata(root, &id)?)?;
            sync_dir(&root.join("objects/segments"))?;
            reclaimed += number(value, "used_bytes")?;
            removed.push(id);
            failpoint("gc-after-segment-delete")?;
        } else {
            if number(value, "current_live_bytes")? == 0
                && number(value, "physical_reference_count")? > 0
            {
                blocked.push(id.clone());
            }
            if number(value, "current_live_bytes")? > 0 && number(value, "dead_bytes")? > 0 {
                partial.push(id);
            }
            retained.push(value.clone());
        }
    }
    Ok(
        json!({"removed_segments":removed,"retained_segments":retained,"partially_live_segments":partial,"blocked_segments":blocked,"bytes_reclaimed":reclaimed}),
    )
}

pub(super) fn gc(root: &Path, keep: usize) -> Result<Value> {
    let live = liveness(root, keep)?;
    // Resolve every retained pin before the first deletion, so damaged metadata fails closed.
    let locations = locations(root, &live)?;
    let PhysicalReferences {
        by_segment: references,
        stale_runtime: stale,
    } = physical_refs(root, &live, &locations)?;
    let inventory = inventory(root, &locations, &references)?;
    let mut removed_generations = Vec::new();
    for key in live.manifests.keys() {
        if !live.generations.contains(key) {
            records::forget(root, &key.0, key.1, "garbage-collected")?;
            removed_generations.push(json!({"domain":key.0,"generation":key.1}));
            failpoint("gc-after-generation-remove")?;
        }
    }
    let mut removed_metadata = Vec::new();
    let mut removed_blobs = Vec::new();
    let mut removed_special = Vec::new();
    for (directory, removed) in [
        ("objects/metadata", &mut removed_metadata),
        ("objects/blob-metadata", &mut removed_blobs),
    ] {
        for path in children(&root.join(directory))? {
            if path.extension().is_none_or(|s| s != "json") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .context("invalid metadata name")?;
            if name == ".tar-eof" {
                if live.views.is_empty() {
                    remove(&path)?;
                    removed_special.push(name.to_owned());
                }
            } else if !live.objects.contains(name) {
                remove(&path)?;
                removed.push(name.to_owned());
                failpoint("gc-after-object-metadata-remove")?;
            }
        }
    }
    let mut removed_views = Vec::new();
    for directory in children(&root.join("views/by-closure"))? {
        if !live.views.contains(&directory) {
            removed_views.push(
                directory
                    .file_name()
                    .context("invalid view name")?
                    .to_string_lossy()
                    .into_owned(),
            );
            remove(&directory)?;
        }
    }
    let mut roots = BTreeSet::new();
    for path in records::manifests(root)? {
        let value: Value = read(&path)?;
        if !value["root"]["artifact"].is_null() {
            let descriptor: root::Descriptor =
                serde_json::from_value(value["root"]["artifact"].clone())?;
            descriptor.validate()?;
            roots.insert(descriptor.sha256);
        }
    }
    let mut removed_roots = Vec::new();
    for path in children(&root.join("root-artifacts"))? {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("invalid root artifact name")?;
        if !roots.contains(name) {
            pool_path(root, &path)?;
            removed_roots.push(name.to_owned());
            remove(&path)?;
        }
    }
    let mut result = delete_segments(root, &inventory)?;
    for (key,value) in json!({
        "schema": "airlock.closure-store.gc.v1",
        "status": "passed",
        "pool_root": root,
        "retained_generations": live.generations.iter().map(|(domain,generation)|json!({"domain":domain,"generation":generation})).collect::<Vec<_>>(),
        "stale_running_refs": live.stale_running,
        "stale_runtime_view_refs": stale,
        "in_progress_generations": live.incomplete,
        "active_transaction_ids": live.transactions,
        "removed_generations": removed_generations,
        "removed_object_metadata": removed_metadata,
        "removed_object_blobs": removed_blobs,
        "removed_special_extents": removed_special,
        "removed_views": removed_views,
        "removed_root_artifacts": removed_roots,
        "segments_dir": root.join("objects/segments"),
        "segment_store_append_only": true,
        "segment_compaction": "available",
    }).as_object().context("invalid GC receipt")? {
        result[key] = value.clone();
    }
    Ok(result)
}

pub(super) fn refresh_maps(root: &Path) -> Result<Vec<PathBuf>> {
    let pinned: BTreeSet<PathBuf> = records::running_refs(root)?
        .iter()
        .filter(|(_, v)| {
            matches!(
                v["volume"].as_str(),
                Some("host-retained" | "builder-persistent")
            )
        })
        .filter_map(|(_, v)| v["blob_map"].as_str().map(PathBuf::from))
        .collect();

    let mut paths = Vec::new();
    for directory in children(&root.join("views/by-closure"))? {
        pool_path(root, &directory)?;
        paths.push(directory.join("store.blob-map.json"));
    }
    for manifest in records::manifests(root)? {
        paths.push(
            manifest
                .parent()
                .context("missing generation parent")?
                .join("store.blob-map.json"),
        );
    }

    let mut refreshed = Vec::new();
    for path in paths {
        if path.is_symlink() || !path.try_exists()? || pinned.contains(&path) {
            continue;
        }
        let mut map: Value = read(&path)?;
        let mut items = views::extents(&map)?;
        let mut segments = BTreeMap::new();
        let mut changed = false;
        for item in &mut items {
            let meta = segments::object_metadata(root, &item.object)?;
            if !meta.try_exists()? {
                continue;
            }
            let extent = segments::extent(root, &read(&meta)?, true)?;
            ensure!(
                item.length == extent.size && item.object_sha256 == extent.sha256,
                "cannot refresh changed object identity"
            );
            changed |= (item.segment_id.as_str(), item.segment_offset)
                != (extent.segment_id.as_str(), extent.segment_offset);
            item.segment_id = extent.segment_id.clone();
            item.segment_blob = extent.segment_blob.clone();
            item.segment_offset = extent.segment_offset;
            let meta: Value = read(&segments::metadata(root, &extent.segment_id)?)?;
            segments.insert(extent.segment_id.clone(),json!({"id":extent.segment_id,"blob":extent.segment_blob,"size":fs::metadata(&extent.segment_blob)?.len(),"status":meta["status"]}));
        }
        let new_segments = json!(segments.values().collect::<Vec<_>>());
        changed |= map["segments"] != new_segments;
        if changed {
            map["segments"] = new_segments;
            map["segment_count"] = json!(segments.len());
            map["extents"] = serde_json::to_value(items)?;
            map["refreshed_at"] = json!(now());
            write(&path, &map, 0o444)?;
            refreshed.push(path);
        }
    }
    Ok(refreshed)
}

fn publish_location(
    root: &Path,
    name: &str,
    extent: &segments::Extent,
    transaction: &str,
) -> Result<()> {
    let path = segments::object_metadata(root, name)?;
    let mut value: Value = read(&path)?;
    let old = segments::extent(root, &value, true)?;
    let mut history = value["locations"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![serde_json::to_value(&old).expect("serializable extent")]);
    for location in &mut history {
        if location["status"] == "current" || location.get("status").is_none() {
            location["status"] = json!("retired");
            location["retired_at"] = json!(now());
            location["retired_by_transaction"] = json!(transaction);
        }
    }
    let mut new = serde_json::to_value(extent)?;
    new["status"] = json!("current");
    new["created_at"] = json!(now());
    new["transaction"] = json!(transaction);
    history.push(new);
    for (key, field) in serde_json::to_value(extent)?
        .as_object()
        .context("invalid extent")?
    {
        value[key] = field.clone();
    }
    value["locations"] = json!(history);
    value["relocated_at"] = json!(now());
    value["relocated_by_transaction"] = json!(transaction);
    value["transaction"] = json!(transaction);
    write(&path, &value, 0o444)?;
    failpoint("compact-between-location-records")?;
    reconcile_object(root, name, &value)?;
    Ok(())
}

// The blob record is published first and owns the canonical extent. After a crash,
// its verified identity can repair the convenience copy in the logical object record.
fn reconcile_object(root: &Path, name: &str, value: &Value) -> Result<bool> {
    if name != ".tar-eof" {
        let path = root
            .join("objects/metadata")
            .join(format!("{}.json", component(name)?));
        if path.try_exists()? {
            let mut object: Value = read(&path)?;
            ensure!(
                object["schema"] == "airlock.closure-store.object.v2"
                    && object["entry"] == name
                    && value["entry"] == name
                    && object["path"] == value["path"]
                    && object["tree_hash"] == value["tree_hash"],
                "cannot reconcile changed object identity"
            );
            if object["tar_fragment"] == *value {
                return Ok(false);
            }

            segments::extent(root, value, true)?;
            ensure!(
                object["tar_fragment"]["sha256"] == value["sha256"]
                    && object["tar_fragment"]["size"] == value["size"],
                "cannot reconcile changed fragment contents"
            );
            object["tar_fragment"] = value.clone();
            object["tree_materialized"] = json!(false);
            write(&path, &object, 0o644)?;
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn compact(
    root: &Path,
    keep: usize,
    threshold: u64,
    maximum: usize,
    target: u64,
    dry_run: bool,
) -> Result<Value> {
    ensure!(
        (1..=100).contains(&threshold) && maximum > 0,
        "invalid compaction bounds"
    );
    let live = liveness(root, keep)?;
    let live_locations = locations(root, &live)?;
    let PhysicalReferences {
        by_segment: references,
        ..
    } = physical_refs(root, &live, &live_locations)?;
    let plan_inventory = inventory(root, &live_locations, &references)?;
    let mut candidates: Vec<_> = plan_inventory
        .iter()
        .filter(|v| {
            v["sealed"] == true
                && v["oversized"] != true
                && v["current_live_bytes"].as_u64().is_some_and(|n| n > 0)
                && v["utilization_percent"]
                    .as_f64()
                    .is_some_and(|n| n <= threshold as f64)
        })
        .cloned()
        .collect();
    candidates.sort_by(|a, b| {
        a["utilization_percent"]
            .as_f64()
            .unwrap_or(100.0)
            .total_cmp(&b["utilization_percent"].as_f64().unwrap_or(100.0))
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    candidates.truncate(maximum);
    let zero: Vec<_> = plan_inventory
        .iter()
        .filter(|v| {
            v["sealed"] == true
                && v["current_live_bytes"] == 0
                && v["physical_reference_count"] == 0
        })
        .collect();
    let mut result = json!({
        "schema": "airlock.closure-store.compaction.v1",
        "status": "passed",
        "dry_run": dry_run,
        "pool_root": root,
        "threshold_percent": threshold,
        "max_segments": maximum,
        "segment_target_bytes": target,
        "candidate_segments": candidates,
        "segment_compaction": "copy-and-switch",
    });
    if dry_run {
        result["segments"] = json!(plan_inventory);
        result["segment_count"] = json!(plan_inventory.len());
        result["directly_reclaimable_segments"] = json!(zero);
        result["objects_to_move"] = json!(
            candidates
                .iter()
                .map(|v| v["current_live_entries"].as_array().map_or(0, Vec::len))
                .sum::<usize>()
        );
        result["bytes_to_copy"] = json!(
            candidates
                .iter()
                .filter_map(|v| v["current_live_bytes"].as_u64())
                .sum::<u64>()
        );
        result["bytes_reclaimable_now"] = json!(
            zero.iter()
                .filter_map(|v| v["used_bytes"].as_u64())
                .sum::<u64>()
        );
        return Ok(result);
    }

    let tx = transaction("compaction")?;
    let tx_path = root.join("transactions").join(format!("{tx}.json"));
    let noop = candidates.is_empty() && zero.is_empty();
    if !noop {
        write(
            &tx_path,
            &json!({
                "schema": "airlock.closure-store.transaction.v1",
                "id": tx,
                "type": "compaction",
                "status": "preparing",
                "created_at": now(),
                "candidate_segments": candidates.iter().map(|v|v["id"].clone()).collect::<Vec<_>>(),
            }),
            0o644,
        )?;
    }
    failpoint("compact-after-tx-create")?;

    let current = root.join("objects/current-segment");
    if !candidates.is_empty() && current.try_exists()? {
        let _lock = PoolLock::acquire(&root.join("objects/segments.lock"))?;
        segments::seal(
            root,
            fs::read_to_string(&current)?.trim(),
            &tx,
            "pre-compaction-open-segment",
        )?;
    }

    let mut moved = Vec::new();
    let mut new_segments = BTreeSet::new();
    let mut copied = 0;
    for candidate in &candidates {
        let id = text(candidate, "id")?;
        for (name, (_, old)) in &live_locations {
            if old.segment_id != id {
                continue;
            }
            let mut temporary = tempfile::NamedTempFile::new_in(root.join("tmp"))?;
            let mut source = File::open(&old.segment_blob)?;
            source.seek(SeekFrom::Start(old.segment_offset))?;
            ensure!(
                std::io::copy(&mut source.take(old.size), temporary.as_file_mut())? == old.size,
                "compaction source truncated"
            );
            temporary.flush()?;
            ensure!(
                hash(temporary.path())? == old.sha256,
                "compaction source checksum mismatch"
            );
            failpoint("compact-after-object-copy")?;
            let extent = segments::append(root, temporary.path(), name, &tx, target)?;
            segments::extent(root, &serde_json::to_value(&extent)?, true)?;
            failpoint("compact-before-location-publish")?;
            publish_location(root, name, &extent, &tx)?;
            failpoint("compact-after-location-publish")?;
            copied += extent.size;
            new_segments.insert(extent.segment_id.clone());
            moved.push(json!({
                "entry": name,
                "old_segment_id": old.segment_id,
                "old_segment_offset": old.segment_offset,
                "new_segment_id": extent.segment_id,
                "new_segment_offset": extent.segment_offset,
                "size": extent.size,
                "sha256": extent.sha256,
            }));
        }
    }

    for id in new_segments {
        let _lock = PoolLock::acquire(&root.join("objects/segments.lock"))?;
        failpoint("compact-before-replacement-seal")?;
        segments::seal(root, &id, &tx, "compaction-replacement")?;
    }

    let refreshed = if moved.is_empty() {
        Vec::new()
    } else {
        refresh_maps(root)?
    };
    failpoint("compact-after-view-refresh")?;
    if !noop {
        write(
            &tx_path,
            &json!({"schema":"airlock.closure-store.transaction.v1","id":tx,"type":"compaction","status":"committed","committed_at":now(),"objects_moved":moved.len(),"bytes_copied":copied}),
            0o644,
        )?;
    }
    failpoint("compact-after-tx-commit")?;
    let live = liveness(root, keep)?;
    let live_locations = locations(root, &live)?;
    let PhysicalReferences {
        by_segment: references,
        ..
    } = physical_refs(root, &live, &live_locations)?;
    let retirement = delete_segments(root, &inventory(root, &live_locations, &references)?)?;
    failpoint("compact-after-retirement")?;
    let dead: u64 = candidates
        .iter()
        .filter_map(|v| v["dead_bytes"].as_u64())
        .sum();
    for (key, value) in json!({
        "noop": noop,
        "transaction": if noop {Value::Null}else{json!(tx)},
        "objects_moved": moved,
        "objects_reused": [],
        "bytes_copied": copied,
        "bytes_reclaimed": retirement["bytes_reclaimed"],
        "segments_retired": retirement["removed_segments"],
        "segments_blocked_from_deletion": retirement["blocked_segments"],
        "views_refreshed": refreshed,
        "write_amplification": copied as f64/dead.max(1) as f64,
    })
    .as_object()
    .context("invalid compaction receipt")?
    {
        result[key] = value.clone();
    }
    Ok(result)
}

pub(super) fn recover(root: &Path) -> Result<Value> {
    let mut protected: BTreeSet<(String, u64)> = records::running_refs(root)?
        .iter()
        .map(|(_, value)| Ok((text(value, "domain")?, number(value, "generation")?)))
        .collect::<Result<_>>()?;
    for domain in children(&root.join("domains"))? {
        pool_path(root, &domain)?;
        let name = domain
            .file_name()
            .and_then(|s| s.to_str())
            .context("invalid domain directory")?;
        if let Some(generation) = records::current(root, name)? {
            protected.insert((name.to_owned(), generation));
        }
    }

    let mut transactions = Vec::new();
    for path in children(&root.join("transactions"))? {
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        pool_path(root, &path)?;
        let tx: Value = read(&path)?;
        ensure!(
            tx["schema"] == "airlock.closure-store.transaction.v1"
                && path.file_stem().and_then(|s| s.to_str()) == Some(text(&tx, "id")?.as_str())
                && matches!(
                    tx["status"].as_str(),
                    Some("preparing" | "committed" | "aborted")
                ),
            "invalid recovery transaction"
        );
        transactions.push((path, tx));
    }

    let mut aborted = Vec::new();
    let mut finalized = Vec::new();
    let mut aborted_ids = BTreeSet::new();
    for (path, mut tx) in transactions {
        if tx["status"] == "committed" {
            if let Some(path) = tx["manifest"].as_str() {
                let path = Path::new(path);
                pool_path(root, path)?;
                if path.try_exists()? {
                    let mut manifest: Value = read(path)?;
                    if manifest["schema"] == GENERATION_SCHEMA && manifest["status"] == "prepared" {
                        ensure!(
                            manifest["transaction"] == tx["id"],
                            "recovery transaction identity mismatch"
                        );
                        manifest["status"] = json!("committed");
                        ensure!(
                            records::committed(root, &manifest)?,
                            "recovery transaction is not committed"
                        );
                        manifest["recovered_at"] = json!(now());
                        write(path, &manifest, 0o644)?;
                        finalized.push(path.to_path_buf());
                    }
                }
            }
        } else {
            aborted_ids.insert(text(&tx, "id")?);
            if tx["status"] != "aborted" {
                tx["status"] = json!("aborted");
                tx["aborted_at"] = json!(now());
                write(&path, &tx, 0o644)?;
                aborted.push(path);
            }
        }
    }
    for path in children(&root.join("tmp"))? {
        remove(&path)?;
    }

    let mut removed_runtime = Vec::new();
    for path in children(&root.join("runtime"))? {
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let value: Value = read(&path)?;
        if !views::dm_exists(&text(&value, "dm_name")?).unwrap_or(true) {
            remove(&path)?;
            removed_runtime.push(path);
        }
    }

    let mut sealed = Vec::new();
    for id in segments::ids(root)? {
        let value: Value = read(&segments::metadata(root, &id)?)?;
        if value["sealed"] != true
            && value["created_by_transaction"]
                .as_str()
                .is_some_and(|tx| aborted_ids.contains(tx))
        {
            let _lock = PoolLock::acquire(&root.join("objects/segments.lock"))?;
            segments::seal(
                root,
                &id,
                &text(&value, "created_by_transaction")?,
                "recovery-after-aborted-transaction",
            )?;
            sealed.push(id);
        }
    }

    let mut removed = Vec::new();
    for domain in children(&root.join("domains"))? {
        pool_path(root, &domain.join("generations"))?;
        for path in children(&domain)? {
            if path
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with(".current.tmp-"))
            {
                remove(&path)?;
            }
        }
        for path in children(&domain.join("generations"))? {
            pool_path(root, &path)?;
            let manifest_path = path.join("manifest.json");
            let manifest: Option<Value> = if manifest_path.try_exists()? {
                Some(read(&manifest_path)?)
            } else {
                None
            };
            if manifest
                .as_ref()
                .is_none_or(|manifest| manifest["schema"] != GENERATION_SCHEMA)
                || !manifest
                    .as_ref()
                    .map(|manifest| records::committed(root, manifest))
                    .transpose()?
                    .unwrap_or(false)
            {
                let name = domain
                    .file_name()
                    .and_then(|s| s.to_str())
                    .context("invalid generation domain")?;
                let generation = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .context("invalid generation id")?
                    .parse()?;
                ensure!(
                    !protected.contains(&(name.to_owned(), generation)),
                    "recovery cannot discard a selected or running generation"
                );
                if let Some(manifest) = &manifest {
                    ensure!(
                        manifest["schema"] == GENERATION_SCHEMA
                            && manifest["status"] == "prepared"
                            && manifest["domain"] == name
                            && manifest["generation"] == generation
                            && manifest["transaction"]
                                .as_str()
                                .is_some_and(|id| aborted_ids.contains(id)),
                        "damaged generation is not an unfinished admission"
                    );
                }
                records::forget(root, name, generation, "incomplete-admission")?;
                removed.push(path);
            }
        }
    }
    let mut reconciled = Vec::new();
    for path in children(&root.join("objects/blob-metadata"))? {
        if path.extension().is_none_or(|s| s != "json")
            || path.file_stem().is_some_and(|s| s == ".tar-eof")
        {
            continue;
        }
        let value: Value = read(&path)?;
        let name = text(&value, "entry")?;
        if reconcile_object(root, &name, &value)? {
            reconciled.push(name);
        }
    }

    let refreshed = refresh_maps(root)?;
    Ok(json!({
        "schema": "airlock.closure-store.recovery.v1",
        "status": "passed",
        "pool_root": root,
        "aborted_transactions": aborted,
        "finalized_generations": finalized,
        "removed_incomplete_generations": removed,
        "removed_runtime_refs": removed_runtime,
        "sealed_aborted_segments": sealed,
        "reconciled_objects": reconciled,
        "refreshed_blob_maps": refreshed,
    }))
}

pub(super) fn status(root: &Path) -> Result<Value> {
    let live = liveness(root, 2)?;
    let locations = locations(root, &live)?;
    let PhysicalReferences {
        by_segment: references,
        stale_runtime: stale,
    } = physical_refs(root, &live, &locations)?;
    let inventory = inventory(root, &locations, &references)?;

    let mut domains: BTreeMap<String, Value> = BTreeMap::new();
    for ((domain, generation), (path, manifest)) in &live.manifests {
        let entry = domains
            .entry(domain.clone())
            .or_insert(json!({"current":records::current(root,domain)?,"generations":[]}));
        entry["generations"]
            .as_array_mut()
            .context("invalid domain generations")?
            .push(json!({
                "generation": generation,
                "manifest": path,
                "system": manifest["system"],
                "closure_hash": manifest["store"]["closure_hash"],
                "object_count": manifest["store"]["object_count"],
                "filesystem": "erofs",
                "view_backend": "erofs-objectmap",
            }));
    }

    let objects = children(&root.join("objects/metadata"))?
        .iter()
        .filter(|p| p.extension().is_some_and(|s| s == "json"))
        .count();
    let blobs = children(&root.join("objects/blob-metadata"))?
        .iter()
        .filter(|p| {
            p.extension().is_some_and(|s| s == "json")
                && p.file_stem().is_some_and(|s| s != ".tar-eof")
        })
        .count();
    Ok(json!({
        "schema": "airlock.closure-store.status.v1",
        "status": "passed",
        "pool_root": root,
        "object_count": objects,
        "object_blob_count": blobs,
        "object_segment_count": blobs,
        "segments_dir": root.join("objects/segments"),
        "segment_count": inventory.len(),
        "sealed_segment_count": inventory.iter().filter(|v|v["sealed"]==true).count(),
        "open_segment_count": inventory.iter().filter(|v|v["sealed"]!=true).count(),
        "segment_bytes_total": inventory.iter().filter_map(|v|v["used_bytes"].as_u64()).sum::<u64>(),
        "segments": inventory,
        "domains": domains,
        "running": records::running_refs(root)?.iter().map(|(_,v)|v).collect::<Vec<_>>(),
        "stale_runtime_view_refs": stale,
        "segment_compaction": "available",
    }))
}
