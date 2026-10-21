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

/// A bounded Make invocation covers prerequisites, nested wrappers, threads and subprocesses.
#[test]
fn image_build_budget_reserves_a_physical_core_through_make_and_children() -> Result<()> {
    let fixture = tempfile::tempdir()?;
    let dev = binary("AIRLOCK_DEV_BIN", "airlock-dev");
    let peer = binary("AIRLOCK_TEST_PEER_BIN", "airlock-test-peer");
    let quote = |path: &std::path::Path| airlock_tests::process::quote(path.to_str().unwrap());
    let makefile = fixture.path().join("Makefile");
    fs::write(
        &makefile,
        format!(
            r#".PHONY: all prerequisite
all: prerequisite
	@{peer} observe-build-cpus all
prerequisite:
	@{dev} --repository-root {root} build-budget -- {peer} observe-build-cpus prerequisite
"#,
            peer = quote(&peer),
            dev = quote(&dev),
            root = quote(fixture.path()),
        ),
    )?;
    let inherited = rustix::thread::sched_getaffinity(None)?;
    let config = "store = /private-store\nbuild-dir = /scratch\n";
    let output = run(
        Command::new(&dev)
            .arg("--repository-root")
            .arg(fixture.path())
            .args(["build-budget", "--", "make", "--no-print-directory", "-f"])
            .arg(&makefile)
            .env("NIX_CONFIG", config)
            .env("CARGO_BUILD_JOBS", "999"),
        None,
    )?
    .checked()?;
    let observations = std::str::from_utf8(&output)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0]["stage"], "prerequisite");
    assert_eq!(observations[1]["stage"], "all");
    let budget: Value = serde_json::from_slice(&fs::read(
        fixture.path().join(".airlock-logs/build-cpu-budget.json"),
    )?)?;
    let cpus = budget["build_cpus"].as_array().context("build CPUs")?;
    let reserved = budget["reserved_core_cpus"]
        .as_array()
        .context("reserved CPUs")?;
    assert!(!cpus.is_empty() && !reserved.is_empty());
    for cpu in cpus {
        assert!(inherited.is_set(cpu.as_u64().unwrap() as usize));
        assert!(!reserved.contains(cpu));
    }
    for observation in observations {
        for key in ["cpus", "thread_cpus", "child_cpus"] {
            assert_eq!(observation[key], budget["build_cpus"]);
        }
        assert_eq!(observation["cargo_jobs"], cpus.len().to_string());
        let actual = observation["nix_config"].as_str().unwrap();
        assert!(actual.starts_with(config));
        assert!(actual.ends_with(&format!(
            "max-jobs = 1\ncores = {jobs}\n",
            jobs = cpus.len()
        )));
    }
    assert_eq!(rustix::thread::sched_getaffinity(None)?, inherited);

    // GNU Make executes recursive recipe lines under --dry-run. The outer wrapper
    // therefore runs, while every actual image-build prerequisite remains inert.
    let budget_path = fixture.path().join(".airlock-logs/build-cpu-budget.json");
    for goal in [
        "usb-dev-image",
        "usb-dom0-console-image",
        "usb-builder-offline-check",
    ] {
        fs::remove_file(&budget_path)?;
        run(
            Command::new("make")
                .current_dir(common::root())
                .args([
                    "--no-print-directory",
                    "--dry-run",
                    goal,
                    "AIRLOCK_IMAGE_BUILD_LIMITED=0",
                ])
                .arg(format!(
                    "AIRLOCK_DEV={dev} --repository-root {root}",
                    dev = quote(&dev),
                    root = quote(fixture.path())
                ))
                .env_remove("AIRLOCK_IMAGE_BUILD_LIMITED"),
            None,
        )?
        .checked()?;
        let budget: Value = serde_json::from_slice(&fs::read(&budget_path)?)?;
        assert_eq!(budget["build_cpus"], json!(cpus));
    }

    let output = run(
        Command::new(dev)
            .arg("--repository-root")
            .arg(fixture.path())
            .args(["build-budget", "--"])
            .arg(peer)
            .env("CARGO_BUILD_JOBS", "1")
            .env("AIRLOCK_TEST_PEER_MODE", "journalctl")
            .env("AIRLOCK_TEST_EXIT_CODE", "23"),
        None,
    )?;
    assert_eq!(output.status.code(), Some(23));
    Ok(())
}

/// The initial compiler and linker run on one already allowed CPU before the Rust wrapper exists.
#[test]
fn image_build_bootstrap_limits_compilation_before_native_policy_is_available() -> Result<()> {
    let fixture = tempfile::tempdir()?;
    let tools = fixture.path().join("tools");
    fs::create_dir(&tools)?;
    airlock_tests::process::peer_link(&tools, "cargo")?;
    let mut paths = vec![tools];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").context("PATH")?,
    ));
    let record = fixture.path().join("bootstrap.json");
    let result = run(
        Command::new(common::tool("bash")?)
            .arg(common::root().join("crates/airlock-dev/scripts/airlock-dev.sh"))
            .args(["build-budget", "--", "true"])
            .env("PATH", std::env::join_paths(paths)?)
            .env("AIRLOCK_TEST_CPU_BOOTSTRAP_RECORD", &record),
        None,
    )?;
    assert_eq!(result.status.code(), Some(37));
    let record: Value = serde_json::from_slice(&fs::read(record)?)?;
    let cpus = record["cpus"].as_array().context("bootstrap CPUs")?;
    assert_eq!(cpus.len(), 1);
    assert!(rustix::thread::sched_getaffinity(None)?.is_set(cpus[0].as_u64().unwrap() as usize));
    let args = record["args"].as_array().context("bootstrap arguments")?;
    assert!(
        args.windows(2)
            .any(|pair| pair == [json!("--jobs"), json!("1")])
    );
    Ok(())
}

/// Explicitly exercise the real local Nix sandbox using only a pre-existing private-store shell.
#[test]
#[ignore = "explicit tiny Nix build; requires AIRLOCK_NIX_ROOT and AIRLOCK_CPU_PROBE_BASH"]
fn image_build_budget_is_inherited_by_nix_builders() -> Result<()> {
    let store = std::path::PathBuf::from(std::env::var("AIRLOCK_NIX_ROOT")?);
    anyhow::ensure!(
        store.is_absolute() && store != std::path::Path::new("/"),
        "private store root required"
    );
    let bash: airlock_nix_core::store_path::StorePath =
        std::env::var("AIRLOCK_CPU_PROBE_BASH")?.parse()?;
    anyhow::ensure!(
        store
            .join(bash.as_str().trim_start_matches('/'))
            .join("bin/bash")
            .is_file(),
        "explicitly prepare the probe shell"
    );
    let fixture = tempfile::tempdir()?;
    let expression = fixture.path().join("probe.nix");
    let script = r#"
        {
          while IFS= read -r line; do
            case "$line" in Cpus_allowed_list:*) printf '%s\n' "$line";; esac
          done < /proc/self/status
          printf 'NIX_BUILD_CORES=%s\n' "$NIX_BUILD_CORES"
        } > "$out"
    "#;
    fs::write(
        &expression,
        format!(
            r#"
        builtins.derivation {{
          name = "airlock-cpu-budget-probe-{identity}";
          system = builtins.currentSystem;
          builder = (builtins.storePath "{bash}") + "/bin/bash";
          args = [ "-c" {script} ];
          preferLocalBuild = true;
          allowSubstitutes = false;
        }}
    "#,
            identity = fixture
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .trim_start_matches('.'),
            bash = bash.as_str(),
            script = serde_json::to_string(script)?,
        ),
    )?;
    let result = run(
        Command::new(binary("AIRLOCK_DEV_BIN", "airlock-dev"))
            .arg("--repository-root")
            .arg(fixture.path())
            .args(["build-budget", "--"])
            .arg(common::tool("nix")?)
            .args(["--extra-experimental-features", "nix-command", "--store"])
            .arg(&store)
            .args([
                "build",
                "--impure",
                "--no-link",
                "--print-out-paths",
                "--file",
            ])
            .arg(expression),
        None,
    )?
    .checked()?;
    let output: airlock_nix_core::store_path::StorePath =
        std::str::from_utf8(&result)?.trim().parse()?;
    let observed = fs::read_to_string(store.join(output.as_str().trim_start_matches('/')))?;
    let budget: Value = serde_json::from_slice(&fs::read(
        fixture.path().join(".airlock-logs/build-cpu-budget.json"),
    )?)?;
    let cpus = budget["build_cpus"].as_array().unwrap();
    let list = observed
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .context("builder affinity")?;
    let mut actual = Vec::new();
    for item in list.trim().split(',') {
        let (start, end) = item.split_once('-').unwrap_or((item, item));
        actual.extend(start.parse::<usize>()?..=end.parse::<usize>()?);
    }
    assert_eq!(json!(actual), budget["build_cpus"]);
    assert!(
        observed
            .lines()
            .any(|line| line == format!("NIX_BUILD_CORES={count}", count = cpus.len()))
    );
    println!(
        "{}",
        json!({"budget":budget,"builder_observation":observed,"output":output.as_str()})
    );
    Ok(())
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
