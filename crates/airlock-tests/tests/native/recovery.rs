//! GPU recovery scenarios retain durable evidence and one-attempt guards inside a private user namespace.

use crate::common::{self, peers::Peers};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::symlink, path::PathBuf};

const BOOT: &str = "12345678-1234-4234-8234-123456789abc";

const GPU: &str = "0000:09:03.0";

/// Display recovery sees only this synthetic sysfs/debugfs tree and finite command replies.
struct Recovery {
    // Owns hardware attributes, boot identity, command peers and durable recovery reports.
    peers: Peers,
}

impl Recovery {
    /// Prepare a supported AMD GPU and its reset attribute without exposing host hardware.
    fn new() -> Result<Self> {
        let mut peers = Peers::new()?;
        let device = peers.path("sys/bus/pci/devices").join(GPU);
        fs::create_dir_all(&device)?;
        fs::write(device.join("vendor"), "0x1002")?;
        fs::write(device.join("device"), "0x1681")?;
        fs::create_dir_all(peers.path("sys/drivers/amdgpu"))?;
        symlink("../../../../drivers/amdgpu", device.join("driver"))?;
        let debug = peers.path("debug").join(GPU);
        fs::create_dir_all(&debug)?;
        fs::write(debug.join("amdgpu_gpu_recover"), "reset returned")?;
        fs::write(peers.path("boot-id"), BOOT)?;
        peers.rules(
            "gpu-journal",
            json!([
                {"contains":"--since","missing":"healthy","stdout":"flip_done timed out\n"},
                {}
            ]),
        )?;
        peers.rules("gpu-systemctl", json!([{}]))?;
        peers.rules("gpu-cat", json!([{}]))?;
        Ok(Self { peers })
    }

    /// Run the production service with private writable state and return both success and failure evidence.
    fn run(&self, action: &str, confirmed: bool) -> Result<airlock_vm_service::support::Output> {
        let mut command = self.peers.command(common::binary(
            "AIRLOCK_SYS_GUI_BIN",
            "airlock-sys-gui-service",
        ))?;
        command.args(["gpu", action]);
        if confirmed {
            command.arg("--confirm-display-stall");
        }
        for (name, path) in [
            ("AIRLOCK_GPU_SYSFS", "sys"),
            ("AIRLOCK_GPU_DEBUG", "debug"),
            ("AIRLOCK_GPU_RECOVERY_RUNTIME", "run"),
            ("AIRLOCK_GPU_RECOVERY_LOGS", "logs"),
            ("AIRLOCK_GPU_BOOT_ID", "boot-id"),
            ("AIRLOCK_GPU_JOURNALCTL", "bin/gpu-journal"),
            ("AIRLOCK_GPU_SYSTEMCTL", "bin/gpu-systemctl"),
            ("AIRLOCK_GPU_CAT", "bin/gpu-cat"),
        ] {
            command.env(name, self.peers.path(path));
        }
        common::run(
            &mut common::namespace(&command, &self.peers.path(""))?,
            None,
        )
    }

    /// Resolve the report retained by the boot's one-attempt marker back into the fixture.
    fn report(&self) -> Result<(PathBuf, Value)> {
        let marker = self.peers.read("run/reset-attempted.json")?;
        let path = marker["report"]
            .as_str()
            .context("recovery report path")?
            .strip_prefix("/var/lib/airlock/")
            .context("report outside private fixture")?;
        Ok((self.peers.path(path), self.peers.read(path)?))
    }

    /// Require a rejection with an observable reason instead of accepting any process failure.
    fn rejects(&self, action: &str, confirmed: bool, reason: &str) -> Result<()> {
        let output = self.run(action, confirmed)?;
        assert!(!output.status.success(), "{action} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(reason),
            "{reason}: {stderr}",
            stderr = String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

/// Capture remains observational, and reset requires both confirmation and a recent timeout.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_capture_and_reset_authorization() -> Result<()> {
    let recovery = Recovery::new()?;
    let result: Value = serde_json::from_slice(&recovery.run("capture", false)?.checked()?)?;
    assert_eq!(result["status"], "captured");
    assert!(!recovery.peers.path("run/reset-attempted.json").exists());
    assert!(recovery.peers.calls()?.iter().all(|call| {
        !call["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|argument| argument.as_str().unwrap().contains("amdgpu_gpu_recover"))
    }));
    recovery.rejects("recover", false, "confirm")?;
    fs::write(recovery.peers.path("healthy"), "")?;
    recovery.rejects("recover", true, "no recent")?;
    assert!(!recovery.peers.path("run/reset-started.json").exists());
    Ok(())
}

/// A successful reset and a desktop restart have independent, durable one-attempt claims.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_reset_and_desktop_restart_cannot_replay() -> Result<()> {
    let recovery = Recovery::new()?;
    let result: Value = serde_json::from_slice(&recovery.run("recover", true)?.checked()?)?;
    assert_eq!(result["physical_recovery_verified"], false);
    let (path, mut report) = recovery.report()?;
    assert_eq!(report["reset"]["returncode"], 0);
    assert!(recovery.peers.path("run/reset-started.json").exists());
    recovery.rejects("recover", true, "already attempted")?;
    report["status"] = json!("reset-requested");
    fs::write(path, serde_json::to_vec(&report)?)?;
    recovery.rejects("reset", false, "already consumed")?;

    assert!(
        !recovery
            .peers
            .path("run/desktop-restart-attempted.json")
            .exists()
    );
    recovery.run("restart-desktop", true)?.checked()?;
    recovery.rejects("restart-desktop", true, "already attempted")?;
    Ok(())
}

/// Failed or unknown resets do not authorize a desktop restart.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_failed_and_unknown_reset_preserve_restart_guard() -> Result<()> {
    for unknown in [false, true] {
        let recovery = Recovery::new()?;
        if unknown {
            recovery.run("recover", true)?.checked()?;
            let (path, mut report) = recovery.report()?;
            report["reset"]["timeout"] = json!(true);
            fs::write(path, serde_json::to_vec(&report)?)?;
        } else {
            let control = recovery
                .peers
                .path("debug")
                .join(GPU)
                .join("amdgpu_gpu_recover");
            fs::remove_file(&control)?;
            fs::create_dir(&control)?;
            recovery.rejects("recover", true, "reset failed")?;
        }
        recovery.rejects("restart-desktop", true, "did not finish")?;
        assert!(
            !recovery
                .peers
                .path("run/desktop-restart-attempted.json")
                .exists()
        );
    }
    Ok(())
}

/// A later GPU identity cannot consume an earlier capture or reset authorization.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_hardware_identity_change_blocks_recovery() -> Result<()> {
    for action in ["reset", "restart-desktop"] {
        let recovery = Recovery::new()?;
        if action == "reset" {
            let capture: Value =
                serde_json::from_slice(&recovery.run("capture", false)?.checked()?)?;
            let logical = capture["report"].as_str().context("capture path")?;
            let relative = logical
                .strip_prefix("/var/lib/airlock/")
                .context("capture outside fixture")?;
            let path = recovery.peers.path(relative);
            let mut report = recovery.peers.read(relative)?;
            report["status"] = json!("reset-requested");
            fs::write(path, serde_json::to_vec(&report)?)?;
            fs::write(
                recovery.peers.path("run/reset-attempted.json"),
                serde_json::to_vec(&json!({
                    "report":logical,"boot_id":BOOT,"gpu":GPU
                }))?,
            )?;
        } else {
            recovery.run("recover", true)?.checked()?;
        }
        for parent in ["sys/bus/pci/devices", "debug"] {
            fs::rename(
                recovery.peers.path(parent).join(GPU),
                recovery.peers.path(parent).join("0000:09:04.0"),
            )?;
        }
        recovery.rejects(action, action != "reset", "identity changed")?;
        assert!(
            !recovery
                .peers
                .path("run/desktop-restart-attempted.json")
                .exists()
        );
    }
    let recovery = Recovery::new()?;
    fs::write(
        recovery
            .peers
            .path("sys/bus/pci/devices")
            .join(GPU)
            .join("device"),
        "0x9999",
    )?;
    recovery.rejects("capture", false, "exactly one")?;
    Ok(())
}

/// Truncated diagnostics cannot authorize reset, and capture limits preserve earlier evidence.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_evidence_is_durable_and_bounded() -> Result<()> {
    let mut recovery = Recovery::new()?;
    recovery.peers.rules(
        "gpu-journal",
        json!([
            {"require_record":{"directory":"logs","field":"status","value":"capturing"}}
        ]),
    )?;
    let result: Value = serde_json::from_slice(&recovery.run("capture", false)?.checked()?)?;
    assert_eq!(result["status"], "captured");

    for total in [false, true] {
        let mut recovery = Recovery::new()?;
        let contents = if total {
            vec![1; 900 * 1024]
        } else {
            [
                b"flip_done timed out\n".as_slice(),
                &vec![b'x'; 2 * 1024 * 1024],
            ]
            .concat()
        };
        fs::write(recovery.peers.path("large-evidence"), contents)?;
        recovery.peers.rules(
            "gpu-journal",
            if total {
                json!([{"stdout_file":"large-evidence"}])
            } else {
                json!([{"contains":"--since","stdout_file":"large-evidence"},{}])
            },
        )?;
        recovery.rejects(
            "recover",
            true,
            if total {
                "total GPU evidence limit"
            } else {
                "no recent"
            },
        )?;
        let path = fs::read_dir(recovery.peers.path("logs"))?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .context("GPU evidence record")?;
        assert!(fs::metadata(&path)?.len() < 8 * 1024 * 1024);
        let report: Value = serde_json::from_slice(&fs::read(path)?)?;
        assert_eq!(report["kernel"]["returncode"], 0);
        assert!(
            report["recent_kernel"]["error"]
                .as_str()
                .unwrap()
                .contains(if total {
                    "total GPU evidence limit"
                } else {
                    "capture limit"
                })
        );
        assert!(!recovery.peers.path("run/reset-attempted.json").exists());
    }
    Ok(())
}

/// An operation still holding reset ownership cannot be displaced by a second recovery request.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_active_reset_lock_preserves_ownership() -> Result<()> {
    let recovery = Recovery::new()?;
    recovery.run("capture", false)?.checked()?;
    let lock = fs::File::open(super::gpu::operation_lock(
        &recovery.peers.path("run"),
        "operation-reset.lock",
    )?)?;
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    recovery.rejects("recover", true, "lock")?;
    assert!(!recovery.peers.path("run/reset-attempted.json").exists());
    Ok(())
}
