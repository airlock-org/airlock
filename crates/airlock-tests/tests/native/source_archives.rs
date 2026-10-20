//! Source archives and offline manifests bind generated inputs to an exact committed machine revision.

use airlock_tests::{
    process::run,
    repository::{Repository, git},
};
use anyhow::Result;
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::Command,
};

/// Create a committed dependency graph in the fixture machine without involving Nix evaluation.
fn lock(fixture: &Repository, value: Value) -> Result<()> {
    let machine = fixture.root.join("airlock-machine");
    fs::write(machine.join("flake.lock"), serde_json::to_vec(&value)?)?;
    git(&machine, &["add", "flake.lock"])?;
    git(&machine, &["commit", "-qm", "Lock inputs"])?;
    Ok(())
}

/// Run the selected source operation against the machine repository rather than the parent gitlink.
fn command(fixture: &Repository, operation: &str, output: &Path) -> Command {
    let mut command = fixture.command(&["source", operation]);
    command
        .arg("--repository")
        .arg(fixture.root.join("airlock-machine"));
    command.arg("--output").arg(output);
    command
}

/// Archived flake directories and nested local inputs remain under their captured source; traversal cannot publish a manifest.
#[test]
fn offline_manifests_bind_revision_lock_and_nested_source_locations() -> Result<()> {
    for directory in ["", "crates", "../apps", "/apps", "apps/../other"] {
        let fixture = Repository::new()?;
        lock(
            &fixture,
            json!({"root": "root", "nodes": {
                "root": {"inputs": {"airlock-apps": "airlock-apps"}},
                "airlock-apps": {"locked": {"type": "git", "dir": directory}, "inputs": {"vendor": "vendor"}},
                "vendor": {"locked": {"path": "./vendor"}, "inputs": {}}
            }}),
        )?;
        let source = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-source";
        let archive = json!({"path": "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-machine", "inputs": {
            "airlock-apps": {"path": source, "inputs": {"vendor": {"inputs": {}}}}
        }});
        let output = fixture.path("inputs.json");
        let result = run(
            &mut command(&fixture, "offline-inputs", &output),
            Some(serde_json::to_vec(&archive)?),
        )?;
        if !["", "crates"].contains(&directory) {
            assert!(!result.status.success());
            assert!(
                String::from_utf8_lossy(&result.stderr)
                    .contains("invalid archived flake directory")
            );
            assert!(!output.exists());
            continue;
        }
        result.checked()?;
        let manifest: Value = serde_json::from_slice(&fs::read(output)?)?;
        let expected = if directory.is_empty() {
            source.to_owned()
        } else {
            format!("{source}/{directory}")
        };
        assert_eq!(manifest["schema"], "airlock.sys-agent-offline-inputs.v1");
        let machine = fixture.root.join("airlock-machine");
        assert_eq!(manifest["revision"], git(&machine, &["rev-parse", "HEAD"])?);
        assert_eq!(
            manifest["lock_hash"],
            format!(
                "{hash:x}",
                hash = Sha256::digest(fs::read(machine.join("flake.lock"))?)
            )
        );
        assert_eq!(
            manifest["inputs"],
            json!({"airlock-apps": expected, "airlock-apps/vendor": format!("{expected}/vendor")})
        );
    }
    for local in [false, true] {
        let fixture = Repository::new()?;
        let mut dependency = json!({"inputs": {"shelly": "shelly"}});
        if local {
            dependency["locked"] = json!({"path": "./vendor/nixos-qubes"});
        }
        lock(
            &fixture,
            json!({"root": "root", "nodes": {
                "root": {"inputs": {"nixos-qubes": "nixos-qubes"}}, "nixos-qubes": dependency, "shelly": {}
            }}),
        )?;
        let machine = "/nix/store/cccccccccccccccccccccccccccccccc-machine";
        let source = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-source";
        let shelly = "/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-source";
        let mut qubes = json!({"inputs": {"shelly": {"path": shelly, "inputs": {}}}});
        if !local {
            qubes["path"] = json!(source);
        }
        let output = fixture.path("inputs.json");
        let archive = json!({"path": machine, "inputs": {"nixos-qubes": qubes}});
        run(
            &mut command(&fixture, "offline-inputs", &output),
            Some(serde_json::to_vec(&archive)?),
        )?
        .checked()?;
        let manifest: Value = serde_json::from_slice(&fs::read(output)?)?;
        assert_eq!(
            manifest["inputs"],
            json!({
                "nixos-qubes": if local {format!("{machine}/vendor/nixos-qubes")} else {source.to_owned()},
                "nixos-qubes/shelly": shelly
            })
        );
    }
    Ok(())
}

/// Bundles restore exact committed source into an empty repository; changed source is rejected before creating another archive.
#[test]
fn machine_source_bundle_round_trips_committed_content() -> Result<()> {
    let fixture = Repository::new()?;
    let machine = fixture.root.join("airlock-machine");
    let revision = git(&machine, &["rev-parse", "HEAD"])?;
    let output = fixture.path("source.tar.gz");
    let report: Value =
        serde_json::from_slice(&run(&mut command(&fixture, "create", &output), None)?.checked()?)?;
    assert_eq!(report["revision"], revision);
    let mut archive = tar::Archive::new(GzDecoder::new(File::open(output)?));
    let mut names = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if name == Path::new("manifest.json") {
            let manifest: Value = serde_json::from_slice(&bytes)?;
            assert_eq!(
                manifest,
                json!({"schema": "airlock.sys-agent-machine-source.v1", "revision": revision,
                "lock_hash": report["lock_hash"], "submodules": []})
            );
        } else if name == Path::new("repo.bundle") {
            fs::write(fixture.path("repo.bundle"), bytes)?;
        }
        names.push(name);
    }
    names.sort();
    assert_eq!(
        names,
        ["manifest.json", "repo.bundle"].map(std::path::PathBuf::from)
    );
    let restored = fixture.path("restored");
    fs::create_dir(&restored)?;
    git(&restored, &["init", "-q"])?;
    git(
        &restored,
        &[
            "bundle",
            "verify",
            fixture.path("repo.bundle").to_str().unwrap(),
        ],
    )?;
    git(
        &restored,
        &[
            "fetch",
            fixture.path("repo.bundle").to_str().unwrap(),
            &revision,
        ],
    )?;
    git(&restored, &["checkout", "--detach", &revision])?;
    assert_eq!(
        fs::read(restored.join("flake.lock"))?,
        fs::read(machine.join("flake.lock"))?
    );

    fs::write(machine.join("local-work"), "keep")?;
    let output = fixture.path("dirty.tar.gz");
    let result = run(&mut command(&fixture, "create", &output), None)?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("commit or stash checkout changes"));
    assert!(!output.exists());
    Ok(())
}

/// Reusable shallow seeds authenticate archive bytes, replace stale special files and retain exactly one committed revision.
#[test]
fn machine_seed_reuse_authenticates_bytes_without_extracting_untrusted_inputs() -> Result<()> {
    let fixture = Repository::new()?;
    let output = fixture.path("seed.tar.gz");
    let make = || -> Result<()> {
        run(command(&fixture, "seed", &output).arg("--reuse"), None)?.checked()?;
        Ok(())
    };
    make()?;
    let original = fs::read(&output)?;
    make()?;
    assert_eq!(fs::read(&output)?, original);
    fs::write(&output, "corrupted archive")?;
    make()?;
    let extracted = fixture.path("extracted");
    fs::create_dir(&extracted)?;
    tar::Archive::new(GzDecoder::new(File::open(&output)?)).unpack(&extracted)?;
    let revision = git(
        &fixture.root.join("airlock-machine"),
        &["rev-parse", "HEAD"],
    )?;
    let manifest: Value = serde_json::from_slice(&fs::read(extracted.join("manifest.json"))?)?;
    assert_eq!(manifest["schema"], "airlock.sys-agent-machine-seed.v1");
    assert_eq!(manifest["revision"], revision);
    assert_eq!(
        git(&extracted.join("repo"), &["rev-parse", "HEAD"])?,
        revision
    );
    assert_eq!(
        git(
            &extracted.join("repo"),
            &["rev-parse", "--is-shallow-repository"]
        )?,
        "true"
    );
    assert_eq!(
        git(&extracted.join("repo"), &["rev-list", "--count", "HEAD"])?,
        "1"
    );

    for target in [&output, &fixture.path("seed.tar.gz.identity.json")] {
        fs::remove_file(target)?;
        rustix::fs::mknodat(
            rustix::fs::CWD,
            target,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )?;
        make()?;
        assert!(output.is_file() && fixture.path("seed.tar.gz.identity.json").is_file());
    }
    Ok(())
}

/// First boot seeds a private shallow checkout once and preserves subsequent user edits.
#[test]
fn machine_seed_installation_is_private_and_idempotent() -> Result<()> {
    let fixture = Repository::new()?;
    let archive = fixture.path("seed.tar.gz");
    run(&mut command(&fixture, "seed", &archive), None)?.checked()?;
    let home = fixture.path("vm-home");
    fs::create_dir(&home)?;
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    let execute = || -> Result<()> {
        let mut command = Command::new(crate::common::binary(
            "AIRLOCK_VM_SERVICE_BIN",
            "airlock-vm-service",
        ));
        command
            .arg("worker")
            .arg("--repo")
            .arg(home.join("airlock-machine"))
            .arg("--cache")
            .arg(home.join(".cache/airlock-machine-source"))
            .arg("--state")
            .arg(home.join("state"))
            .arg("--store")
            .arg(home.join("store"))
            .arg("seed")
            .arg("--archive")
            .arg(&archive)
            .arg("--user")
            .arg(uid.to_string())
            .arg("--group")
            .arg(gid.to_string());
        run(&mut command, None)?.checked()?;
        Ok(())
    };
    execute()?;
    let seeded = home.join("airlock-machine");
    assert_eq!(
        git(&seeded, &["config", "airlock.seedRevision"])?,
        git(
            &fixture.root.join("airlock-machine"),
            &["rev-parse", "HEAD"]
        )?
    );
    assert_eq!(
        git(&seeded, &["rev-parse", "--is-shallow-repository"])?,
        "true"
    );
    assert_eq!(git(&seeded, &["status", "--porcelain"])?, "");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        home.join(".cache").metadata()?.permissions().mode() & 0o777,
        0o700
    );
    fs::write(seeded.join("local-work"), "preserve me")?;
    execute()?;
    assert_eq!(
        fs::read_to_string(seeded.join("local-work"))?,
        "preserve me"
    );
    Ok(())
}

/// The shipped receiver binds a real bundle to its declared revision/lock and preserves dirty local work on retry.
#[test]
fn native_machine_source_import_binds_archive_and_preserves_local_edits() -> Result<()> {
    let fixture = Repository::new()?;
    let archive = fixture.path("source.tar.gz");
    let result = run(&mut command(&fixture, "create", &archive), None)?.checked()?;
    let record: Value = serde_json::from_slice(&result)?;
    let revision = record["revision"].as_str().unwrap();
    let hash = record["lock_hash"].as_str().unwrap();
    let repo = fixture.path("imported");
    let execute = |hash: &str| -> Result<airlock_vm_service::support::Output> {
        let mut command = Command::new(crate::common::binary(
            "AIRLOCK_VM_SERVICE_BIN",
            "airlock-vm-service",
        ));
        command
            .arg("worker")
            .arg("--repo")
            .arg(&repo)
            .arg("--cache")
            .arg(fixture.path("cache"))
            .arg("--state")
            .arg(fixture.path("state"))
            .arg("--store")
            .arg(fixture.path("store"))
            .args([
                "source",
                "import",
                "--revision",
                revision,
                "--lock-hash",
                hash,
            ]);
        run(&mut command, Some(fs::read(&archive)?))
    };
    let imported: Value = serde_json::from_slice(&execute(hash)?.checked()?)?;
    assert_eq!(imported["revision"], revision);
    assert_eq!(imported["lock_hash"], hash);
    execute(hash)?.checked()?;
    assert!(!execute(&"0".repeat(64))?.status.success());
    fs::write(repo.join("local-edit"), "preserve")?;
    assert!(!execute(hash)?.status.success());
    assert_eq!(fs::read_to_string(repo.join("local-edit"))?, "preserve");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"])?, revision);
    Ok(())
}
