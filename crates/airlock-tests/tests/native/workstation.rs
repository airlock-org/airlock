//! Exercise committed source selection and reviewed deployment through the prepared workstation command.

use crate::common;
use airlock_tests::{
    process::{binary, run},
    repository::{Repository, git},
};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::{fs, process::Command};

/// Read a successful CLI report without obscuring tool diagnostics.
fn report(command: &mut Command) -> Result<Value> {
    Ok(serde_json::from_slice(&run(command, None)?.checked()?)?)
}

/// Plan and apply import committed source; every normal operation then uses the reviewed machine command namespace.
#[test]
fn reviewed_deployment_uses_machine_namespace() -> Result<()> {
    let fixture = Repository::new()?;
    for action in ["plan", "apply", "status", "verify", "resume", "rollback"] {
        run(
            &mut fixture.command(&["deploy", action, "--target", "user@fixture"]),
            None,
        )?
        .checked()?;
        let calls = fixture.calls("ssh")?;
        assert_eq!(
            calls.last().unwrap(),
            &json!([
                "--",
                "user@fixture",
                format!("'airlock' 'machine' '{action}'")
            ])
        );
    }
    let mut archive = tar::Archive::new(GzDecoder::new(fs::File::open(
        fixture.path("received.tar.gz"),
    )?));
    let mut names = archive
        .entries()?
        .map(|entry| Ok(entry?.path()?.into_owned()))
        .collect::<Result<Vec<_>>>()?;
    names.sort();
    assert_eq!(
        names,
        ["manifest.json", "repo.bundle"].map(std::path::PathBuf::from)
    );
    Ok(())
}

/// Invalid repair requests and dirty builds are rejected before network or build tools run.
#[test]
fn invalid_deployment_requests_fail_before_side_effects() -> Result<()> {
    let fixture = Repository::new()?;
    for (args, expected) in [
        (
            vec!["resume", "--update-coordinator"],
            "requires guest activation",
        ),
        (vec!["apply", "--host-repair"], "requires development-shell"),
        (
            vec!["status", "--host-repair", "--development-shell"],
            "requires development-shell",
        ),
        (vec!["guest-plan"], "--receipt is required"),
    ] {
        let mut arguments = vec!["deploy"];
        arguments.extend(args);
        arguments.extend(["--target", "fixture"]);
        let output = run(&mut fixture.command(&arguments), None)?;
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{error}");
    }
    fs::write(fixture.root.join("uncommitted"), "work")?;
    let output = run(&mut fixture.command(&["deploy", "build"]), None)?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("commit repository changes"));
    assert!(fixture.calls("ssh")?.is_empty());
    assert!(fixture.calls("nix")?.is_empty());
    Ok(())
}

/// Development recovery updates only the coordinator; machine builds select the explicitly supplied checkout.
#[test]
fn development_recovery_and_build_select_exact_source() -> Result<()> {
    let fixture = Repository::new()?;
    let release = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-built";
    fixture.configure(json!({"shell": true, "release": release, "inventory": release}))?;
    for action in ["resume", "rollback"] {
        run(
            &mut fixture.command(&[
                "deploy",
                action,
                "--target",
                "fixture",
                "--development-shell",
                "--update-coordinator",
            ]),
            None,
        )?
        .checked()?;
        let builds = fixture.calls("nix")?;
        assert!(
            builds
                .iter()
                .any(|args| args.as_array().unwrap().iter().any(|arg| arg
                    .as_str()
                    .is_some_and(|word| word.ends_with("#airlock-dom0-service"))))
        );
        assert!(
            !builds
                .iter()
                .any(|args| args.to_string().contains("machine-release"))
        );
        let units = fixture.calls("systemd-run")?;
        let command = units.last().unwrap().as_array().unwrap();
        assert_eq!(
            &command[command.len() - 2..],
            &[
                json!(format!("{release}/bin/airlock-dom0-service")),
                json!(action)
            ]
        );
    }
    assert_eq!(
        report(&mut fixture.command(&["deploy", "build"]))?["release"],
        release
    );
    let calls = fixture.calls("nix")?;
    let source = calls
        .iter()
        .flat_map(|args| args.as_array().unwrap())
        .filter_map(Value::as_str)
        .find(|arg| arg.ends_with("/airlock-machine#airlock-machine-release-usb-lab"))
        .context("machine flake")?;
    let prepared = std::path::Path::new(
        source
            .trim_start_matches("path:")
            .split('#')
            .next()
            .unwrap(),
    );
    let identity: Value = serde_json::from_slice(&fs::read(prepared.join("airlock-source.json"))?)?;
    assert_eq!(
        identity["revision"],
        git(
            &fixture.root.join("airlock-machine"),
            &["rev-parse", "HEAD"]
        )?
    );
    Ok(())
}

/// Source wrapping protects an active prepared artifact from pruning and preserves the caller's checkout and refs.
#[test]
fn local_nix_keeps_prepared_source_leased_until_exit() -> Result<()> {
    let fixture = Repository::new()?;
    let revision = fixture.advance_machine()?;
    let machine = fixture.root.join("airlock-machine");
    let refs = git(&machine, &["show-ref"])?;
    fixture.configure(json!({"prune": true, "nix_exit": 7}))?;
    let mut command = fixture.command(&["nix", "--", "build", ".#image"]);
    command
        .current_dir(&fixture.root)
        .env_remove("AIRLOCK_PREPARED_CACHE");
    command.env(
        "AIRLOCK_TEST_DEV_EXECUTABLE",
        binary("AIRLOCK_DEV_BIN", "airlock-dev"),
    );
    assert_eq!(run(&mut command, None)?.status.code(), Some(7));
    let calls = fixture.calls("nix")?;
    let source = calls[0][1].as_str().context("prepared reference")?;
    assert!(source.starts_with("path:") && source.ends_with("#image"));
    let prepared = std::path::Path::new(
        source
            .trim_start_matches("path:")
            .split('#')
            .next()
            .unwrap(),
    )
    .canonicalize()?;
    assert_eq!(
        prepared.parent(),
        Some(fixture.root.join("target/nix").as_path())
    );
    assert!(!prepared.join(".git").exists());
    assert!(!prepared.join("target").exists());
    assert!(prepared.join("flake.nix").is_file());
    let pruning: Value = serde_json::from_slice(&fs::read(fixture.path("pruning.json"))?)?;
    assert_eq!(pruning["candidates"], json!([]));
    assert_eq!(pruning["protected"], json!([&prepared]));
    assert_eq!(
        fs::read_to_string(prepared.join("airlock-machine/README"))?,
        "unpublished feature\n"
    );
    let metadata: Value = serde_json::from_slice(&fs::read(
        prepared.join("airlock-machine/airlock-source.json"),
    )?)?;
    assert_eq!(metadata["revision"], revision);
    assert_eq!(git(&machine, &["show-ref"])?, refs);
    assert_eq!(git(&fixture.root, &["status", "--porcelain"])?, "");

    let mut command = fixture.command(&["source", "prune", "--keep", "0", "--apply"]);
    command.env_remove("AIRLOCK_PREPARED_CACHE");
    let removed = report(&mut command)?;
    assert_eq!(removed["candidates"][0]["path"], json!(&prepared));
    assert!(!prepared.exists());
    Ok(())
}

/// Missing, unpinned, staged or modified machine source cannot reach Nix.
#[test]
fn invalid_machine_source_stops_before_nix() -> Result<()> {
    for problem in ["missing", "mismatch", "staged", "dirty"] {
        let fixture = Repository::new()?;
        let machine = fixture.root.join("airlock-machine");
        let expected = match problem {
            "missing" => {
                git(
                    &fixture.root,
                    &["submodule", "deinit", "--force", "airlock-machine"],
                )?;
                "not initialized"
            }
            "mismatch" => {
                fixture.advance_machine()?;
                git(&fixture.root, &["reset", "--quiet", "HEAD~"])?;
                "does not match"
            }
            "staged" => {
                fixture.advance_machine()?;
                git(&fixture.root, &["reset", "--soft", "HEAD~"])?;
                let pin = git(&fixture.root, &["ls-tree", "HEAD", "--", "airlock-machine"])?;
                git(
                    &machine,
                    &[
                        "switch",
                        "--quiet",
                        "--detach",
                        pin.split_whitespace().nth(2).unwrap(),
                    ],
                )?;
                "Commit the staged"
            }
            _ => {
                fs::write(machine.join("README"), "dirty\n")?;
                "Commit or stash"
            }
        };
        let reference = format!("{root}#image", root = fixture.root.display());
        let output = run(
            &mut fixture.command(&["nix", "--", "eval", &reference]),
            None,
        )?;
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{problem}: {error}");
        assert!(fixture.calls("nix")?.is_empty());
    }
    Ok(())
}

/// Publication checks query the actual upstream main rather than accepting cached refs or a feature branch.
#[test]
fn publication_requires_the_actual_upstream_main() -> Result<()> {
    let fixture = Repository::new()?;
    run(&mut fixture.command(&["check-publish"]), None)?.checked()?;
    let revision = fixture.advance_machine()?;
    let machine = fixture.root.join("airlock-machine");
    git(
        &machine,
        &["update-ref", "refs/remotes/origin/main", &revision],
    )?;
    git(
        &machine,
        &[
            "push",
            "--quiet",
            fixture.upstream.to_str().unwrap(),
            "HEAD:refs/heads/feature",
        ],
    )?;
    let output = run(&mut fixture.command(&["check-publish"]), None)?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Publish the machine commit on main"));
    git(
        &machine,
        &[
            "push",
            "--quiet",
            fixture.upstream.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    )?;
    run(&mut fixture.command(&["check-publish"]), None)?.checked()?;
    Ok(())
}

/// Both repository wrappers select the same external store, and native Nix refuses to spawn without explicit selection.
#[test]
fn workstation_nix_requires_guarded_store_selection() -> Result<()> {
    let fixture = Repository::new()?;
    let data = fixture.path("external data");
    for script in [
        common::root().join("crates/airlock-dev/scripts/lib/airlock-env.sh"),
        common::root().join("airlock-machine/scripts/dev/lib/airlock-env.sh"),
    ] {
        let mut command = Command::new("bash");
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("AIRLOCK_")
                || ["NIX_CONFIG", "TMPDIR", "XDG_CACHE_HOME"]
                    .iter()
                    .any(|name| key == *name)
            {
                command.env_remove(key);
            }
        }
        command
            .env(
                "AIRLOCK_DEV_BIN",
                common::binary("AIRLOCK_DEV_BIN", "airlock-dev"),
            )
            .env("AIRLOCK_DATA_ROOT", &data);
        command
            .args([
                "-c",
                "source \"$1\" && printf '%s\\n%s\\n' \"$AIRLOCK_NIX_ROOT\" \"$NIX_CONFIG\"",
                "bash",
            ])
            .arg(script);
        let text = String::from_utf8(run(&mut command, None)?.checked()?)?;
        assert_eq!(
            text,
            format!(
                "{store}\nstore = {store}\n",
                store = data.join("nix-root").display()
            )
        );
    }
    let mut command = fixture.command(&["nix", "--", "--version"]);
    command
        .env_remove("AIRLOCK_USE_HOST_NIX_STORE")
        .env_remove("AIRLOCK_NIX_CONFIGURED")
        .env_remove("NIX_CONFIG");
    let output = run(&mut command, None)?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("guarded Airlock store"));
    assert!(fixture.calls("nix")?.is_empty());
    Ok(())
}

/// Automatic retention respects active leases, named pins, grace periods and a busy cache, without breaking preparation.
#[test]
fn automatic_source_retention_preserves_pins_and_active_leases() -> Result<()> {
    use rustix::fs::{FlockOperation, flock};
    use std::{
        fs::File,
        time::{Duration, SystemTime},
    };

    let fixture = Repository::new()?;
    let prepare = |automatic: bool| -> Result<std::path::PathBuf> {
        let mut command = fixture.command(&["source", "prepare"]);
        if automatic {
            command.env_remove("AIRLOCK_PREPARED_AUTO_PRUNE");
        } else {
            command.env("AIRLOCK_PREPARED_AUTO_PRUNE", "0");
        }
        Ok(std::path::PathBuf::from(
            report(&mut command)?["source"]
                .as_str()
                .context("prepared source")?,
        ))
    };
    let old = SystemTime::now() - Duration::from_secs(48 * 60 * 60);
    let mut trees = Vec::new();
    for index in 0..10 {
        fs::write(
            fixture.root.join("README"),
            format!("configuration {index}\n"),
        )?;
        let tree = prepare(false)?;
        let lock = tree
            .parent()
            .unwrap()
            .join(".locks")
            .join(tree.file_name().unwrap());
        File::options()
            .write(true)
            .open(lock)?
            .set_modified(old + Duration::from_secs(index))?;
        trees.push(tree);
    }
    fs::write(fixture.root.join("README"), "current configuration\n")?;
    let current = prepare(false)?;
    let cache = current.parent().unwrap();
    run(
        &mut fixture.command(&["source", "pin", trees[0].to_str().unwrap(), "evidence"]),
        None,
    )?
    .checked()?;
    for tree in &trees[2..6] {
        File::options()
            .write(true)
            .open(cache.join(".locks").join(tree.file_name().unwrap()))?
            .set_modified(SystemTime::now() - Duration::from_secs(3600))?;
    }
    let active = File::open(cache.join(".locks").join(trees[1].file_name().unwrap()))?;
    flock(&active, FlockOperation::LockShared)?;
    let busy = File::open(cache.join(".cache.lock"))?;
    flock(&busy, FlockOperation::LockShared)?;
    assert_eq!(prepare(true)?, current);
    assert!(trees.iter().all(|tree| tree.exists()));
    drop(busy);

    assert_eq!(prepare(true)?, current);
    assert!(trees[..6].iter().all(|tree| tree.exists()));
    assert!(trees[6..].iter().all(|tree| !tree.exists()));
    for (index, tree) in trees[2..6].iter().enumerate() {
        File::options()
            .write(true)
            .open(cache.join(".locks").join(tree.file_name().unwrap()))?
            .set_modified(old + Duration::from_secs(index as u64))?;
    }
    assert_eq!(prepare(true)?, current);
    assert!(!trees[2].exists());
    assert!(trees[3..6].iter().all(|tree| tree.exists()));
    assert!(trees[0].exists() && trees[1].exists());
    drop(active);
    File::options()
        .write(true)
        .open(cache.join(".locks").join(trees[1].file_name().unwrap()))?
        .set_modified(old)?;
    assert_eq!(prepare(true)?, current);
    assert!(!trees[1].exists());
    assert!(trees[0].exists());

    fs::write(cache.join(".pins/broken"), "invalid pin")?;
    let result = run(&mut fixture.command(&["source", "prepare"]), None)?;
    assert!(String::from_utf8_lossy(&result.stderr).contains("cache cleanup skipped"));
    assert_eq!(
        serde_json::from_slice::<Value>(&result.checked()?)?["source"],
        json!(current)
    );
    Ok(())
}

/// An unpublished local application pin is fetched through a temporary Git view without rewriting the upstream or local refs.
#[test]
fn local_application_pin_is_available_to_nix_without_publication() -> Result<()> {
    let fixture = Repository::new()?;
    let upstream = fixture.path("apps-upstream.git");
    git(
        &fixture.root,
        &[
            "clone",
            "--quiet",
            "--bare",
            fixture.root.to_str().unwrap(),
            upstream.to_str().unwrap(),
        ],
    )?;
    fs::write(
        fixture.root.join("app-change"),
        "unpublished application code",
    )?;
    git(&fixture.root, &["add", "app-change"])?;
    git(&fixture.root, &["commit", "-qm", "Application change"])?;
    let revision = git(&fixture.root, &["rev-parse", "HEAD"])?;
    let uri = format!("file://{path}", path = upstream.display());
    fs::write(
        fixture.root.join("flake.lock"),
        serde_json::to_vec(&json!({"nodes": {"airlock-apps": {"locked": {
            "type": "git", "dir": "crates", "url": uri, "rev": revision, "ref": "refs/heads/main", "submodules": false
        }}}}))?,
    )?;
    git(&fixture.root, &["commit", "-qam", "Pin application source"])?;
    let before = git(&fixture.root, &["show-ref"])?;
    fixture.configure(json!({"fetch_remote": uri}))?;
    let mut command = fixture.command(&["nix", "--", "build", ".#image"]);
    command.current_dir(&fixture.root);
    run(&mut command, None)?.checked()?;
    assert_eq!(
        fs::read_to_string(fixture.path("fetched"))?.trim(),
        revision
    );
    assert_ne!(git(&upstream, &["rev-parse", "HEAD"])?, revision);
    assert_eq!(git(&fixture.root, &["show-ref"])?, before);
    assert_eq!(git(&fixture.root, &["status", "--porcelain"])?, "");
    Ok(())
}

/// Host repair captures the recorded image baseline while retaining the candidate revision as separate repair provenance.
#[test]
fn host_repair_preserves_recorded_image_source() -> Result<()> {
    use sha2::{Digest, Sha256};

    let fixture = Repository::new()?;
    let machine = fixture.root.join("airlock-machine");
    let image = git(&fixture.root, &["rev-parse", "HEAD"])?;
    let baseline = git(&machine, &["rev-parse", "HEAD"])?;
    let original = fs::read(machine.join("flake.lock"))?;
    fs::write(machine.join("flake.lock"), "{\"candidate\":true}\n")?;
    git(&machine, &["commit", "-qam", "Candidate"])?;
    let candidate = git(&machine, &["rev-parse", "HEAD"])?;
    git(&fixture.root, &["commit", "-qam", "Pin candidate"])?;
    let release = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-built";
    let topology = json!({"domains": {}});
    let topology_hash = format!("{hash:x}", hash = Sha256::digest(b"{\"domains\": {}}"));
    fixture.configure(json!({
        "shell": true, "release": release, "inventory": release,
        "snapshot": {"host": release, "profile": "usb-lab", "devMode": true,
            "boot": {"revision": image}, "manifest": null, "topology": topology, "topologyHash": topology_hash},
        "boot": {"revision": baseline, "profile": "usb-lab", "coordinator": format!("{release}/bin/airlock-dom0-service")}
    }))?;
    run(
        &mut fixture.command(&[
            "deploy",
            "plan",
            "--target",
            "fixture",
            "--development-shell",
            "--host-repair",
        ]),
        None,
    )?
    .checked()?;
    let directory = fs::read_dir(fixture.root.join(".airlock-logs/machine-host-repair"))?
        .next()
        .context("repair evidence")??
        .path();
    let saved: Value = serde_json::from_slice(&fs::read(directory.join("snapshot.json"))?)?;
    assert_eq!(saved["revision"], baseline);
    assert_ne!(baseline, candidate);
    assert_eq!(
        saved["lockHash"],
        format!("{hash:x}", hash = Sha256::digest(original))
    );
    assert_eq!(saved["repairRevision"], candidate);
    assert!(saved["baselineFlake"].as_str().unwrap().contains("%20"));
    assert_eq!(saved["topologyHash"], topology_hash);
    Ok(())
}
