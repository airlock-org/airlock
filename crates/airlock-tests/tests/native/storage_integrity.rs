//! Admission and compaction reject damaged data before publishing or reclaiming retained storage.

use crate::common::{self, store::Store};
use anyhow::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
};

/// Read metadata only after the production command has completed its durable publication.
fn read(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

/// Snapshot contents and modification times so read-only operations cannot silently rewrite metadata.
pub(super) fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, (Vec<u8>, u128)>> {
    let mut result = BTreeMap::new();
    if !root.exists() {
        return Ok(result);
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() && !path.is_symlink() {
            for (relative, value) in snapshot(&path)? {
                result.insert(path.strip_prefix(root)?.join(relative), value);
            }
        } else if path.is_file() {
            let changed = path
                .metadata()?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos();
            result.insert(
                path.strip_prefix(root)?.to_owned(),
                (fs::read(&path)?, changed),
            );
        }
    }
    Ok(result)
}

/// Collect only committed manifests, keeping expected rejection checks independent of staging layout.
fn manifests(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    Ok(snapshot(root)?
        .into_iter()
        .filter(|(path, _)| path.file_name().is_some_and(|name| name == "manifest.json"))
        .map(|(path, (bytes, _))| (path, bytes))
        .collect())
}

/// Rewrite an actual produced archive so corruption cases exercise the real admission stream parser.
fn archive(
    payload: Vec<u8>,
    mutate: impl FnOnce(&mut BTreeMap<String, Vec<u8>>) -> Result<()>,
) -> Result<Vec<u8>> {
    let mut contents = BTreeMap::new();
    for entry in tar::Archive::new(payload.as_slice()).entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_str().unwrap().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        contents.insert(name, bytes);
    }
    mutate(&mut contents)?;
    let mut result = tar::Builder::new(Vec::new());
    for (name, bytes) in contents {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o444);
        header.set_cksum();
        result.append_data(&mut header, name, bytes.as_slice())?;
    }
    Ok(result.into_inner()?)
}

/// Invalid object identity, provenance or bounds cannot replace even an already committed generation.
#[test]
fn artifact_manifest_rejection_preserves_existing_generations() -> Result<()> {
    for damage in [
        "domain",
        "system",
        "revision",
        "lock",
        "count",
        "references",
        "duplicate",
        "entry",
        "fragment",
        "tree",
        "nar-size",
        "nar-hash",
    ] {
        let store = Store::new()?;
        let prior = store.admit(true)?;
        let before = manifests(&store.pool.join("domains"))?;
        let payload = archive(store.artifact()?, |members| {
            let mut manifest: Value = serde_json::from_slice(&members["manifest.json"])?;
            match damage {
                "domain" => manifest["domain"] = json!("other-vm"),
                "system" => {
                    manifest["system"] =
                        json!(format!("/nix/store/{hash}-missing", hash = "z".repeat(32)))
                }
                "revision" => manifest["source_revision"] = json!("dirty"),
                "lock" => manifest["lock_hash"] = json!("invalid"),
                "count" => {
                    manifest["object_count"] = json!(manifest["object_count"].as_u64().unwrap() + 1)
                }
                "references" => {
                    manifest["objects"][0]["references"] =
                        json!([format!("/nix/store/{hash}-missing", hash = "z".repeat(32))])
                }
                "duplicate" => {
                    let object = manifest["objects"][0].clone();
                    manifest["objects"].as_array_mut().unwrap().push(object);
                    manifest["object_count"] =
                        json!(manifest["object_count"].as_u64().unwrap() + 1);
                }
                "entry" => manifest["objects"][0]["entry"] = json!("../escape"),
                "fragment" => manifest["objects"][0]["fragment"] = json!("../escape.tar"),
                "tree" => manifest["objects"][0]["tree_hash"] = json!("0".repeat(64)),
                "nar-size" => manifest["objects"][0]["nar_size"] = json!(-1),
                "nar-hash" => manifest["objects"][0]["nar_hash"] = json!(""),
                _ => unreachable!(),
            }
            members.insert("manifest.json".into(), serde_json::to_vec(&manifest)?);
            Ok(())
        })?;
        assert!(
            !common::run(&mut store.receiver(), Some(payload))?
                .status
                .success(),
            "accepted {damage}"
        );
        assert_eq!(manifests(&store.pool.join("domains"))?, before);
        assert_eq!(
            store.run(&[
                "pool",
                "describe-generation",
                "--pool-root",
                store.pool_text(),
                "--domain",
                "usr-home",
                "--generation",
                &prior["generation"].to_string()
            ])?["selected"],
            true
        );
        assert_eq!(fs::read_dir(store.pool.join("tmp"))?.count(), 0);
    }
    let store = Store::new()?;
    let payload = store.artifact()?;
    let first = common::json(&mut store.receiver(), Some(payload.clone()))?;
    let second = common::json(&mut store.receiver(), Some(payload))?;
    assert_eq!(first["generation"], second["generation"]);
    assert_eq!(second["objects_admitted"], 0);
    Ok(())
}

/// Negotiation rejects a substituted prepared identity before asking the sender for any object bytes.
#[test]
fn artifact_negotiation_rejects_changed_prepared_identity_and_bad_frames() -> Result<()> {
    let store = Store::new()?;
    let identity = store.temporary.path().join("identity.json");
    let inventory = json!({"schema": "airlock.pool-artifact.v4", "domain": "usr-home", "system": store.system,
        "closure_source": store.closure, "root_source": store.root, "kernel_name": "",
        "source_revision": "a".repeat(40), "lock_hash": "b".repeat(64), "root_artifact": null});
    let mut expected = inventory.clone();
    expected["root_artifact"] = json!({"changed": true});
    expected.as_object_mut().unwrap().remove("schema");
    fs::write(&identity, serde_json::to_vec(&expected)?)?;
    let receiver = || {
        let mut command = store.command(&["artifact", "receive-incremental", "--pool-root"]);
        command.arg(&store.pool).args([
            "--expected-domain",
            "usr-home",
            "--approved-by",
            "sys-ctrl",
        ]);
        command
    };
    let mut wire = serde_json::to_vec(&inventory)?;
    wire.push(b'\n');
    let output = common::run(
        receiver().arg("--expected-identity").arg(identity),
        Some(wire),
    )?;
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(error["schema"], "airlock.pool-artifact-error.v1");
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("prepared candidate")
    );
    assert!(manifests(&store.pool.join("domains"))?.is_empty());
    for message in [b"{}".as_slice(), b"{invalid}\n", b"[]\n"] {
        let output = common::run(&mut receiver(), Some(message.to_vec()))?;
        assert!(!output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout)?["schema"],
            "airlock.pool-artifact-error.v1"
        );
    }
    Ok(())
}

/// Sparse identity depends on disk contents rather than host allocation and remains independent of private edits.
#[test]
fn root_artifacts_preserve_sparse_disk_identity_and_private_copy_independence() -> Result<()> {
    let store = Store::new()?;
    let dense = store.temporary.path().join("dense.img");
    fs::write(&dense, fs::read(&store.root)?)?;
    let expected = common::json(
        store
            .command(&["image", "root-descriptor"])
            .arg(&store.root),
        None,
    )?;
    assert_eq!(
        common::json(
            store.command(&["image", "root-descriptor"]).arg(dense),
            None
        )?,
        expected
    );
    assert!(
        expected["payload_bytes"].as_u64().unwrap()
            < expected["logical_bytes"].as_u64().unwrap() / 10
    );
    let receipt = common::json(&mut store.receiver(), Some(store.artifact()?))?;
    let retained = store
        .pool
        .join("root-artifacts")
        .join(expected["sha256"].as_str().unwrap())
        .join("root.img");
    assert_eq!(receipt["root_artifact"], expected);
    assert_eq!(fs::read(&retained)?, fs::read(&store.root)?);
    assert!(retained.metadata()?.blocks() * 512 < retained.metadata()?.len() / 10);
    let private = store.temporary.path().join("private.img");
    fs::copy(&retained, &private)?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600))?;
    let mut file = fs::OpenOptions::new().write(true).open(&private)?;
    file.seek(SeekFrom::Start(2 * 65536))?;
    file.write_all(b"private change")?;
    assert_eq!(
        common::json(
            store.command(&["image", "root-descriptor"]).arg(retained),
            None
        )?,
        expected
    );
    assert_ne!(
        common::json(
            store.command(&["image", "root-descriptor"]).arg(private),
            None
        )?,
        expected
    );
    Ok(())
}

/// Sparse stream corruption, overlap, unsupported formats and redirected retention roots cannot publish artifacts.
#[test]
fn damaged_or_redirected_root_artifacts_are_never_retained() -> Result<()> {
    for damage in [
        "checksum",
        "truncated",
        "overlap",
        "format",
        "abi",
        "zero-extent",
    ] {
        let store = Store::new()?;
        let payload = archive(store.artifact()?, |members| {
            let mut manifest: Value = serde_json::from_slice(&members["manifest.json"])?;
            match damage {
                "checksum" => manifest["root_artifact"]["sha256"] = json!("0".repeat(64)),
                "truncated" => {
                    members.get_mut("root.sparse").unwrap().pop();
                }
                "overlap" | "zero-extent" => {
                    let data = members.get_mut("root.sparse").unwrap();
                    let first = b"AIRLOCK-ROOT-SPARSE-1\n".len() + 8;
                    if damage == "overlap" {
                        let second = first + 12 + 65536;
                        data[second..second + 8].fill(0);
                    } else {
                        data[first + 12..first + 12 + 65536].fill(0);
                    }
                    manifest["root_artifact"]["sha256"] =
                        json!(format!("{hash:x}", hash = Sha256::digest(data)));
                }
                "format" => manifest["root_artifact"]["format"] = json!("unsupported"),
                "abi" => manifest["root_artifact"]["boot_abi"] = json!("unsupported"),
                _ => unreachable!(),
            }
            members.insert("manifest.json".into(), serde_json::to_vec(&manifest)?);
            Ok(())
        })?;
        assert!(
            !common::run_file(&mut store.receiver(), &payload)?
                .status
                .success(),
            "accepted {damage}"
        );
        assert!(manifests(&store.pool.join("domains"))?.is_empty());
        assert_eq!(fs::read_dir(store.pool.join("tmp"))?.count(), 0);
        let retained = store.pool.join("root-artifacts");
        if retained.is_dir() {
            for entry in fs::read_dir(retained)? {
                assert!(!entry?.path().join("root.img").exists());
            }
        }
    }
    let store = Store::new()?;
    store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    let outside = store.temporary.path().join("outside");
    fs::create_dir(&outside)?;
    symlink(&outside, store.pool.join("root-artifacts"))?;
    assert!(
        !common::run_file(&mut store.receiver(), &store.artifact()?)?
            .status
            .success()
    );
    assert_eq!(fs::read_dir(outside)?.count(), 0);
    Ok(())
}

/// Unsupported pool formats remain byte-for-byte intact, including their directory layout.
#[test]
fn unsupported_pool_formats_and_invalid_extents_fail_before_mutation() -> Result<()> {
    let store = Store::new()?;
    store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    let before = snapshot(&store.pool)?;
    store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    assert_eq!(snapshot(&store.pool)?, before);
    for layout in ["single-blob", "unsegmented-manifest", "wrong-schema"] {
        let store = Store::new()?;
        fs::create_dir(&store.pool)?;
        if layout == "single-blob" {
            fs::create_dir(store.pool.join("objects"))?;
            fs::write(store.pool.join("objects/data.blob"), b"retained data")?;
        } else {
            let value = if layout == "unsegmented-manifest" {
                json!({"schema": "airlock.closure-store.v1", "object_key": "nix-store-basename", "object_store": {"data": "objects/data.blob"}})
            } else {
                json!({"schema": "unknown"})
            };
            fs::write(
                store.pool.join("MANIFEST.json"),
                serde_json::to_vec(&value)?,
            )?;
        }
        let before = snapshot(&store.pool)?;
        let directories = fs::read_dir(&store.pool)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<BTreeSet<_>>>()?;
        assert!(
            !common::run(
                &mut store.command(&["pool", "status", "--pool-root", store.pool_text()]),
                None
            )?
            .status
            .success()
        );
        assert_eq!(snapshot(&store.pool)?, before);
        assert_eq!(
            fs::read_dir(&store.pool)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<BTreeSet<_>>>()?,
            directories
        );
    }
    for (field, value) in [
        ("segment_id", json!("../escape")),
        ("segment_offset", json!(-1)),
        ("length", json!(u64::MAX)),
        ("logical_offset", json!(512)),
    ] {
        let store = Store::new()?;
        let receipt = store.admit(true)?;
        let path = Path::new(receipt["canonical_blob_map"].as_str().unwrap());
        let mut map = read(path)?;
        map["extents"][0][field] = value;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
        fs::write(path, serde_json::to_vec(&map)?)?;
        assert!(
            !common::run(
                &mut store.command(&[
                    "pool",
                    "attach-blob-view",
                    "--pool-root",
                    store.pool_text(),
                    "--domain",
                    "usr-home",
                    "--generation",
                    "1"
                ]),
                None
            )?
            .status
            .success()
        );
        assert_eq!(fs::read_dir(store.pool.join("runtime"))?.count(), 0);
    }
    Ok(())
}

/// Create a partially live segment by retaining one object from an earlier larger generation.
fn fragmented() -> Result<(Store, Value)> {
    let store = Store::new()?;
    let mut additions = Vec::new();
    for letter in ["b", "c", "d"] {
        let name = format!("{hash}-object", hash = letter.repeat(32));
        fs::write(store.physical.join(&name), letter.as_bytes().repeat(8192))?;
        additions.push(format!("/nix/store/{name}"));
    }
    fs::write(
        store.closure.join("store-paths"),
        format!(
            "{system}\n{objects}\n",
            system = store.system,
            objects = additions.join("\n")
        ),
    )?;
    let first = common::json(
        store.admission().args(["--segment-target-bytes", "32768"]),
        None,
    )?;
    fs::write(
        store.closure.join("store-paths"),
        format!(
            "{system}\n{object}\n",
            system = store.system,
            object = additions[0]
        ),
    )?;
    let second = common::json(
        store.admission().args(["--segment-target-bytes", "32768"]),
        None,
    )?;
    store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(!Path::new(first["manifest"].as_str().unwrap()).exists());
    Ok((store, second))
}

/// Limit each relocation scenario to one small segment without changing production compaction policy.
fn compact(store: &Store) -> Command {
    store.command(&[
        "pool",
        "compact",
        "--pool-root",
        store.pool_text(),
        "--threshold-percent",
        "80",
        "--max-segments",
        "1",
        "--keep-generations",
        "0",
        "--segment-target-bytes",
        "32768",
    ])
}

/// Planning is read-only, and running disk maps retain old bytes until their owner explicitly releases them.
#[test]
fn compaction_preserves_running_segments_until_pin_release() -> Result<()> {
    let (store, receipt) = fragmented()?;
    let before = snapshot(&store.pool)?;
    let plan = common::json(compact(&store).args(["--dry-run", "true"]), None)?;
    assert!(!plan["candidate_segments"].as_array().unwrap().is_empty());
    assert_eq!(snapshot(&store.pool)?, before);
    let generation = receipt["generation"].to_string();
    store.run(&[
        "pool",
        "mark-running",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        &generation,
        "--volume",
        "store-data",
    ])?;
    let candidates: BTreeSet<_> = plan["candidate_segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect();
    let result = common::json(&mut compact(&store), None)?;
    assert!(!result["objects_moved"].as_array().unwrap().is_empty());
    assert!(candidates.iter().all(|id| {
        result["segments_blocked_from_deletion"]
            .as_array()
            .unwrap()
            .contains(&json!(id))
    }));
    store.run(&[
        "pool",
        "clear-running",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        &generation,
        "--volume",
        "store-data",
    ])?;
    let result = store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(candidates.iter().all(|id| {
        result["removed_segments"]
            .as_array()
            .unwrap()
            .contains(&json!(id))
    }));
    assert_eq!(
        store.run(&[
            "pool",
            "describe-generation",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "usr-home",
            "--generation",
            &generation
        ])?["selected"],
        true
    );
    Ok(())
}

/// Every durable relocation boundary can recover a usable selected view; damaged backing aborts reclamation.
#[test]
fn interrupted_or_corrupt_compaction_preserves_selected_data() -> Result<()> {
    for failure in [
        "compact-after-tx-create",
        "compact-after-object-copy",
        "compact-between-location-records",
        "compact-after-location-publish",
        "compact-after-view-refresh",
        "compact-after-tx-commit",
    ] {
        let (store, receipt) = fragmented()?;
        assert!(
            !common::run(
                compact(&store).env("AIRLOCK_CLOSURE_STORE_FAILPOINT", failure),
                None
            )?
            .status
            .success()
        );
        store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
        let result = store.run(&[
            "pool",
            "load-generation",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "usr-home",
            "--generation",
            &receipt["generation"].to_string(),
        ])?;
        assert_eq!(result["system"], store.system);
        common::json(compact(&store).args(["--dry-run", "true"]), None)?;
    }
    let (store, _) = fragmented()?;
    let metadata = fs::read_dir(store.pool.join("objects/blob-metadata"))?
        .filter_map(std::result::Result::ok)
        .find(|entry| entry.file_name().as_encoded_bytes().starts_with(b"a"))
        .unwrap()
        .path();
    let value = read(metadata)?;
    let path = Path::new(value["segment_blob"].as_str().unwrap());
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let mut file = fs::OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(value["segment_offset"].as_u64().unwrap()))?;
    file.write_all(b"damaged")?;
    let before = snapshot(&store.pool)?;
    assert!(!common::run(&mut compact(&store), None)?.status.success());
    assert_eq!(snapshot(&store.pool)?, before);
    Ok(())
}

/// Reconstruct separately retained extents only for an explicitly prepared real EROFS index check.
fn extract(store: &Store, receipt: &Value) -> Result<PathBuf> {
    let directory = store.temporary.path().join("extracted");
    let external = store.temporary.path().join("external.tar");
    let mut file = File::create(&external)?;
    for extent in read(receipt["canonical_blob_map"].as_str().unwrap())?["extents"]
        .as_array()
        .unwrap()
    {
        let mut source = File::open(extent["segment_blob"].as_str().unwrap())?;
        source.seek(SeekFrom::Start(extent["segment_offset"].as_u64().unwrap()))?;
        file.seek(SeekFrom::Start(extent["logical_offset"].as_u64().unwrap()))?;
        let length = extent["length"].as_u64().unwrap();
        assert_eq!(std::io::copy(&mut source.take(length), &mut file)?, length);
    }
    common::run(
        Command::new(common::tool("fsck.erofs")?)
            .arg(format!("--device={path}", path = external.display()))
            .arg(format!("--extract={path}", path = directory.display()))
            .arg(receipt["canonical_metadata_image"].as_str().unwrap()),
        None,
    )?
    .checked()?;
    Ok(directory)
}

/// Real additive indexes read both object generations without modifying their predecessor's metadata image.
#[test]
#[ignore = "requires explicitly prepared AIRLOCK_EROFS_MKFS with Airlock tar-offset support and fsck.erofs"]
fn packaged_erofs_indexes_preserve_predecessors_and_reject_corruption() -> Result<()> {
    let mkfs = std::env::var_os("AIRLOCK_EROFS_MKFS")
        .expect("prepare the patched EROFS package explicitly");
    let store = Store::new()?;
    let first = common::json(store.admission().env("AIRLOCK_MKFS_EROFS", &mkfs), None)?;
    let before = fs::read(first["canonical_metadata_image"].as_str().unwrap())?;
    let addition = format!("{hash}-addition", hash = "0".repeat(32));
    let relative = Path::new(&addition)
        .join("long-name-")
        .join("x".repeat(160));
    fs::create_dir_all(store.physical.join(&relative).parent().unwrap())?;
    fs::write(store.physical.join(&relative), "new payload")?;
    let mut paths = fs::OpenOptions::new()
        .append(true)
        .open(store.closure.join("store-paths"))?;
    writeln!(paths, "/nix/store/{addition}")?;
    let second = common::json(
        store
            .admission()
            .env("AIRLOCK_MKFS_EROFS", &mkfs)
            .args(["--previous-generation", &first["generation"].to_string()]),
        None,
    )?;
    assert_eq!(second["store"]["index_depth"], 1);
    assert_eq!(
        fs::read(first["canonical_metadata_image"].as_str().unwrap())?,
        before
    );
    let target = extract(&store, &second)?;
    assert_eq!(
        fs::read_to_string(
            target
                .join(Path::new(&store.system).file_name().unwrap())
                .join("init")
        )?,
        "system fixture"
    );
    assert_eq!(fs::read_to_string(target.join(relative))?, "new payload");

    let store = Store::new()?;
    let first = common::json(store.admission().env("AIRLOCK_MKFS_EROFS", &mkfs), None)?;
    let path = Path::new(first["canonical_metadata_image"].as_str().unwrap());
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    fs::write(path, b"corrupt metadata")?;
    let addition = format!("{hash}-addition", hash = "b".repeat(32));
    fs::write(store.physical.join(&addition), "new")?;
    let mut paths = fs::OpenOptions::new()
        .append(true)
        .open(store.closure.join("store-paths"))?;
    writeln!(paths, "/nix/store/{addition}")?;
    let output = common::run(
        store
            .admission()
            .env("AIRLOCK_MKFS_EROFS", mkfs)
            .args(["--previous-generation", &first["generation"].to_string()]),
        None,
    )?;
    assert!(!output.status.success());
    assert_eq!(
        store.run(&[
            "pool",
            "describe-generation",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "usr-home",
            "--generation",
            &first["generation"].to_string()
        ])?["selected"],
        true
    );
    Ok(())
}

/// Normal admission invokes actual Nix verification before publication and keeps small objects in one open segment.
#[test]
fn pool_verification_and_segment_packing_preserve_admission_boundaries() -> Result<()> {
    let store = Store::new()?;
    common::peer_link(store.temporary.path(), "nix")?;
    let arguments = store.temporary.path().join("nix-verify.json");
    let path = format!(
        "{root}:{path}",
        root = store.temporary.path().display(),
        path = std::env::var("PATH")?
    );
    let receipt = common::json(
        store
            .admission_mode("nix-store")
            .env("PATH", &path)
            .env("AIRLOCK_TEST_NIX_ARGS", &arguments),
        None,
    )?;
    assert_eq!(receipt["verification"]["mode"], "nix-store");
    let command = read(&arguments)?;
    for option in ["--extra-experimental-features", "nix-command"] {
        assert!(command.as_array().unwrap().contains(&json!(option)));
    }
    let rejected_pool = store.temporary.path().join("rejected-pool");
    // Replace the destination argument without duplicating a clap option or weakening production verification.
    let mut command = store.admission_mode("nix-store");
    let args: Vec<_> = command
        .get_args()
        .map(|value| {
            if value == store.pool.as_os_str() {
                rejected_pool.clone().into_os_string()
            } else {
                value.to_owned()
            }
        })
        .collect();
    command = store.command(&[]);
    command
        .args(args.into_iter().skip(1))
        .env("PATH", path)
        .env("AIRLOCK_TEST_NIX_ARGS", arguments)
        .env("AIRLOCK_TEST_NIX_EXIT", "1");
    assert!(!common::run(&mut command, None)?.status.success());
    assert!(manifests(&rejected_pool.join("domains"))?.is_empty());

    let store = Store::new()?;
    let mut objects = vec![store.system.clone()];
    for letter in ["b", "c", "d"] {
        let name = format!("{hash}-small", hash = letter.repeat(32));
        fs::write(store.physical.join(&name), letter.repeat(1024))?;
        objects.push(format!("/nix/store/{name}"));
    }
    fs::write(
        store.closure.join("store-paths"),
        format!("{paths}\n", paths = objects.join("\n")),
    )?;
    common::json(
        store
            .admission()
            .args(["--segment-target-bytes", "1048576"]),
        None,
    )?;
    let mut segments = BTreeSet::new();
    for path in objects {
        let name = Path::new(&path).file_name().unwrap();
        let value = read(
            store
                .pool
                .join("objects/blob-metadata")
                .join(name)
                .with_extension("json"),
        )?;
        segments.insert(value["segment_id"].clone().as_str().unwrap().to_owned());
    }
    assert_eq!(segments.len(), 1);
    let status = store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    assert_eq!(status["segment_count"], 1);
    assert_eq!(status["open_segment_count"], 1);
    Ok(())
}

/// In-progress object metadata stays protected until recovery aborts it; committed generations finalize without activation.
#[test]
fn recovery_distinguishes_active_objects_from_committed_unselected_generations() -> Result<()> {
    let store = Store::new()?;
    let failed = common::run(
        store
            .admission()
            .env("AIRLOCK_CLOSURE_STORE_FAILPOINT", "object-after-metadata"),
        None,
    )?;
    assert!(!failed.status.success());
    let name = Path::new(&store.system)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let gc = store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(!gc["active_transaction_ids"].as_array().unwrap().is_empty());
    assert!(
        !gc["removed_object_blobs"]
            .as_array()
            .unwrap()
            .contains(&json!(name))
    );
    store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
    assert!(
        store.run(&["pool", "status", "--pool-root", store.pool_text()])?["domains"]
            .get("usr-home")
            .is_none()
    );
    let gc = store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(
        gc["removed_object_blobs"]
            .as_array()
            .unwrap()
            .contains(&json!(name))
    );

    let store = Store::new()?;
    let failed = common::run(
        store.admission().args(["--activate", "false"]).env(
            "AIRLOCK_CLOSURE_STORE_FAILPOINT",
            "generation-after-tx-commit",
        ),
        None,
    )?;
    assert!(!failed.status.success());
    assert_eq!(
        read(
            store
                .pool
                .join("domains/usr-home/generations/1/manifest.json")
        )?["status"],
        "prepared"
    );
    let gc = store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(
        gc["in_progress_generations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["domain"] == "usr-home" && entry["generation"] == 1)
    );
    let recovered = store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
    assert_eq!(
        recovered["finalized_generations"].as_array().unwrap().len(),
        1
    );
    let status = store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    assert!(status["domains"]["usr-home"]["current"].is_null());
    assert_eq!(
        status["domains"]["usr-home"]["generations"][0]["generation"],
        1
    );
    Ok(())
}

/// Switching honors its atomic publication boundary; interrupted GC retains the selected generation.
#[test]
fn interrupted_switch_and_gc_preserve_the_last_published_selection() -> Result<()> {
    let store = Store::new()?;
    store.admit(true)?;
    let addition = format!("{hash}-new", hash = "b".repeat(32));
    fs::write(store.physical.join(&addition), "new data")?;
    let mut paths = fs::OpenOptions::new()
        .append(true)
        .open(store.closure.join("store-paths"))?;
    writeln!(paths, "/nix/store/{addition}")?;
    store.admit(false)?;
    for (failure, expected) in [
        ("switch-before-current-replace", 1),
        ("switch-after-current-replace", 2),
    ] {
        let output = common::run(
            store
                .command(&[
                    "pool",
                    "switch",
                    "--pool-root",
                    store.pool_text(),
                    "--domain",
                    "usr-home",
                    "--generation",
                    "2",
                ])
                .env("AIRLOCK_CLOSURE_STORE_FAILPOINT", failure),
            None,
        )?;
        assert!(!output.status.success());
        store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
        assert_eq!(
            store.run(&["pool", "status", "--pool-root", store.pool_text()])?["domains"]["usr-home"]
                ["current"],
            expected
        );
    }
    store.run(&[
        "pool",
        "rollback",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        "1",
    ])?;
    let output = common::run(
        store
            .command(&[
                "pool",
                "gc",
                "--pool-root",
                store.pool_text(),
                "--keep-generations",
                "0",
            ])
            .env(
                "AIRLOCK_CLOSURE_STORE_FAILPOINT",
                "gc-after-object-metadata-remove",
            ),
        None,
    )?;
    assert!(!output.status.success());
    store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
    let status = store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    assert_eq!(status["domains"]["usr-home"]["current"], 1);
    assert_eq!(
        status["domains"]["usr-home"]["generations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    Ok(())
}
