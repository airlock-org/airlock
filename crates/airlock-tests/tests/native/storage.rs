//! Admission recovery and artifact protocols use real services on ordinary temporary files.

use crate::common::{self, store::Store};
use anyhow::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, Write},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

/// Read one transaction or generation record without changing its representation.
fn read(path: impl AsRef<Path>) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

/// Enumerate committed manifests while keeping assertions independent of directory order.
fn manifests(store: &Store) -> Result<Vec<std::path::PathBuf>> {
    let domains = store.pool.join("domains");
    let mut result = Vec::new();
    if !domains.exists() {
        return Ok(result);
    }
    for domain in fs::read_dir(domains)? {
        let generations = domain?.path().join("generations");
        if !generations.is_dir() {
            continue;
        }
        for entry in fs::read_dir(generations)? {
            let path = entry?.path().join("manifest.json");
            if path.exists() {
                result.push(path);
            }
        }
    }
    Ok(result)
}

/// Running references retain bytes until explicitly cleared; generation numbers remain monotonic.
#[test]
fn admission_selection_running_pins_and_gc() -> Result<()> {
    let store = Store::new()?;
    let first = store.admit(true)?;
    let second = store.admit(false)?;
    assert_eq!(
        (first["generation"].as_u64(), second["generation"].as_u64()),
        (Some(1), Some(2))
    );
    assert_eq!(first["objects_admitted"], 1);
    assert_eq!(second["objects_reused"], 1);
    assert_eq!(
        store.run(&[
            "pool",
            "describe-generation",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "usr-home",
            "--generation",
            "1"
        ])?["selected"],
        true
    );

    store.run(&[
        "pool",
        "mark-running",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        "2",
        "--volume",
        "store-data",
    ])?;
    let collected = store.run(&[
        "pool",
        "gc",
        "--pool-root",
        store.pool_text(),
        "--keep-generations",
        "0",
    ])?;
    assert!(
        collected["retained_generations"]
            .as_array()
            .unwrap()
            .contains(&json!({"domain":"usr-home","generation":2}))
    );
    store.run(&[
        "pool",
        "clear-running",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        "2",
        "--volume",
        "store-data",
    ])?;
    assert_eq!(
        store.run(&[
            "pool",
            "gc",
            "--pool-root",
            store.pool_text(),
            "--keep-generations",
            "0"
        ])?["removed_generations"],
        json!([{"domain":"usr-home","generation":2}])
    );
    store.run(&[
        "pool",
        "retire-generation",
        "--pool-root",
        store.pool_text(),
        "--domain",
        "usr-home",
        "--generation",
        "1",
    ])?;
    assert_eq!(
        store.run(&[
            "pool",
            "gc",
            "--pool-root",
            store.pool_text(),
            "--keep-generations",
            "0"
        ])?["removed_root_artifacts"],
        json!([first["root_artifact"]["sha256"]])
    );
    assert!(store.admit(true)?["generation"].as_u64().unwrap() > 1);
    Ok(())
}

/// Every publication failpoint recovers without selecting a partially committed generation.
#[test]
fn interrupted_admissions_preserve_the_selected_committed_view() -> Result<()> {
    for failure in [
        "admit-after-tx-create",
        "object-after-append",
        "generation-before-manifest",
        "generation-after-manifest",
        "generation-after-tx-commit",
        "generation-after-manifest-commit",
        "switch-before-current-replace",
    ] {
        let store = Store::new()?;
        store.admit(true)?;
        let addition = format!("{hash}-addition", hash = "b".repeat(32));
        fs::write(store.physical.join(&addition), b"new object")?;
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(store.closure.join("store-paths"))?,
            "/nix/store/{addition}"
        )?;
        let output = common::run(
            store
                .admission()
                .env("AIRLOCK_CLOSURE_STORE_FAILPOINT", failure),
            None,
        )?;
        assert!(!output.status.success(), "{failure}");

        store.run(&["pool", "recover", "--pool-root", store.pool_text()])?;
        assert_eq!(
            store.run(&[
                "pool",
                "describe-generation",
                "--pool-root",
                store.pool_text(),
                "--domain",
                "usr-home",
                "--generation",
                "1"
            ])?["selected"],
            true,
            "{failure}"
        );
        for manifest in manifests(&store)? {
            let manifest = read(manifest)?;
            assert_eq!(manifest["status"], "committed", "{failure}");
            let transaction = manifest["transaction"].as_str().unwrap();
            assert_eq!(
                read(store.pool.join(format!("transactions/{transaction}.json")))?["status"],
                "committed"
            );
        }
    }
    Ok(())
}

/// A full artifact is retained without changing the selected generation.
#[test]
fn full_artifact_is_retained_inactive() -> Result<()> {
    let store = Store::new()?;
    let receipt = common::json(&mut store.receiver(), Some(store.artifact()?))?;
    assert_eq!(receipt["retained"], true);
    assert_eq!(receipt["activated"], false);
    assert_eq!(receipt["objects_admitted"], 1);
    assert!(!store.pool.join("domains/usr-home/current").exists());
    Ok(())
}

/// Incremental reuse transfers no repeated payload and propagates receiver rejection.
#[test]
fn incremental_channel_reuses_bytes_and_propagates_rejection() -> Result<()> {
    let store = Store::new()?;
    let first = common::json(&mut store.sender()?, None)?;
    let second = common::json(store.sender()?.arg("--reuse-generation"), None)?;
    assert_eq!(second["generation_reused"], true);
    assert_eq!(second["generation"], first["generation"]);
    assert_eq!(second["objects_transferred"], 0);
    assert_eq!(second["root_payload_bytes_transferred"], 0);

    // A fresh command avoids duplicate CLI flags while changing the declared domain.
    let command = store.sender()?;
    let args: Vec<_> = command
        .get_args()
        .map(|arg| {
            if arg == "usr-home" {
                "other-vm".into()
            } else {
                arg.to_owned()
            }
        })
        .collect();
    let mut rejected = Command::new(&store.cli);
    store
        .environment(&mut rejected)
        .args(args)
        .env("AIRLOCK_TEST_RECEIVER", &store.service)
        .env("AIRLOCK_TEST_POOL", &store.pool);
    let output = common::run(&mut rejected, None)?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("pool receiver rejected admission"));
    Ok(())
}

/// Provenance changes require a new review generation even when bytes can be reused.
#[test]
fn changed_provenance_gets_a_new_generation() -> Result<()> {
    let store = Store::new()?;
    let first = common::json(&mut store.sender()?, None)?;
    let command = store.sender()?;
    let mut args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
    for (flag, value) in [
        ("--source-revision", "c".repeat(40)),
        ("--lock-hash", "d".repeat(64)),
    ] {
        let index = args.iter().position(|arg| arg == flag).unwrap();
        args[index + 1] = value.into();
    }
    let mut command = Command::new(&store.cli);
    store
        .environment(&mut command)
        .args(args)
        .arg("--reuse-generation")
        .env("AIRLOCK_TEST_RECEIVER", &store.service)
        .env("AIRLOCK_TEST_POOL", &store.pool);
    let second = common::json(&mut command, None)?;
    assert!(second["generation"].as_u64() > first["generation"].as_u64());
    assert_eq!(second["source_revision"], "c".repeat(40));
    assert_eq!(second["lock_hash"], "d".repeat(64));
    assert_eq!(second["objects_transferred"], 0);
    Ok(())
}

/// A sender accepts only a receipt tied to its exact review identity.
#[test]
fn unrelated_receipts_are_rejected() -> Result<()> {
    for (field, value) in [
        ("schema", json!("unrelated")),
        ("generation", json!(0)),
        ("source_revision", json!("c".repeat(40))),
        ("lock_hash", json!("d".repeat(64))),
    ] {
        let store = Store::new()?;
        let mut receipt = json!({
            "schema": "airlock.closure-store.admission.v1",
            "generation": 1,
            "status": "passed",
            "domain": "usr-home",
            "system": store.system,
            "activated": false,
            "retained": true,
            "root_source": store.root,
            "kernel_name": "",
            "source_revision": "a".repeat(40),
            "lock_hash": "b".repeat(64),
            "generation_reused": true
        });
        receipt[field] = value;
        let peer = common::peer_link(store.temporary.path(), "receipt-peer")?;
        let output = common::run(
            store
                .artifact_command("send")
                .arg("--qrexec-client")
                .arg(peer)
                .arg("--reuse-generation")
                .env("AIRLOCK_TEST_RECEIPT", receipt.to_string()),
            None,
        )?;
        assert!(!output.status.success(), "{field}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("invalid pool admission receipt"),
            "{field}"
        );
    }
    Ok(())
}

/// A missing blob map cannot publish a misleading running pin.
#[test]
fn missing_map_cannot_create_an_empty_pin() -> Result<()> {
    let store = Store::new()?;
    let receipt = store.admit(true)?;
    fs::remove_file(receipt["canonical_blob_map"].as_str().unwrap())?;
    let output = common::run(
        &mut store.command(&[
            "pool",
            "mark-running",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "usr-home",
            "--generation",
            "1",
            "--volume",
            "store-data",
        ]),
        None,
    )?;
    assert!(!output.status.success());
    assert_eq!(fs::read_dir(store.pool.join("running"))?.count(), 0);
    Ok(())
}

/// Committed transaction IDs and ownership must match their generation manifest.
#[test]
fn transaction_must_belong_to_its_generation() -> Result<()> {
    for (field, value) in [
        ("domain", json!("other")),
        ("generation", json!(99)),
        ("id", json!("other")),
    ] {
        let store = Store::new()?;
        let receipt = store.admit(true)?;
        let manifest = read(receipt["manifest"].as_str().unwrap())?;
        let transaction = manifest["transaction"].as_str().unwrap();
        let path = store.pool.join(format!("transactions/{transaction}.json"));
        let mut tx = read(&path)?;
        tx[field] = value;
        fs::write(path, serde_json::to_vec(&tx)?)?;

        let output = common::run(
            &mut store.command(&[
                "pool",
                "load-generation",
                "--pool-root",
                store.pool_text(),
                "--domain",
                "usr-home",
                "--generation",
                "1",
            ]),
            None,
        )?;
        assert!(!output.status.success(), "{field}");
    }
    Ok(())
}

/// Garbage collection and recovery cannot reuse a discarded generation number.
#[test]
fn discarded_generation_numbers_are_never_reissued() -> Result<()> {
    for cleanup in ["gc", "recover"] {
        let store = Store::new()?;
        store.admit(true)?;
        if cleanup == "gc" {
            assert_eq!(store.admit(false)?["generation"], 2);
        } else {
            assert!(
                !common::run(
                    store
                        .admission()
                        .env("AIRLOCK_CLOSURE_STORE_FAILPOINT", "admit-after-tx-create"),
                    None
                )?
                .status
                .success()
            );
        }
        store.run(
            &[
                "pool",
                cleanup,
                "--pool-root",
                store.pool_text(),
                "--keep-generations",
                "0",
            ]
            .into_iter()
            .take(if cleanup == "gc" { 6 } else { 4 })
            .collect::<Vec<_>>(),
        )?;
        assert!(!store.pool.join("domains/usr-home/generations/2").exists());
        assert!(store.admit(true)?["generation"].as_u64().unwrap() > 2);
    }
    Ok(())
}

/// Maintenance refuses redirected directories and leaves unrelated bytes intact.
#[test]
fn directory_symlinks_cannot_redirect_maintenance() -> Result<()> {
    for operation in ["gc", "recover"] {
        let store = Store::new()?;
        store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
        let outside = store.temporary.path().join("outside");
        let generation = outside.join("generations/1");
        fs::create_dir_all(&generation)?;
        fs::write(generation.join("sentinel"), b"preserve outside bytes")?;
        std::os::unix::fs::symlink(&outside, store.pool.join("domains/redirect"))?;

        assert!(
            !common::run(
                &mut store.command(&["pool", operation, "--pool-root", store.pool_text()]),
                None
            )?
            .status
            .success()
        );
        assert_eq!(
            fs::read(generation.join("sentinel"))?,
            b"preserve outside bytes"
        );
    }
    Ok(())
}

/// Initialization validates existing paths before creating pool metadata or directories.
#[test]
fn initialization_rejects_symlinks_before_creating_directories() -> Result<()> {
    let store = Store::new()?;
    let outside = store.temporary.path().join("outside");
    fs::create_dir(&outside)?;
    fs::create_dir(&store.pool)?;
    std::os::unix::fs::symlink(&outside, store.pool.join("objects"))?;
    assert!(
        !common::run(
            &mut store.command(&["pool", "status", "--pool-root", store.pool_text()]),
            None
        )?
        .status
        .success()
    );
    assert_eq!(fs::read_dir(outside)?.count(), 0);
    assert!(!store.pool.join("MANIFEST.json").exists());
    Ok(())
}

/// Recovery preserves a damaged selected generation for explicit repair.
#[test]
fn damaged_selected_generation_is_kept() -> Result<()> {
    let store = Store::new()?;
    let receipt = store.admit(true)?;
    let path = Path::new(receipt["manifest"].as_str().unwrap());
    let mut manifest = read(path)?;
    manifest["schema"] = json!("damaged");
    fs::write(path, serde_json::to_vec(&manifest)?)?;
    assert!(
        !common::run(
            &mut store.command(&["pool", "recover", "--pool-root", store.pool_text()]),
            None
        )?
        .status
        .success()
    );
    assert_eq!(read(path)?["schema"], "damaged");
    assert!(store.pool.join("domains/usr-home/current").is_symlink());
    Ok(())
}

/// Archive names, including absolute paths and traversal, cannot escape receiver staging.
#[test]
fn unexpected_archive_members_cannot_escape_staging() -> Result<()> {
    for name in [
        "../escape",
        "/absolute",
        "objects/../escape.tar",
        "objects/a.tar",
    ] {
        let store = Store::new()?;
        // Raw header names are intentional: safe archive-builder APIs reject these before the receiver sees them.
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(4);
        header.set_mode(0o644);
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&header, &b"evil"[..])?;
        let payload = builder.into_inner()?;
        assert!(
            !common::run(&mut store.receiver(), Some(payload))?
                .status
                .success(),
            "{name}"
        );
        assert!(!store.temporary.path().join("escape").exists());
        assert_eq!(fs::read_dir(store.pool.join("tmp"))?.count(), 0);
    }
    Ok(())
}

/// Split gzip signatures still require a valid trailer before an artifact is committed.
#[test]
fn gzip_trailer_is_checked_after_split_signature() -> Result<()> {
    for corrupt in [false, true] {
        let store = Store::new()?;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&store.artifact()?)?;
        let mut payload = encoder.finish()?;
        if corrupt {
            let index = payload.len() - 8;
            payload[index] ^= 1;
        }
        // Output files and a polling deadline avoid a pipe writer/readback deadlock.
        let mut stdout = tempfile::tempfile()?;
        let stderr = tempfile::tempfile()?;
        let mut child = store
            .receiver()
            .stdin(Stdio::piped())
            .stdout(stdout.try_clone()?)
            .stderr(stderr.try_clone()?)
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        input.write_all(&payload[..1])?;
        input.flush()?;
        std::thread::sleep(Duration::from_millis(50));
        input.write_all(&payload[1..])?;
        drop(input);
        let waited = common::until(|| Ok(child.try_wait()?.is_some()));
        if waited.is_err() {
            let _ = child.kill();
        }
        let status = child.wait()?;
        waited?;
        if corrupt {
            assert!(!status.success());
            assert!(manifests(&store)?.is_empty());
        } else {
            assert!(status.success());
            stdout.rewind()?;
            assert_eq!(
                serde_json::from_reader::<_, Value>(&stdout)?["retained"],
                true
            );
        }
    }
    Ok(())
}

/// Rehashed fragments still cannot conceal nonzero bytes after the inner tar terminator.
#[test]
fn fragment_cannot_hide_bytes_after_tar_terminator() -> Result<()> {
    let store = Store::new()?;
    let payload = store.artifact()?;
    let mut source = tar::Archive::new(&payload[..]);
    let mut members = Vec::new();
    for entry in source.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        let header = entry.header().clone();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if name.starts_with("objects/") {
            bytes.extend_from_slice(&[0; 1024]);
            let mut hidden = [0; 512];
            hidden[..6].copy_from_slice(b"hidden");
            bytes.extend_from_slice(&hidden);
        }
        members.push((name, header, bytes));
    }
    let mut manifest: Value =
        serde_json::from_slice(&members.iter().find(|m| m.0 == "manifest.json").unwrap().2)?;
    for object in manifest["objects"].as_array_mut().unwrap() {
        let fragment = &members
            .iter()
            .find(|m| Some(m.0.as_str()) == object["fragment"].as_str())
            .unwrap()
            .2;
        object["fragment_size"] = json!(fragment.len());
        object["fragment_sha256"] = json!(format!("{hash:x}", hash = Sha256::digest(fragment)));
    }

    let mut target = tar::Builder::new(Vec::new());
    for (name, mut header, bytes) in members {
        let bytes = if name == "manifest.json" {
            serde_json::to_vec(&manifest)?
        } else {
            bytes
        };
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        target.append(&header, &bytes[..])?;
    }
    assert!(
        !common::run(&mut store.receiver(), Some(target.into_inner()?))?
            .status
            .success()
    );
    assert!(manifests(&store)?.is_empty());
    Ok(())
}

/// Damaged current pointers stop recovery without deleting retained manifests.
#[test]
fn bad_current_pointer_stops_recovery() -> Result<()> {
    for target in [
        None,
        Some("../elsewhere/1"),
        Some("/elsewhere/generations/1"),
        Some("generations/0"),
    ] {
        let store = Store::new()?;
        let receipt = store.admit(true)?;
        let pointer = store.pool.join("domains/usr-home/current");
        fs::remove_file(&pointer)?;
        if let Some(target) = target {
            std::os::unix::fs::symlink(target, pointer)?;
        } else {
            fs::write(pointer, b"damaged pointer")?;
        }
        assert!(
            !common::run(
                &mut store.command(&["pool", "recover", "--pool-root", store.pool_text()]),
                None
            )?
            .status
            .success()
        );
        assert!(Path::new(receipt["manifest"].as_str().unwrap()).exists());
    }
    Ok(())
}

/// A bind-mounted temporary tree requires an explicitly prepared user mount namespace.
#[test]
#[ignore = "requires unprivileged user/mount namespaces and mount tools"]
fn cleanup_refuses_bind_mounts() -> Result<()> {
    let store = Store::new()?;
    store.run(&["pool", "status", "--pool-root", store.pool_text()])?;
    let source = store.temporary.path().join("mounted-source");
    let target = store.pool.join("tmp/interrupted-mount");
    fs::create_dir(&source)?;
    fs::create_dir(&target)?;
    fs::write(source.join("sentinel"), b"preserve mounted bytes")?;
    common::run(
        Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
            ])
            .arg(common::peer())
            .arg("recover-mounted")
            .arg(&source)
            .arg(&target)
            .arg(&store.service)
            .arg(&store.pool),
        None,
    )?
    .checked()?;
    assert_eq!(
        fs::read(source.join("sentinel"))?,
        b"preserve mounted bytes"
    );
    Ok(())
}
