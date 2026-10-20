//! Host software views keep independent boot artifacts and generation-specific writable state.
//! Publication retains the view before a boot entry can refer to it; selection never reboots.

use super::{
    boot_artifacts, builder, database,
    fs::*,
    generations::{self, Admission},
    records, views,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub(super) const POOL: &str = "/var/lib/airlock/closure-store";

/// Persistent host-store layout shared with the generated boot configuration.
pub(super) const HOST: &str = airlock_machine_core::host_store::ROOT;

pub(super) const MACHINE: &str = "/var/lib/airlock/machine";

/// Open the same selected overlay configured for the host's Nix daemon.
pub(super) fn overlay_uri() -> String {
    airlock_machine_core::host_store::URI.into()
}

pub(super) fn validate(
    root: &Path,
    logical: &Path,
    system: &str,
    allow_retired: bool,
) -> Result<Value> {
    let path = root
        .join("host/boots")
        .join(format!("{}.json", entry(system)?));
    pool_path(root, &path)?;
    let value: Value = read(&path)?;
    ensure!(
        value["schema"] == "airlock.host-store.v1"
            && value["system"] == system
            && (allow_retired || value["retired"] != true),
        "host boot record identity mismatch or retired"
    );
    let physical = |path: &str| -> Result<PathBuf> {
        let path = Path::new(path);
        ensure!(
            !path
                .components()
                .any(|c| c == std::path::Component::ParentDir),
            "host metadata escapes pool"
        );
        Ok(root.join(path.strip_prefix(logical)?))
    };

    let manifest: Value = read(&physical(&text(&value, "manifest")?)?)?;
    ensure!(
        records::committed(root, &manifest)?
            && manifest["domain"] == "dom0"
            && manifest["generation"] == value["generation"]
            && manifest["store"]["closure_hash"] == value["closure_hash"]
            && manifest["store"]["canonical_metadata_image"] == value["metadata_image"]
            && manifest["store"]["canonical_blob_map"] == value["blob_map"],
        "host generation differs from boot record"
    );

    let paths: BTreeSet<String> = serde_json::from_value(manifest["store"]["paths"].clone())?;
    ensure!(
        paths.contains(system),
        "host system absent from admitted view"
    );
    for key in ["registration", "metadata_image", "blob_map"] {
        let path = physical(&text(&value, key)?)?;
        pool_path(root, &path)?;
        ensure!(
            hash(&path)? == text(&value, &format!("{key}_sha256"))?,
            "host {key} checksum mismatch"
        );
    }

    let registered = builder::registration(&fs::read_to_string(physical(&text(
        &value,
        "registration",
    )?)?)?)?;
    ensure!(
        registered.keys().cloned().collect::<BTreeSet<_>>() == paths,
        "host registration differs from view"
    );

    let generation = number(&value, "generation")?;
    let pin: Value = read(&records::running_path(
        root,
        "dom0",
        generation,
        "host-retained",
    )?)?;
    let map: Value = read(&physical(&text(&value, "blob_map")?)?)?;
    let refs = views::references(&map, &format!("running:dom0:{generation}:host-retained"))?;
    ensure!(
        pin["schema"] == "airlock.closure-store.running-ref.v1"
            && pin["status"] == "running"
            && pin["domain"] == "dom0"
            && pin["generation"] == generation
            && pin["manifest"] == value["manifest"]
            && pin["closure_hash"] == value["closure_hash"]
            && pin["physical_references"] == json!(refs),
        "host retention pin mismatch"
    );
    Ok(value)
}

fn admit(
    root: &Path,
    domain: &str,
    system: &str,
    registration: &str,
    incremental: bool,
    previous: Option<&Value>,
) -> Result<Value> {
    let registered = builder::registration(registration)?;
    ensure!(
        registered.contains_key(system),
        "system missing from host/builder registration"
    );

    let temporary = tempfile::tempdir_in(root.join("tmp"))?;
    let paths = format!(
        "{}\n",
        registered.keys().cloned().collect::<Vec<_>>().join("\n")
    );
    atomic(
        &temporary.path().join("store-paths"),
        paths.as_bytes(),
        0o644,
    )?;
    write(&temporary.path().join("manifest.json"), &json!({}), 0o644)?;

    let result = generations::admit(&Admission {
        pool: root,
        domain,
        system,
        source: temporary.path(),
        source_identity: &temporary.path().to_string_lossy(),
        root_source: "",
        kernel: "",
        physical: Path::new("/nix/store"),
        segment_target: SEGMENT_TARGET,
        verification: if incremental { "incremental" } else { "tree" },
        approved_by: "dom0",
        approval_source: "host-publication",
        activate: false,
        retain: true,
        previous_index: previous,
        device: "/dev/xvde",
        blob_device: "/dev/xvdf",
        mountpoint: "/nix/store",
    })?;

    let manifest = PathBuf::from(text(&result, "manifest")?);
    atomic(
        &manifest
            .parent()
            .context("missing generation parent")?
            .join("registration"),
        registration.as_bytes(),
        0o444,
    )?;
    Ok(result)
}

pub(super) fn publish(
    root: &Path,
    system: &str,
    registration: &str,
    systems: &[String],
    incremental: bool,
) -> Result<Value> {
    pool_path(root, &root.join("host/boots"))?;
    if root
        .join("host/boots")
        .join(format!("{}.json", entry(system)?))
        .try_exists()?
    {
        return validate(root, root, system, false);
    }

    let running = root.join("host/running.json");
    let previous = if running.try_exists()? {
        let running: Value = read(&running)?;
        let old = validate(root, root, &text(&running, "system")?, false)?;
        Some(read::<Value>(Path::new(&text(&old, "manifest")?))?["store"].clone())
    } else {
        None
    };

    let result = admit(
        root,
        "dom0",
        system,
        registration,
        incremental,
        previous.as_ref(),
    )?;
    let manifest_path = PathBuf::from(text(&result, "manifest")?);
    let manifest: Value = read(&manifest_path)?;

    let registered = builder::registration(registration)?;
    ensure!(
        systems.iter().all(|system| registered.contains_key(system)),
        "host boot choice absent from admitted closure"
    );
    let store = &manifest["store"];
    let record = json!({
        "schema": "airlock.host-store.v1",
        "generation": manifest["generation"],
        "manifest": manifest_path,
        "registration": manifest_path.parent().context("missing manifest parent")?.join("registration"),
        "registration_sha256": hex(&Sha256::digest(registration)),
        "metadata_image": store["canonical_metadata_image"],
        "metadata_image_sha256": hash(Path::new(&text(store,"canonical_metadata_image")?))?,
        "blob_map": store["canonical_blob_map"],
        "blob_map_sha256": hash(Path::new(&text(store,"canonical_blob_map")?))?,
        "closure_hash": store["closure_hash"],
    });

    records::running(
        root,
        "dom0",
        number(&manifest, "generation")?,
        "host-retained",
        true,
    )?;
    let map: Value = read(Path::new(&text(&record, "blob_map")?))?;
    let mut durable: BTreeSet<PathBuf> = views::extents(&map)?
        .iter()
        .map(|e| e.segment_blob.clone())
        .collect();
    for key in ["registration", "metadata_image", "blob_map"] {
        durable.insert(PathBuf::from(text(&record, key)?));
    }
    for path in durable {
        File::open(path)?.sync_all()?;
    }
    failpoint("host-before-boot-record")?;
    for choice in systems
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(system))
    {
        let mut selected = record.clone();
        selected["system"] = json!(choice);
        let path = root
            .join("host/boots")
            .join(format!("{}.json", entry(choice)?));
        pool_path(root, &path)?;
        write(&path, &selected, 0o644)?;
    }
    let mut result = record;
    result["system"] = json!(system);
    Ok(result)
}

pub(super) fn refresh_builder(
    root: &Path,
    role: &str,
    host_registration: &str,
    incremental: bool,
    seed_role: Option<&str>,
) -> Result<Value> {
    ensure!(
        ["sys-ctrl", "usr-assistant"].contains(&role),
        "invalid builder role"
    );
    let fresh = !root
        .join("builders")
        .join(format!("{role}.json"))
        .try_exists()?;
    let source = if fresh {
        seed_role.unwrap_or(role)
    } else {
        role
    };
    let previous: Value = read(&root.join("builders").join(format!("{source}.json")))?;
    let old_path = records::manifest_path(
        root,
        &format!("{source}-builder"),
        number(&previous, "generation")?,
    )?;
    let directory = old_path.parent().context("missing old builder directory")?;
    let registration = builder::merge(&[
        &fs::read_to_string(directory.join("registration"))?,
        host_registration,
    ])?;
    if !fresh && previous["registration_sha256"] == hex(&Sha256::digest(&registration)) {
        return Ok(previous);
    }
    let old: Value = read(&old_path)?;

    let result = admit(
        root,
        &format!("{role}-builder"),
        &text(&old, "system")?,
        &registration,
        incremental,
        Some(&old["store"]),
    )?;

    let manifest = PathBuf::from(text(&result, "manifest")?);
    builder::prepare(
        root,
        &result,
        &manifest
            .parent()
            .context("missing new builder directory")?
            .join("registration"),
        Some(&directory.join("flake-inputs.json")),
        if fresh { None } else { Some(&previous) },
    )
}

fn publication_registration(system: &str) -> Result<String> {
    let live = command(
        "nix-store",
        &["--store", &overlay_uri(), "--gc", "--print-live"],
        None,
    )?;
    let records = builder::registration(&command(
        "nix-store",
        &["--store", &overlay_uri(), "--dump-db"],
        None,
    )?)?;
    let mut roots: BTreeSet<String> = live.lines().map(str::to_owned).collect();
    roots.insert(system.to_owned());
    for path in children(&Path::new(HOST).join("active/upper"))? {
        let logical = format!(
            "/nix/store/{}",
            path.file_name()
                .context("invalid upper object")?
                .to_string_lossy()
        );
        if records.contains_key(&logical) {
            roots.insert(logical);
        }
    }
    let needed = builder::reference_closure(&records, &roots)?;
    Ok(needed
        .iter()
        .map(|path| records[path].text.as_str())
        .collect())
}

pub(super) fn publish_live(system: &str) -> Result<Value> {
    let root = Path::new(POOL);
    ensure_pool(root)?;
    let temporary = Path::new(HOST).join("publishing");
    mkdir_all(&temporary)?;

    let temporary = tempfile::tempdir_in(temporary)?;
    command(
        "nix-store",
        &[
            "--add-root",
            &temporary.path().join("root").to_string_lossy(),
            "--realise",
            system,
        ],
        None,
    )?;
    let registration = publication_registration(system)?;
    let _lock = PoolLock::acquire(&root.join("admission.lock"))?;
    let record = publish(root, system, &registration, &[], true)?;
    let directory = Path::new(HOST)
        .join("generations")
        .join(number(&record, "generation")?.to_string());
    if !directory.join("nix/var/nix/db/db.sqlite").try_exists()? {
        database::seed(
            Path::new("/nix/var/nix/db/db.sqlite"),
            &directory.join("seed-metadata.sqlite"),
        )?;
    }
    refresh_builder(root, "sys-ctrl", &registration, true, None)?;
    refresh_builder(root, "usr-assistant", &registration, true, Some("sys-ctrl"))?;
    Ok(record)
}

pub(super) fn select(root: &Path, esp: &Path, system: &str, firmware: bool) -> Result<Value> {
    let record = validate(root, root, system, false)?;
    let entry = boot_artifacts::verify(root, esp, &record)?;
    boot_artifacts::select_entry(esp, &entry, firmware)?;
    write(&root.join("host/selected.json"), &record, 0o644)?;
    Ok(record)
}

pub(super) fn retire(root: &Path, esp: &Path, system: &str) -> Result<Value> {
    for name in ["selected", "running"] {
        let path = root.join(format!("host/{name}.json"));
        if path.try_exists()? {
            ensure!(
                read::<Value>(&path)?["system"] != system,
                "cannot retire {name} host"
            );
        }
    }
    let path = root
        .join("host/boots")
        .join(format!("{}.json", entry(system)?));
    if path.try_exists()? {
        let mut record = validate(root, root, system, true)?;
        record["retired"] = json!(true);
        write(&path, &record, 0o644)?;
    }
    boot_artifacts::forget(root, esp, system)?;
    let mut groups: BTreeMap<u64, Vec<(PathBuf, Value)>> = BTreeMap::new();
    for path in children(&root.join("host/boots"))? {
        let value: Value = read(&path)?;
        groups
            .entry(number(&value, "generation")?)
            .or_default()
            .push((path, value));
    }
    for (generation, records) in groups {
        if !records.iter().all(|(_, v)| v["retired"] == true) {
            continue;
        }
        let refs = root.join("running/dom0").join(generation.to_string());
        if children(&refs)?
            .iter()
            .any(|p| p.file_name().is_none_or(|s| s != "host-retained.json"))
        {
            continue;
        }
        let path = records::manifest_path(root, "dom0", generation)?;
        let mut manifest: Value = read(&path)?;
        manifest["retention"]["retain"] = json!(false);
        write(&path, &manifest, 0o644)?;
        for (path, _) in records {
            remove(&path)?;
        }
        sync_dir(&root.join("host/boots"))?;
        records::running(root, "dom0", generation, "host-retained", false)?;
    }
    Ok(json!({"system":system,"retired":true}))
}

pub(super) fn allocated(path: &Path) -> Result<u64> {
    let mut seen = BTreeSet::new();
    let mut size = 0;
    for path in descendants(path)? {
        let stat = fs::symlink_metadata(path)?;
        if seen.insert((stat.dev(), stat.ino())) {
            size += stat.blocks() * 512;
        }
    }
    Ok(size)
}
