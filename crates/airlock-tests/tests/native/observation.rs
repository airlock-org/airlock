//! Development observation and guarded GC inspect only explicit source and private-store state.

use crate::common::{self, peers::Peers};
use airlock_tests::repository::git;
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::{fs::symlink, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
};

/// Cleanup checks nondefault VM workdirs too, before deleting any selected disk state.
#[test]
fn generated_cleanup_preserves_all_workdirs_when_an_alternate_vm_is_running() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().join("workspace");
    let selected = root.join("selected-vm");
    let alternate = root.join(".airlock-vm-alternate");
    for path in [&selected, &alternate] {
        fs::create_dir_all(path)?;
        fs::write(path.join("disk.qcow2"), "preserve disk")?;
    }
    let mut child = airlock_nix_core::runtime::process::OwnedChild(
        Command::new(common::peer())
            .arg0("qemu-system-fixture")
            .arg("hold")
            .arg(alternate.join("disk.qcow2"))
            .env("AIRLOCK_TEST_PEER_MODE", "airlock-test-peer")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let command = || {
        let mut command = Command::new(common::binary("AIRLOCK_DEV_BIN", "airlock-dev"));
        command
            .arg("--repository-root")
            .arg(&root)
            .args(["clean", "vm-workdirs"])
            .env("AIRLOCK_DATA_ROOT", temporary.path().join("data"))
            .env("AIRLOCK_VM_DIR", &selected)
            .env_remove("AIRLOCK_NIX_ROOT")
            .env_remove("AIRLOCK_NIX_STORE");
        command
    };
    let result = common::run(&mut command(), None)?;
    assert!(
        !result.status.success(),
        "cleanup accepted a running alternate VM"
    );
    for path in [&selected, &alternate] {
        assert_eq!(
            fs::read_to_string(path.join("disk.qcow2"))?,
            "preserve disk"
        );
    }

    child.0.kill()?;
    child.0.wait()?;
    common::run(&mut command(), None)?.checked()?;
    assert!(!selected.exists());
    assert!(!alternate.exists());
    assert!(temporary.path().join("data/nix-root").is_dir());
    Ok(())
}

/// GC derives roots from structured receipts, VM images and backing files, never arbitrary log text or the host store.
#[test]
fn guarded_gc_retains_structured_roots_and_preserves_store_selection() -> Result<()> {
    let mut peers = Peers::new()?;
    let data = peers.path("data with spaces");
    let nix_root = data.join("nix-root");
    let store = nix_root.join("nix/store");
    let vm = data.join("vm");
    let logs = data.join("logs");
    let links = data.join("result-links");
    for path in [&store, &vm, &logs, &links] {
        fs::create_dir_all(path)?;
    }
    let names = [
        "11111111111111111111111111111111-vm-image",
        "22222222222222222222222222222222-backing-image",
        "33333333333333333333333333333333-system",
        "44444444444444444444444444444444-tool",
        "55555555555555555555555555555555-retained-backing",
        "66666666666666666666666666666666-shared-backing",
    ];
    for name in names {
        fs::create_dir(store.join(name))?;
    }
    fs::write(store.join(names[5]).join("base.qcow2"), "backing bytes")?;
    fs::write(store.join(names[4]).join("base.qcow2"), "backup base")?;
    symlink(
        format!("/nix/store/{base}/base.qcow2", base = names[5]),
        store.join(names[1]).join("base.qcow2"),
    )?;
    symlink(
        store.join(names[0]),
        vm.join("airlock-vm-gui-config-dev-image-result"),
    )?;
    symlink(
        format!("/nix/store/{tool}", tool = names[3]),
        links.join("qemu-kvm-result"),
    )?;
    let overlay = vm.join("airlock-vm-gui-config-dev-image.qcow2");
    let backup = vm.join("airlock-vm-gui-config-dev-image.qcow2.20261020.bak");
    fs::write(&overlay, "")?;
    fs::write(&backup, "")?;
    fs::write(
        logs.join("config-apply.json"),
        serde_json::to_vec(&json!({"schema": "airlock.config-apply.v1",
        "system_apply_result": {"host_system_source": format!("/nix/store/{system}", system = names[2])},
        "apply_log": "noisy log mentions /nix/store/99999999999999999999999999999999-do-not-root"}))?,
    )?;
    let config = format!("store = {root}", root = nix_root.display());
    peers.rules("nix-store", json!([
        {"args": ["--gc", "--print-dead"], "env": {"NIX_CONFIG": config},
            "stdout": "/nix/store/deadbeefdeadbeefdeadbeefdeadbeef-dead\n", "stderr": "removing stale temporary roots file fixture\n"},
        {"args": ["--gc"], "env": {"NIX_CONFIG": config}, "stdout": "freed 0 bytes\n"}
    ]))?;
    let qemu = json!([
        {"args": ["info", "--output=json", overlay],
         "stdout": serde_json::to_string(&json!({"filename": overlay,
             "backing-filename": format!("/nix/store/{base}/base.qcow2", base = names[1])}))?},
        {"args": ["info", "--output=json", store.join(names[5]).join("base.qcow2")],
         "stdout": serde_json::to_string(&json!({"filename": store.join(names[5]).join("base.qcow2")}))?},
        {"args": ["info", "--output=json", backup],
         "stdout": serde_json::to_string(&json!({"filename": backup,
             "backing-filename": format!("../nix-root/nix/store/{base}/base.qcow2", base = names[4])}))?},
        {"args": ["info", "--output=json", store.join(names[4]).join("base.qcow2")],
         "stdout": serde_json::to_string(&json!({"filename": store.join(names[4]).join("base.qcow2")}))?}
    ]);
    peers.rules("qemu-img", qemu.clone())?;
    let command = |peers: &Peers, mode: &str| -> Result<Command> {
        let mut command = peers.command(common::binary("AIRLOCK_DEV_BIN", "airlock-dev"))?;
        command
            .arg("--repository-root")
            .arg(common::root().join("airlock-machine"))
            .arg("gc")
            .env(
                "AIRLOCK_DEV_BIN",
                common::binary("AIRLOCK_DEV_BIN", "airlock-dev"),
            )
            .arg(mode);
        command
            .env("AIRLOCK_GC_ALLOW_NONSTANDARD_DATA_ROOT", "1")
            .env("AIRLOCK_DATA_ROOT", &data)
            .env("AIRLOCK_NIX_ROOT", &nix_root)
            .env("AIRLOCK_NIX_STORE", &nix_root)
            .env("AIRLOCK_VM_DIR", &vm)
            .env("AIRLOCK_LOG_DIR", &logs)
            .env("AIRLOCK_RESULT_LINK_DIR", &links)
            .env("AIRLOCK_NIX_STORE_BIN", peers.path("bin/nix-store"))
            .env("AIRLOCK_QEMU_IMG_BIN", peers.path("bin/qemu-img"))
            .env_remove("AIRLOCK_NIX_CONFIGURED")
            .env_remove("AIRLOCK_USE_HOST_NIX_STORE")
            .env_remove("NIX_CONFIG");
        Ok(command)
    };
    let output =
        String::from_utf8(common::run(&mut command(&peers, "dry-run")?, None)?.checked()?)?;
    assert!(output.contains("Dead path count:  1"));
    let manifest = fs::read_to_string(nix_root.join("nix/var/nix/gcroots/airlock/manifest.tsv"))?;
    for name in names {
        assert!(manifest.contains(&format!("/nix/store/{name}")));
    }
    assert!(!manifest.contains("99999999999999999999999999999999-do-not-root"));
    assert!(
        fs::read_to_string(logs.join("airlock-store-gc-dead-paths.stderr.log"))?
            .contains("removing stale temporary roots file fixture")
    );
    let read = || -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(
            logs.join("airlock-store-gc.json"),
        )?)?)
    };
    let dry = read()?;
    assert_eq!(dry["schema"], "airlock.store-gc.v1");
    assert_eq!(dry["nix_root"], json!(nix_root));
    assert_eq!(dry["root_count"], 6);
    assert_eq!(dry["dead_count"], 1);
    assert_eq!(dry["gc_ran"], false);
    for mode in ["run", "auto"] {
        let mut command = command(&peers, mode)?;
        command.env("AIRLOCK_GC_AUTO_THRESHOLD_GIB", "0");
        common::run(&mut command, None)?.checked()?;
        let report = read()?;
        assert_eq!(report["mode"], mode);
        assert_eq!(report["gc_ran"], true);
    }
    let calls = peers.calls()?;
    let args = |expected: Value| {
        calls
            .iter()
            .filter(|call| call["command"] == "nix-store" && call["args"] == expected)
            .count()
    };
    assert_eq!(args(json!(["--gc", "--print-dead"])), 2);
    assert_eq!(args(json!(["--gc"])), 2);

    // TMPDIR may itself be inside the allowed Airlock data root during image builds.
    let nonstandard = tempfile::tempdir_in("/tmp")?;
    let mut refused = command(&peers, "dry-run")?;
    refused
        .env("AIRLOCK_DATA_ROOT", nonstandard.path())
        .env("AIRLOCK_GC_ALLOW_NONSTANDARD_DATA_ROOT", "0");
    let output = common::run(&mut refused, None)?;
    assert!(!output.status.success());
    let errors = format!(
        "{stdout}{stderr}",
        stdout = String::from_utf8_lossy(&output.stdout),
        stderr = String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        errors.contains("refusing nonstandard data root"),
        "{errors}"
    );
    assert_eq!(peers.calls()?.len(), calls.len());

    // Failed chain or receipt inspection preserves the last known roots and never proceeds to GC.
    for failure in [
        "invalid metadata",
        "cycle",
        "missing backing",
        "invalid receipt",
    ] {
        match failure {
            "invalid metadata" => peers.rules(
                "qemu-img",
                json!([{"prefix": ["info"], "stdout": "{\"filename\":null}"}]),
            )?,
            "cycle" | "missing backing" => {
                let backing = if failure == "cycle" {
                    overlay.clone()
                } else {
                    PathBuf::from("/nix/store/missing/base.qcow2")
                };
                peers.rules("qemu-img", json!([{"prefix": ["info"],
                    "stdout": serde_json::to_string(&json!({"filename": overlay, "backing-filename": backing}))?}]))?;
            }
            _ => {
                peers.rules("qemu-img", qemu.clone())?;
                fs::write(logs.join("config-apply.json"), "{incomplete")?;
            }
        }
        assert!(
            !common::run(&mut command(&peers, "run")?, None)?
                .status
                .success()
        );
        assert_eq!(
            fs::read_to_string(nix_root.join("nix/var/nix/gcroots/airlock/manifest.tsv"))?,
            manifest
        );
        assert_eq!(
            peers
                .calls()?
                .iter()
                .filter(|call| call["command"] == "nix-store")
                .count(),
            4
        );
    }
    Ok(())
}

/// The shipped source-status predicate accepts clean local, tracking and ahead main branches, and rejects edits or ambiguous history.
#[test]
fn source_status_accepts_clean_bundle_clones_but_rejects_dirty_or_detached_state() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    let source = root.join("source");
    fs::create_dir(&source)?;
    git(&source, &["init", "-q", "-b", "main"])?;
    for key in ["user.name", "user.email"] {
        git(&source, &["config", key, "fixture@example.invalid"])?;
    }
    let inspect = |repo: &std::path::Path, expected: bool| -> Result<()> {
        let status = git(
            repo,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
            ],
        )?;
        let path = root.join("status");
        fs::write(&path, status)?;
        let result = common::run(
            Command::new(common::binary("AIRLOCK_SMOKE_BIN", "airlock-smoke"))
                .arg("clean-main")
                .arg(path),
            None,
        )?;
        assert_eq!(result.status.success(), expected);
        Ok(())
    };
    inspect(&source, false)?;
    fs::write(source.join("tracked"), "baseline\n")?;
    git(&source, &["add", "."])?;
    git(&source, &["commit", "-qm", "Baseline"])?;
    inspect(&source, true)?;
    let bundle = root.join("config.bundle");
    git(
        &source,
        &[
            "bundle",
            "create",
            bundle.to_str().unwrap(),
            "HEAD",
            "refs/heads/main",
        ],
    )?;
    let clone = root.join("staging");
    git(
        root,
        &["clone", bundle.to_str().unwrap(), clone.to_str().unwrap()],
    )?;
    for key in ["user.name", "user.email"] {
        git(&clone, &["config", key, "fixture@example.invalid"])?;
    }
    inspect(&clone, true)?;
    fs::write(clone.join("flake.lock"), "{}\n")?;
    inspect(&clone, false)?;
    git(&clone, &["add", "flake.lock"])?;
    inspect(&clone, false)?;
    git(&clone, &["commit", "-qm", "Lock inputs"])?;
    inspect(&clone, true)?;
    fs::write(clone.join("tracked"), "changed\n")?;
    inspect(&clone, false)?;
    fs::write(clone.join("tracked"), "baseline\n")?;
    git(&clone, &["checkout", "--detach"])?;
    inspect(&clone, false)?;
    git(&clone, &["checkout", "-b", "other"])?;
    inspect(&clone, false)?;
    Ok(())
}

/// Optional host observation remains useful on a minimal machine and never provisions missing Nix/systemd tools.
#[test]
fn host_capabilities_tolerates_missing_optional_tools() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let report = common::json(
        Command::new(common::peer())
            .arg("host-capabilities")
            .env("PATH", temporary.path()),
        None,
    )?;
    assert_eq!(report["schema"], "airlock.host-capabilities.v1");
    assert_eq!(report["nix"], json!({"available": false, "version": ""}));
    assert_eq!(report["host"]["virtualization"], "");
    Ok(())
}
