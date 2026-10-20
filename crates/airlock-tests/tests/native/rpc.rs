//! Exercise native machine and VM receivers against inert import/coordinator peers in a private namespace.

use crate::common::{self, peers::Peers};
use airlock_tests::dialogue::Dialogue;
use anyhow::{Context, Result, ensure};
use flate2::{Compression, write::GzEncoder};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::Command,
    time::Duration,
};

const CANDIDATE: &str = "/var/lib/airlock/machine/candidates/abc";

const SCHEMA: &str = "airlock.sys-ctrl-machine-release.v2";

/// A temporary store view, coordinator configuration and exact transfer header.
struct Rpc {
    // Owns every mutable file and process receipt in the isolated Airlock state mount.
    peers: Peers,
    // Deliberately finite replies returned by inert coordinator and importer executables.
    config: Value,
    // Sender-authored identity and negotiated objects; mutated independently by rejection tests.
    header: Value,
    // Immutable release identity whose fixture directory is mounted in the private Nix store.
    release: String,
    // Executable coordinator identity admitted by that immutable release manifest.
    coordinator: String,
}

impl Rpc {
    /// Build a read-only store view forwarding actual tool dependencies to a separately retained mount.
    fn new() -> Result<Self> {
        let peers = Peers::new()?;
        let release = format!("/nix/store/{hash}-rpc-release", hash = "a".repeat(32));
        let coordinator = format!(
            "/nix/store/{hash}-rpc-coordinator/bin/airlock-dom0-service",
            hash = "d".repeat(32)
        );
        for name in ["machine/candidates/abc", "etc", "release", "view"] {
            fs::create_dir_all(peers.path(name))?;
        }
        for entry in fs::read_dir("/nix/store")? {
            let name = entry?.file_name();
            symlink(
                PathBuf::from("/var/reference-store").join(&name),
                peers.path("view").join(name),
            )?;
        }
        fs::create_dir(
            peers
                .path("view")
                .join(PathBuf::from(&release).file_name().unwrap()),
        )?;
        let coordinator_directory = peers.path("view").join(
            PathBuf::from(&coordinator)
                .strip_prefix("/nix/store")?
                .parent()
                .unwrap(),
        );
        fs::create_dir_all(&coordinator_directory)?;
        symlink(
            "/var/lib/airlock/bin/engine",
            coordinator_directory.join("airlock-dom0-service"),
        )?;
        for name in ["engine", "nix", "nix-store", "systemd-run"] {
            common::peer_link(&peers.path("bin"), name)?;
        }
        let manifest = json!({"schema":"airlock.machine-release.v1","revision":"b".repeat(40),
            "lockHash":"c".repeat(64),"profile":"usb-lab","coordinator":coordinator,"guestTransport":"direct-pool-v1"});
        fs::write(
            peers.path("release/release.json"),
            serde_json::to_vec(&manifest)?,
        )?;
        let header = json!({"schema":SCHEMA,"release":release,"revision":"b".repeat(40),"lock_hash":"c".repeat(64),
            "profile":"usb-lab","paths":{&release:4},"receipts":{}});
        Ok(Self {
            peers,
            release,
            coordinator,
            header,
            config: json!({"plan":{"changed":["usr-home"]},"state":"f".repeat(64),"family":"machine","valid":[]}),
        })
    }

    /// Require source approval using the same protected marker as the installed receiver.
    fn protected(&mut self) -> Result<()> {
        fs::write(self.peers.path("etc/review-required"), "")?;
        self.header["review_authorization"] = json!("e".repeat(64));
        Ok(())
    }

    /// Construct a receiver command with a private store, /etc policy and state directory.
    fn command(&self, operation: &str, action: Option<&str>) -> Result<Command> {
        fs::write(
            self.peers.path("config.json"),
            serde_json::to_vec(&self.config)?,
        )?;
        let mut command = Command::new("bwrap");
        command
            .args([
                "--die-with-parent",
                "--unshare-user",
                "--uid",
                "0",
                "--gid",
                "0",
                "--ro-bind",
                "/",
                "/",
                "--ro-bind",
            ])
            .arg(self.peers.path("view"))
            .arg("/nix/store")
            .arg("--ro-bind")
            .arg(self.peers.path("release"))
            .arg(&self.release)
            .args(["--tmpfs", "/etc", "--ro-bind"])
            .arg(self.peers.path("etc"))
            .arg("/etc/airlock")
            .args([
                "--tmpfs",
                "/var",
                "--dir",
                "/var/lib",
                "--ro-bind",
                "/nix/store",
                "/var/reference-store",
                "--bind",
            ])
            .arg(self.peers.path(""))
            .arg("/var/lib/airlock")
            .args([
                "--proc",
                "/proc",
                "--dev",
                "/dev",
                "--setenv",
                "PATH",
                "/var/lib/airlock/bin",
                "--setenv",
                "TMPDIR",
                "/var/lib/airlock",
            ])
            .arg(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))
            .args(["rpc", "--engine", "/var/lib/airlock/bin/engine", operation])
            .env("AIRLOCK_TEST_RPC_ROOT", "/var/lib/airlock");
        if let Some(action) = action {
            command.arg(action);
        }
        Ok(command)
    }

    /// Send an exact request stream and retain all protocol frames, including missing-object negotiation.
    fn wire(
        &self,
        operation: &str,
        action: Option<&str>,
        wire: Vec<u8>,
        accepted: bool,
    ) -> Result<Vec<Value>> {
        let result = common::run(&mut self.command(operation, action)?, Some(wire))?;
        ensure!(
            result.status.success() == accepted,
            "unexpected RPC outcome: {stdout}; {stderr}",
            stdout = String::from_utf8_lossy(&result.stdout),
            stderr = String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout)?
            .lines()
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }

    /// Serialize one framed JSON request without a transfer body.
    fn request(
        &self,
        operation: &str,
        action: Option<&str>,
        value: Value,
        accepted: bool,
    ) -> Result<Value> {
        let mut wire = serde_json::to_vec(&value)?;
        wire.push(b'\n');
        self.wire(operation, action, wire, accepted)?
            .pop()
            .context("missing RPC reply")
    }

    /// Serialize the reviewed machine header and a finite synthetic NAR export.
    fn transfer(&self, action: &str, accepted: bool) -> Result<Vec<Value>> {
        self.wire(
            "machine-receive",
            Some(action),
            stream(&self.header, b"exported NAR")?,
            accepted,
        )
    }
}

/// Compose the normal negotiation header and gzip-compressed export without a Python transport helper.
fn stream(header: &Value, data: &[u8]) -> Result<Vec<u8>> {
    let mut wire = serde_json::to_vec(header)?;
    wire.push(b'\n');
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(data)?;
    wire.extend(gzip.finish()?);
    Ok(wire)
}

/// Invalid release identities and typed authority are rejected before any import or coordinator operation.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_rpc_rejects_invalid_headers_and_control_authority() -> Result<()> {
    let rpc = Rpc::new()?;
    for change in [
        json!({"schema":"unknown"}),
        json!({"release":"/tmp/release"}),
        json!({"revision":"bad"}),
        json!({"profile":"other"}),
        json!({"paths":{"/tmp/invalid":1}}),
        json!({"paths":{&rpc.release:true}}),
        json!({"paths":{&rpc.release:-1}}),
        json!({"unexpected":"ignored"}),
    ] {
        let mut header = rpc.header.clone();
        header
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(!rpc.request("machine-receive", Some("plan"), header, false)?["error"].is_null());
        assert!(rpc.peers.calls()?.is_empty());
    }
    let mut missing = rpc.header.clone();
    missing.as_object_mut().unwrap().remove("receipts");
    rpc.request("machine-receive", Some("apply"), missing, false)?;
    for payload in [
        json!({"action":"shell"}),
        json!({"action":"status","command":"id"}),
        json!({"action":"release-prune","release":"/tmp/untrusted","confirm":"/tmp/untrusted"}),
        json!({"action":"trash-purge","identity":"../private","confirm":"../private"}),
        json!({"action":"trash-purge","identity":"123-usr-home","confirm":"different"}),
        json!({"action":"status","before_expiry":true}),
        json!({"action":"trash-purge","identity":"--help","confirm":"--help"}),
        json!({"action":"trash-purge","identity":"test","confirm":"test","before_expiry":1}),
        json!({"action":"release-retain","release":"/tmp/untrusted","retain":true}),
    ] {
        let mut request = json!({"schema":"airlock.machine-control.v1"});
        request
            .as_object_mut()
            .unwrap()
            .extend(payload.as_object().unwrap().clone());
        rpc.request("machine-control", None, request, false)?;
        assert!(rpc.peers.calls()?.is_empty());
    }
    Ok(())
}

/// Imported objects must match negotiation exactly before admission, binding or detached activation.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_machine_receiver_negotiates_import_and_dispatches_in_order() -> Result<()> {
    for action in ["prepare", "plan", "apply"] {
        let rpc = Rpc::new()?;
        let replies = rpc.transfer(action, true)?;
        assert_eq!(replies[0], json!({"schema":SCHEMA,"missing":[rpc.release]}));
        let result = replies.last().unwrap();
        assert_eq!(result["paths_transferred"], 1);
        assert_eq!(result["nar_bytes"], 12);
        assert_eq!(fs::read(rpc.peers.path("imported"))?, b"exported NAR");
        let calls = rpc.peers.calls()?;
        assert_eq!(calls[0]["program"], "nix");
        assert_eq!(calls[1]["args"], json!(["--import"]));
        if action == "prepare" {
            assert_eq!(
                calls.last().unwrap()["args"],
                json!(["admission-prepare", rpc.release, "--caller", "sys-ctrl"])
            );
        } else {
            assert!(calls.iter().any(|call| call["args"][0] == "machine-bind"));
            assert_eq!(
                calls.iter().any(|call| call["args"][0] == "prepared-check"),
                action == "plan"
            );
            assert_eq!(
                calls.iter().any(|call| call["program"] == "systemd-run"),
                action == "apply"
            );
            if action == "apply" {
                assert!(
                    calls.last().unwrap()["args"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(rpc.coordinator))
                );
            } else {
                let path = rpc.peers.path("machine/prepared").join(format!(
                    "{prepared}.json",
                    prepared = result["prepared"].as_str().unwrap()
                ));
                assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
                assert_eq!(
                    serde_json::from_slice::<Value>(&fs::read(path)?)?["candidate"],
                    CANDIDATE
                );
            }
        }
    }
    let duplicate = format!("/nix/store/{hash}-rpc-release", hash = "a".repeat(32));
    for (imported, expected) in [
        (json!([]), "negotiated missing inventory"),
        (
            json!([duplicate, duplicate]),
            "negotiated missing inventory",
        ),
        (
            json!([format!("/nix/store/{hash}-extra", hash = "b".repeat(32))]),
            "negotiated missing inventory",
        ),
    ] {
        let mut rpc = Rpc::new()?;
        rpc.config["imported_paths"] = imported;
        let result = rpc.transfer("prepare", false)?;
        assert!(
            result.last().unwrap()["error"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{result:?}"
        );
        assert!(
            !rpc.peers
                .calls()?
                .iter()
                .any(|call| call["args"][0] == "admission-prepare")
        );
    }
    Ok(())
}

/// Corrupt transfer framing, provenance, importer failure and size overflow cannot reach binding.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_invalid_transfer_preserves_binding_boundary_and_bounded_errors() -> Result<()> {
    for damage in ["provenance", "gzip", "import", "space-overflow"] {
        let mut rpc = Rpc::new()?;
        match damage {
            "provenance" => {
                let path = rpc.peers.path("release/release.json");
                let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
                manifest["revision"] = json!("a".repeat(40));
                fs::write(path, serde_json::to_vec(&manifest)?)?;
            }
            "import" => rpc.config["fail"] = json!("nix-store"),
            "space-overflow" => {
                rpc.header["paths"] = json!({&rpc.release:u64::MAX,format!("/nix/store/{hash}-extra",hash="b".repeat(32)):1})
            }
            _ => {}
        }
        let mut wire = stream(&rpc.header, b"exported NAR")?;
        if damage == "gzip" {
            wire.truncate(wire.len() - 5);
        }
        rpc.wire("machine-receive", Some("plan"), wire, false)?;
        assert!(
            !rpc.peers
                .calls()?
                .iter()
                .any(|call| call["args"][0] == "machine-bind"),
            "{damage}"
        );
    }
    let mut rpc = Rpc::new()?;
    rpc.config["fail"] = json!("machine-bind");
    rpc.config["failure"] =
        json!({"error":format!("specific failure {detail}",detail="x".repeat(5000))});
    let replies = rpc.transfer("plan", false)?;
    assert_eq!(replies.len(), 2);
    let reply = replies.last().unwrap();
    assert_eq!(reply["schema"], SCHEMA);
    let error = reply["error"].as_str().unwrap();
    assert!(error.contains("specific failure"));
    assert!(error.len() <= 4096);
    Ok(())
}

/// Reuse requires the same inspected plan, machine state and source identity with an intact saved receipt.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_prepared_machine_apply_revalidates_every_binding() -> Result<()> {
    for change in ["none", "state", "plan", "identity", "tamper", "pruned"] {
        let mut rpc = Rpc::new()?;
        let replies = rpc.transfer("plan", true)?;
        let plan = replies.last().unwrap();
        let previous = rpc.peers.calls()?.len();
        let mut header = rpc.header.clone();
        header.as_object_mut().unwrap().remove("paths");
        header.as_object_mut().unwrap().remove("receipts");
        header["prepared"] = plan["prepared"].clone();
        match change {
            "state" => rpc.config["state"] = json!("a".repeat(64)),
            "plan" => rpc.config["plan"] = json!({"changed":["sys-gui"]}),
            "identity" => header["revision"] = json!("a".repeat(40)),
            "tamper" => {
                let path = rpc.peers.path("machine/prepared").join(format!(
                    "{token}.json",
                    token = plan["prepared"].as_str().unwrap()
                ));
                let mut saved: Value = serde_json::from_slice(&fs::read(&path)?)?;
                saved["coordinator"] = json!("/malicious");
                fs::write(path, serde_json::to_vec(&saved)?)?;
            }
            "pruned" => rpc.config["fail"] = json!("prepared-check"),
            _ => {}
        }
        let result = rpc.request("machine-receive", Some("apply"), header, change == "none")?;
        let calls = rpc.peers.calls()?;
        let later = &calls[previous..];
        assert!(!later.iter().any(|call| matches!(
            call["program"].as_str(),
            Some("nix" | "nix-store")
        ) || call["args"][0] == "machine-bind"));
        if change == "none" {
            assert_eq!(result["prepared_reused"], true);
            assert_eq!(result["paths_transferred"], 0);
            assert!(
                later.last().unwrap()["args"]
                    .as_array()
                    .unwrap()
                    .ends_with(&[json!("--expected-state"), json!("f".repeat(64))])
            );
        } else {
            assert!(!later.iter().any(|call| call["program"] == "systemd-run"));
        }
    }
    Ok(())
}

/// Revocation during transfer cannot be bypassed by changing review family or dropping the marker.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_transfer_rechecks_approval_before_admission() -> Result<()> {
    for action in ["prepare", "plan", "apply"] {
        let mut rpc = Rpc::new()?;
        rpc.protected()?;
        rpc.config["revoke_after_import"] = json!(true);
        if action == "prepare" {
            rpc.config["drop_review_marker"] = json!(true);
        }
        rpc.transfer(action, false)?;
        let calls = rpc.peers.calls()?;
        assert_eq!(
            calls
                .iter()
                .filter(|call| call["args"][0] == "review-control")
                .count(),
            2
        );
        assert!(!calls.iter().any(|call| matches!(
            call["args"][0].as_str(),
            Some("admission-prepare" | "machine-bind")
        )));
    }
    let mut rpc = Rpc::new()?;
    rpc.protected()?;
    rpc.config["family"] = json!("guest");
    rpc.transfer("apply", false)?;
    assert_eq!(rpc.peers.calls()?.len(), 1);
    Ok(())
}

/// Inventory arrives before the sender writes gzip bytes, and failed import is reported before sender EOF.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_transfer_handshake_and_import_failure_are_interactive() -> Result<()> {
    for failed in [false, true] {
        let mut rpc = Rpc::new()?;
        if failed {
            rpc.config["fail"] = json!("nix-store");
        }
        let mut dialogue = Dialogue::spawn(&mut rpc.command("machine-receive", Some("prepare"))?)?;
        let mut header = serde_json::to_vec(&rpc.header)?;
        header.push(b'\n');
        dialogue.send(&header)?;
        assert_eq!(
            dialogue.reply(Duration::from_secs(10))?["missing"],
            json!([rpc.release])
        );
        if failed {
            let reply = dialogue.reply(Duration::from_secs(10))?;
            assert!(
                reply["error"]
                    .as_str()
                    .unwrap()
                    .contains("importer exited unsuccessfully")
            );
            assert!(!dialogue.finish()?.success());
        } else {
            let wire = stream(&json!({}), b"interactive NAR")?;
            dialogue.send(&wire[3..])?;
            assert!(
                dialogue.finish()?.success(),
                "{errors}",
                errors = dialogue.errors()?
            );
            assert_eq!(dialogue.reply(Duration::from_secs(1))?["nar_bytes"], 15);
            assert_eq!(fs::read(rpc.peers.path("imported"))?, b"interactive NAR");
        }
    }
    Ok(())
}

/// Guest approval is bound before activation, and failure preserves the coordinator's busy reason.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_guest_receiver_binds_approval_before_activation() -> Result<()> {
    let receipt = json!({"generation":2,"domain":"usr-home","source_revision":"b".repeat(40),"lock_hash":"c".repeat(64)});
    for action in ["plan", "apply"] {
        let mut rpc = Rpc::new()?;
        rpc.protected()?;
        let mut request = receipt.clone();
        request["review_authorization"] = json!("e".repeat(64));
        let reply = rpc.request("guest-receive", Some(action), request, true)?;
        let calls = rpc.peers.calls()?;
        assert_eq!(
            calls[..3]
                .iter()
                .map(|call| call["args"][0].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["review-control", "guest-plan", "review-bind"]
        );
        assert_eq!(calls[1]["request"], receipt);
        if action == "apply" {
            assert_eq!(calls[3]["program"], "systemd-run");
            assert!(
                calls[3]["args"]
                    .as_array()
                    .unwrap()
                    .ends_with(&[json!("apply"), reply["candidate"].clone()])
            );
            assert!(
                reply["unit"]
                    .as_str()
                    .unwrap()
                    .starts_with("airlock-machine-deploy-")
            );
        } else {
            assert_eq!(calls.len(), 3);
            assert!(reply.get("unit").is_none());
        }
    }
    let mut rpc = Rpc::new()?;
    rpc.config["fail"] = json!("guest-plan");
    rpc.config["failure"] = json!({"status":"busy","error":"deployment holds the machine lock","lock":{"owner":"Deployment abc"}});
    let reply = rpc.request(
        "guest-receive",
        Some("plan"),
        json!({"generation":2}),
        false,
    )?;
    assert_eq!(reply["schema"], "airlock.guest-deploy-error.v1");
    assert_eq!(reply["status"], "busy");
    assert_eq!(reply["lock"]["owner"], "Deployment abc");
    assert_eq!(reply["exit_code"], 1);
    assert_eq!(reply["operation"], "guest-plan");

    for candidate in [
        "/tmp/candidate",
        "/var/lib/airlock/machine/candidates/./abc",
    ] {
        let mut rpc = Rpc::new()?;
        rpc.config["candidate"] = json!(candidate);
        rpc.request("guest-receive", Some("apply"), receipt.clone(), false)?;
        assert!(
            !rpc.peers
                .calls()?
                .iter()
                .any(|call| call["program"] == "systemd-run")
        );
    }
    let mut rpc = Rpc::new()?;
    rpc.protected()?;
    rpc.request("guest-receive", Some("apply"), receipt.clone(), false)?;
    assert!(rpc.peers.calls()?.is_empty());
    rpc.config["fail"] = json!("review-bind");
    let mut request = receipt;
    request["review_authorization"] = json!("e".repeat(64));
    rpc.request("guest-receive", Some("apply"), request, false)?;
    assert!(
        !rpc.peers
            .calls()?
            .iter()
            .any(|call| call["program"] == "systemd-run")
    );
    Ok(())
}

/// Guest framing rejects incomplete, duplicated or oversized JSON before calling any coordinator.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_guest_receiver_rejects_invalid_wire_before_authority() -> Result<()> {
    let rpc = Rpc::new()?;
    for wire in [
        b"{}".to_vec(),
        b"[]\n".to_vec(),
        b"{\"generation\":1,\"generation\":2}\n".to_vec(),
        [b"\"".as_slice(), &vec![b'a'; 65536], b"\"\n"].concat(),
    ] {
        rpc.wire("guest-receive", Some("apply"), wire, false)?;
        assert!(rpc.peers.calls()?.is_empty());
    }
    Ok(())
}

/// Recovery stays available without sys-ctrl; destructive maintenance requires an exact recorded target.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_machine_recovery_uses_fixed_actions_and_exact_confirmations() -> Result<()> {
    let rpc = Rpc::new()?;
    for action in ["status", "verify", "resume", "rollback", "release-list"] {
        let reply = rpc.request(
            "machine-control",
            None,
            json!({"schema":"airlock.machine-control.v1","action":action}),
            true,
        )?;
        let calls = rpc.peers.calls()?;
        if matches!(action, "resume" | "rollback") {
            assert!(
                reply["unit"]
                    .as_str()
                    .unwrap()
                    .starts_with("airlock-machine-deploy-")
            );
            assert_eq!(calls.last().unwrap()["program"], "systemd-run");
            assert_eq!(
                calls.last().unwrap()["args"].as_array().unwrap().last(),
                Some(&json!(action))
            );
        } else {
            assert_eq!(reply["status"], "ready");
            assert_eq!(calls.last().unwrap()["args"], json!([action]));
        }
    }
    rpc.request(
        "machine-control",
        None,
        json!({"schema":"airlock.machine-control.v1","action":"release-prune",
        "release":CANDIDATE,"confirm":CANDIDATE,"allow_unexpired_trash":["123-usr-home"]}),
        true,
    )?;
    assert!(
        rpc.peers.calls()?.last().unwrap()["args"]
            .as_array()
            .unwrap()
            .ends_with(&[
                json!("release-prune"),
                json!(CANDIDATE),
                json!("--confirm"),
                json!(CANDIDATE),
                json!("--allow-unexpired-trash"),
                json!("123-usr-home")
            ])
    );
    for alias in [
        "/var/lib/airlock/machine/candidates/./abc",
        "/var/lib/airlock/machine/candidates//abc",
    ] {
        let before = rpc.peers.calls()?.len();
        rpc.request("machine-control",None,json!({"schema":"airlock.machine-control.v1","action":"release-prune","release":alias,"confirm":alias}),false)?;
        assert_eq!(rpc.peers.calls()?.len(), before);
    }
    symlink(
        "/var/lib/airlock/bin/engine",
        rpc.peers.path("machine/controller"),
    )?;
    rpc.request(
        "machine-control",
        None,
        json!({"schema":"airlock.machine-control.v1","action":"rollback"}),
        true,
    )?;
    assert!(
        rpc.peers.calls()?.last().unwrap()["args"]
            .as_array()
            .unwrap()
            .contains(&json!("/var/lib/airlock/machine/controller"))
    );
    Ok(())
}

/// Concurrent receivers share one user namespace so lock-owner authentication is exercised accurately.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_parallel_transfer_reports_owner_then_releases_for_retry() -> Result<()> {
    let rpc = Rpc::new()?;
    let command = rpc.command("machine-receive", Some("prepare"))?;
    let native = common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service");
    let args: Vec<_> = command.get_args().collect();
    let index = args
        .iter()
        .position(|arg| *arg == native.as_os_str())
        .context("receiver position")?;
    let mut race = Command::new(command.get_program());
    race.args(&args[..index])
        .arg(common::peer())
        .arg("rpc-race")
        .arg(native)
        .arg(serde_json::to_string(&rpc.header)?);
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            race.env(key, value);
        }
    }
    let reply = common::json(&mut race, None)?;
    assert_eq!(reply["lock"]["owner"], "Machine transfer from sys-ctrl");
    Ok(())
}
