//! Native volume attachment binds complete boot identity before mutating Qubes or stopping its daemon.

use crate::common::{self, peers::Peers, store::Store};
use airlock_tests::attachment::volume;
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::fd::AsFd,
    path::PathBuf,
    process::Command,
};

/// Two admitted roots and a supplied Qubes snapshot whose attachment effects are separately recorded.
struct Attachment {
    // Owns pool generations, source data and the actual storage executable.
    store: Store,
    // Owns data-only bridge and daemon replies in a private command namespace.
    peers: Peers,
    // Snapshot observed before the attempted installation; mutations are explicit in each scenario.
    snapshot: Value,
    // Original selected generation and boot artifact identity.
    original: Value,
    // Different admitted root selected by the staged candidate.
    successor: Value,
    // Materialized successor root source used in the native identity comparison.
    replacement: PathBuf,
}

impl Attachment {
    /// Admit two actual generations and project the original native volume definitions into upstream snapshots.
    fn new() -> Result<Self> {
        let store = Store::new()?;
        let original = common::json(
            store
                .admission()
                .args(["--kernel-name", "kernel", "--approval-source", "/approved"]),
            None,
        )?;
        let replacement = store.temporary.path().join("replacement-root.img");
        fs::write(&replacement, fs::read(&store.root)?)?;
        let mut file = fs::OpenOptions::new().write(true).open(&replacement)?;
        file.seek(SeekFrom::Start(4096))?;
        file.write_all(b"successor")?;
        let successor = common::json(
            store
                .command(&["pool", "admit", "--pool-root"])
                .arg(&store.pool)
                .args([
                    "--domain",
                    "usr-home",
                    "--system",
                    &store.system,
                    "--closure-source",
                ])
                .arg(&store.closure)
                .arg("--physical-store-root")
                .arg(&store.physical)
                .args([
                    "--verify-mode",
                    "tree",
                    "--kernel-name",
                    "kernel",
                    "--approval-source",
                    "/approved",
                    "--root-source",
                ])
                .arg(&replacement),
            None,
        )?;
        let configs = common::json(
            store
                .command(&["domain", "image-volumes", "usr-home", "--pool-root"])
                .arg(&store.pool)
                .args([
                    "--admission",
                    "-",
                    "--metadata-size",
                    &original["view_size"].to_string(),
                ]),
            Some(serde_json::to_vec(&original)?),
        )?;
        let mut volumes: Vec<_> = ["root", "private", "volatile", "kernel"]
            .into_iter()
            .map(|name| {
                volume(&json!({
                    "name": name, "pool": "default", "vid": format!("usr-home/{name}"), "rw": true,
                    "snap_on_start": false, "save_on_stop": true, "size": 4096
                }))
            })
            .collect();
        volumes.extend(configs.as_array().unwrap().iter().map(volume));
        let snapshot = json!({
            "domain": "usr-home", "klass": "StandaloneVM", "running": false,
            "features": {"airlock-store-source": "/approved", "airlock-store-mode": "shared-closure",
                "airlock-store-view-backend": "erofs-objectmap", "airlock-store-generation": original["generation"].to_string()},
            "volumes": volumes, "references": [],
            "pools": {"airlock-closure": {"name": "airlock-closure", "driver": "airlock-closure", "root_dir": store.pool}}
        });
        let peers = Peers::new()?;
        common::peer_link(&peers.path("bin"), "storage-bridge")?;
        fs::write(
            peers.path("staged.json"),
            serde_json::to_vec(&json!({"usr-home": successor["generation"]}))?,
        )?;
        let value = Self {
            store,
            peers,
            snapshot,
            original,
            successor,
            replacement,
        };
        value.save()?;
        Ok(value)
    }

    /// Publish the next supplied upstream snapshot separately from the native pool's own durable state.
    fn save(&self) -> Result<()> {
        fs::write(
            self.peers.path("qubes.json"),
            serde_json::to_vec(&self.snapshot)?,
        )?;
        Ok(())
    }

    /// Construct a process-local bridge environment while retaining the production daemon-state override semantics.
    fn environment(&self, command: &mut Command) {
        command
            .env(
                "AIRLOCK_QUBES_OBJECTS_BIN",
                self.peers.path("bin/storage-bridge"),
            )
            .env(
                "AIRLOCK_QUBES_STORAGE_STATE",
                self.peers.path("integration"),
            )
            .env("AIRLOCK_TEST_STORAGE_BRIDGE_ROOT", self.peers.path(""))
            .env("AIRLOCK_STAGED_GENERATIONS", self.peers.path("staged.json"))
            .env("AIRLOCK_NIX_DOMAIN_STORAGE_QUBESD_ACTIVE", "false");
    }

    /// Build the real ensure-store invocation, replacing an option's value only when a scenario requests it.
    fn command(&self, overrides: &[(&str, &str)]) -> Result<Command> {
        let pool = self.store.pool.to_str().unwrap();
        let root = self.replacement.to_str().unwrap();
        let mut arguments = vec![
            "storage",
            "domain",
            "ensure-store",
            "usr-home",
            self.store.closure.to_str().unwrap(),
            "/approved",
            "--system",
            &self.store.system,
            "--root",
            root,
            "--kernel",
            "kernel",
            "--pool-root",
            pool,
            "--verify-mode",
            "tree",
        ];
        for (option, value) in overrides {
            if let Some(index) = arguments.iter().position(|candidate| candidate == option) {
                arguments[index + 1] = value;
            } else {
                arguments.extend([option, value]);
            }
        }
        let mut command = self.peers.command(&self.store.service)?;
        self.environment(&mut command);
        command.args(arguments);
        Ok(command)
    }

    /// Count actual requested upstream installs, independently of whether they succeeded.
    fn installs(&self) -> Result<Vec<Value>> {
        let path = self.peers.path("bridge-calls.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let requests = fs::read_to_string(path)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(requests
            .into_iter()
            .filter(|request| request["operation"] == "install")
            .collect())
    }

    /// Supply daemon observations and durable stop/start effects without running a real service manager.
    fn daemon(&mut self, initial: &str, stuck: bool, query_error: bool) -> Result<()> {
        fs::write(self.peers.path("systemctl.state"), initial)?;
        self.peers.rules("systemctl", json!([
            {"prefix": ["show"], "stdout_file": "systemctl.state", "exit": if query_error { 1 } else { 0 }},
            {"prefix": ["stop"], "write": if stuck { json!({}) } else { json!({"systemctl.state": "inactive"}) }},
            {"prefix": ["start"], "write": {"systemctl.state": "active"}}
        ]))
    }

    /// Compare only daemon mutations; repeated read-only status queries may vary without changing the contract.
    fn daemon_actions(&self) -> Result<Vec<Value>> {
        Ok(self
            .peers
            .calls()?
            .into_iter()
            .filter(|call| call["command"] == "systemctl" && call["args"][0] != "show")
            .map(|call| call["args"][0].clone())
            .collect())
    }
}

/// A root change selects its exact staged generation, while running VMs refuse changes but accept equivalent identity.
#[test]
fn attachment_binds_staged_root_and_preserves_running_vm_identity() -> Result<()> {
    let attachment = Attachment::new()?;
    let original_generation = attachment.original["generation"].to_string();
    let rejected = common::run(
        &mut attachment.command(&[("--generation", &original_generation)])?,
        None,
    )?;
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stdout)
            .contains("staged generation differs from request")
    );
    assert!(attachment.installs()?.is_empty());
    let selected_generation = attachment.successor["generation"].to_string();
    let result = common::json(
        &mut attachment.command(&[("--generation", &selected_generation)])?,
        None,
    )?;
    assert_eq!(result["generation"], attachment.successor["generation"]);
    assert_eq!(result["root_source"], json!(attachment.replacement));
    assert_eq!(attachment.installs()?.len(), 1);

    let mut attachment = Attachment::new()?;
    attachment.snapshot["running"] = json!(true);
    attachment.save()?;
    let output = common::run(&mut attachment.command(&[])?, None)?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("domain is running"));
    assert!(attachment.installs()?.is_empty());
    let root = attachment.store.root.to_str().unwrap();
    let same = attachment.peers.path("same.json");
    assert!(
        !common::run(
            attachment
                .command(&[("--root", root)])?
                .env("AIRLOCK_STAGED_GENERATIONS", &same),
            None
        )?
        .status
        .success()
    );
    fs::write(
        &same,
        serde_json::to_vec(&json!({"usr-home": attachment.original["generation"]}))?,
    )?;
    let result = common::json(
        attachment
            .command(&[("--root", root)])?
            .env("AIRLOCK_STAGED_GENERATIONS", same),
        None,
    )?;
    assert_eq!(result["generation"], attachment.original["generation"]);
    assert!(result.get("engine_current_generation").is_none());
    assert!(attachment.installs()?.is_empty());
    Ok(())
}

/// Unchanged fast paths cannot bypass staged identity, pool, frontend or shared-private-disk validation.
#[test]
fn attachment_rejects_invalid_state_before_qubes_mutation() -> Result<()> {
    let unselected = Attachment::new()?;
    let output = common::run(
        unselected
            .command(&[])?
            .arg("--require-retained")
            .env_remove("AIRLOCK_STAGED_GENERATIONS"),
        None,
    )?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("refusing implicit admission"));
    assert!(unselected.installs()?.is_empty());
    let attachment = Attachment::new()?;
    let result = common::run(
        &mut attachment.command(&[("--root", attachment.store.root.to_str().unwrap())])?,
        None,
    )?;
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("staged generation identity mismatch")
    );
    assert!(attachment.installs()?.is_empty());
    for damage in ["pool", "frontends", "shared-private"] {
        let mut attachment = Attachment::new()?;
        match damage {
            "pool" => {
                attachment.snapshot["pools"]["airlock-closure"]["root_dir"] = json!("/wrong-pool")
            }
            "frontends" => {
                let volumes = attachment.snapshot["volumes"].as_array_mut().unwrap();
                let count = volumes.len();
                volumes.swap(count - 2, count - 1);
            }
            _ => {
                let private = &attachment.snapshot["volumes"][1];
                attachment.snapshot["references"] = json!([{
                    "domain": "other-vm", "pool": private["pool"], "vid": private["vid"], "source": null
                }]);
            }
        }
        attachment.save()?;
        let mut command = if damage == "shared-private" {
            let mut command = attachment.peers.command(&attachment.store.service)?;
            attachment.environment(&mut command);
            command.args(["storage", "domain", "verify-native-volumes", "usr-home"]);
            command
        } else {
            attachment.command(&[])?
        };
        assert!(!common::run(&mut command, None)?.status.success());
        assert!(attachment.installs()?.is_empty());
    }
    for option in ["--device", "--blob-device"] {
        let attachment = Attachment::new()?;
        assert!(
            !common::run(&mut attachment.command(&[(option, "/dev/xvdz")])?, None)?
                .status
                .success()
        );
        assert!(attachment.installs()?.is_empty());
    }
    Ok(())
}

/// Failed installation restores qubesd without advancing selection, and a durable interrupted restart is resumed once.
#[test]
fn attachment_failure_restores_daemon_and_preserves_selected_pool() -> Result<()> {
    let mut attachment = Attachment::new()?;
    attachment.daemon("active", false, false)?;
    let current = attachment.store.pool.join("domains/usr-home/current");
    let selected = fs::read_link(&current)?;
    let output = common::run(
        attachment
            .command(&[])?
            .env("AIRLOCK_TEST_STORAGE_FAIL_INSTALL", "1")
            .env("AIRLOCK_NIX_DOMAIN_STORAGE_QUBESD_ACTIVE", "true"),
        None,
    )?;
    assert!(!output.status.success());
    assert_eq!(
        attachment.daemon_actions()?,
        [json!("stop"), json!("start")]
    );
    assert_eq!(fs::read_link(current)?, selected);

    let mut attachment = Attachment::new()?;
    attachment.daemon("inactive", false, false)?;
    let pending = attachment.peers.path("integration/qubesd-restart.json");
    fs::create_dir_all(pending.parent().unwrap())?;
    fs::write(&pending, br#"{"service":"qubesd.service","restore":true}"#)?;
    fs::write(
        attachment.peers.path("staged.json"),
        serde_json::to_vec(&json!({"usr-home": attachment.original["generation"]}))?,
    )?;
    let result = common::json(
        &mut attachment.command(&[("--root", attachment.store.root.to_str().unwrap())])?,
        None,
    )?;
    assert_eq!(result["metadata_changed"], false);
    assert_eq!(attachment.daemon_actions()?, [json!("start")]);
    assert!(!pending.exists());
    assert!(attachment.installs()?.is_empty());
    Ok(())
}

/// Unknown daemon state fails closed, and installation waits until a requested stop actually reaches inactive.
#[test]
fn attachment_requires_a_known_stopped_daemon_before_installation() -> Result<()> {
    for (initial, query_error) in [("unknown", false), ("", false), ("active", true)] {
        let mut attachment = Attachment::new()?;
        attachment.daemon(initial, false, query_error)?;
        let result = common::run(
            attachment
                .command(&[])?
                .env_remove("AIRLOCK_NIX_DOMAIN_STORAGE_QUBESD_ACTIVE"),
            None,
        )?;
        assert!(!result.status.success());
        assert!(attachment.installs()?.is_empty());
        assert!(attachment.daemon_actions()?.is_empty());
    }
    for (initial, stuck) in [("active", true), ("deactivating", false)] {
        let mut attachment = Attachment::new()?;
        attachment.daemon(initial, stuck, false)?;
        let output = common::run(
            attachment
                .command(&[])?
                .env_remove("AIRLOCK_NIX_DOMAIN_STORAGE_QUBESD_ACTIVE"),
            None,
        )?;
        assert_eq!(output.status.success(), !stuck);
        assert_eq!(
            attachment.daemon_actions()?,
            [json!("stop"), json!("start")]
        );
        assert!(
            !attachment
                .peers
                .path("integration/qubesd-restart.json")
                .exists()
        );
        assert_eq!(attachment.installs()?.len(), usize::from(!stuck));
        if !stuck {
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout)?["generation"],
                attachment.successor["generation"]
            );
        }
    }
    Ok(())
}

/// One attachment lock covers every pool so competing operations cannot install through a second pool root.
#[test]
fn attachment_lock_excludes_operations_across_pool_roots() -> Result<()> {
    let attachment = Attachment::new()?;
    let locks = attachment.peers.path("integration/.locks");
    fs::create_dir_all(&locks)?;
    let path = locks.join("operation-attachment.lock");
    let held = fs::File::create(path)?;
    rustix::fs::flock(
        held.as_fd(),
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )?;
    for pool in [
        &attachment.store.pool,
        &attachment.peers.path("another-pool"),
    ] {
        let output = common::run(
            &mut attachment.command(&[("--pool-root", pool.to_str().unwrap())])?,
            None,
        )?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("operation-attachment.lock"));
        assert!(attachment.installs()?.is_empty());
    }
    Ok(())
}
