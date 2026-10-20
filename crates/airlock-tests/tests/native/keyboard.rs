//! Keyboard startup recovery is authorized only by a fresh kernel failure and can run once per boot.

use crate::common::{self, peers::Peers};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

const BOOT: &str = "12345678-1234-4234-8234-123456789abc";

/// One disposable controller and cursor-aware journal source for the actual guard process.
struct Guard {
    // Owns all sysfs attributes, state records and inert journal command replies.
    peers: Peers,
}

/// An explicit kernel enable failure is the only event authorizing controller recovery.
fn failure() -> Value {
    json!({"__CURSOR":"old","MESSAGE":"atkbd serio0: Failed to enable keyboard on isa0060/serio0"})
}

/// A later keyboard initialization provides software evidence without claiming physical verification.
fn healthy() -> Value {
    json!({"__CURSOR":"new","MESSAGE":"input: AT Translated Set 2 keyboard"})
}

impl Guard {
    /// Configure finite journal replies and a known boot identity without exposing host sysfs.
    fn new() -> Result<Self> {
        let mut peers = Peers::new()?;
        fs::write(peers.path("boot-id"), BOOT)?;
        peers.rules(
            "journalctl",
            json!([
                {"exists":"journal-failed","exit":1},
                {"contains":"--sync"},
                {"contains":"--after-cursor","stdout_file":"after.jsonl"},
                {"stdout_file":"before.jsonl"}
            ]),
        )?;
        let guard = Self { peers };
        guard.journal(&[], &[])?;
        Ok(guard)
    }

    /// Create an i8042 controller whose attributes are regular fixture files.
    fn present(&self, bound: bool) -> Result<()> {
        let port = self.peers.path("controller/i8042/serio0");
        fs::create_dir_all(&port)?;
        if bound {
            fs::create_dir(self.peers.path("controller/atkbd"))?;
            symlink("../../atkbd", port.join("driver"))?;
            fs::create_dir_all(port.join("input/input1"))?;
            fs::write(port.join("input/input1/name"), "AT keyboard")?;
        }
        fs::write(self.peers.path("controller/bind"), "")?;
        fs::write(self.peers.path("controller/unbind"), "")?;
        Ok(())
    }

    /// Replace full and cursor-relative journal streams as scenario data, never executable scripts.
    fn journal(&self, before: &[Value], after: &[Value]) -> Result<()> {
        for (name, rows) in [("before.jsonl", before), ("after.jsonl", after)] {
            let mut bytes = Vec::new();
            for row in rows {
                serde_json::to_writer(&mut bytes, row)?;
                bytes.push(b'\n');
            }
            fs::write(self.peers.path(name), bytes)?;
        }
        Ok(())
    }

    /// Invoke only the production guard command with the controller and logs privately mounted.
    fn run(
        &self,
        action: &str,
        service_result: &str,
    ) -> Result<airlock_vm_service::support::Output> {
        let mut command = self
            .peers
            .command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
        command
            .args(["input", action])
            .env("SERVICE_RESULT", service_result);
        for (name, path) in [
            ("AIRLOCK_INPUT_BOOT_ID_FILE", "boot-id"),
            ("AIRLOCK_INPUT_LOG_ROOT", "logs"),
            ("AIRLOCK_INPUT_CONTROLLER", "controller"),
            ("AIRLOCK_INPUT_JOURNALCTL", "bin/journalctl"),
        ] {
            command.env(name, self.peers.path(path));
        }
        common::run(
            &mut common::namespace(&command, &self.peers.path(""))?,
            None,
        )
    }

    /// Read the boot-local public recovery outcome.
    fn report(&self) -> Result<Value> {
        self.peers
            .read(format!("logs/{BOOT}/input-controller.json"))
    }

    /// Alter a recorded state to model interruption or an explicit systemd result.
    fn save(&self, record: &Value) -> Result<()> {
        fs::write(
            self.peers
                .path(format!("logs/{BOOT}/input-controller.json")),
            serde_json::to_vec(record)?,
        )?;
        Ok(())
    }
}

/// Ordinary messages and absent controllers cannot cause a reset or claim physical input works.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_keyboard_guard_requires_an_explicit_enable_failure() -> Result<()> {
    let absent = Guard::new()?;
    absent.run("boot-guard", "success")?.checked()?;
    assert_eq!(absent.report()?["status"], "not_applicable");
    for records in [
        vec![],
        vec![healthy()],
        vec![json!({"MESSAGE":"atkbd serio0: Failed to deactivate keyboard on isa0060/serio0"})],
    ] {
        let guard = Guard::new()?;
        guard.present(true)?;
        guard.journal(&records, &[])?;
        guard.run("boot-guard", "success")?.checked()?;
        assert_eq!(guard.report()?["status"], "no_failure_detected");
        assert_eq!(guard.report()?["physical_input_verified"], false);
        assert_eq!(fs::read(guard.peers.path("controller/unbind"))?, b"");
    }
    Ok(())
}

/// Successful recovery records the original evidence once and advances its verification cursor.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_keyboard_recovery_is_durable_and_cursor_scoped() -> Result<()> {
    let guard = Guard::new()?;
    guard.present(true)?;
    guard.journal(&[failure()], &[healthy()])?;
    guard.run("boot-guard", "success")?.checked()?;
    let report = guard.report()?;
    assert_eq!(report["status"], "reinitialized");
    assert_eq!(report["attempted"], true);
    assert_eq!(report["physical_input_verified"], false);
    assert_eq!(
        fs::metadata(
            guard
                .peers
                .path(format!("logs/{BOOT}/input-controller.json"))
        )?
        .permissions()
        .mode()
            & 0o777,
        0o600
    );
    assert_eq!(fs::read(guard.peers.path("controller/unbind"))?, b"i8042");
    assert_eq!(
        guard
            .peers
            .read(format!("logs/{BOOT}/input-kernel-before.json"))?,
        json!([failure()])
    );
    fs::write(guard.peers.path("controller/unbind"), "no second write")?;
    let mut later = healthy();
    later["__CURSOR"] = json!("later");
    guard.journal(&[], &[later])?;
    guard.run("boot-guard", "success")?.checked()?;
    assert_eq!(guard.report()?["verified_cursor"], "later");
    assert_eq!(
        fs::read(guard.peers.path("controller/unbind"))?,
        b"no second write"
    );
    assert!(guard.peers.calls()?.iter().any(|call| {
        call["args"]
            .as_array()
            .unwrap()
            .ends_with(&[json!("--after-cursor"), json!("new")])
    }));

    let mut recurrence = failure();
    recurrence["__CURSOR"] = json!("recurrence");
    guard.journal(&[], &[recurrence.clone()])?;
    assert!(!guard.run("boot-guard", "success")?.status.success());
    assert_eq!(
        guard
            .peers
            .read(format!("logs/{BOOT}/input-kernel-before.json"))?,
        json!([failure()])
    );
    assert_eq!(
        guard
            .peers
            .read(format!("logs/{BOOT}/input-kernel-recurrence.json"))?,
        json!([recurrence])
    );
    Ok(())
}

/// Failed, unbound and interrupted attempts are never retried, including after finalization.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_keyboard_failure_and_interruption_cannot_repeat_reset() -> Result<()> {
    for fault in ["write", "kernel", "unbound", "interrupted", "finalize"] {
        let guard = Guard::new()?;
        guard.present(fault != "unbound")?;
        guard.journal(
            &[failure()],
            &[if fault == "kernel" {
                failure()
            } else {
                healthy()
            }],
        )?;
        if fault == "write" {
            fs::remove_file(guard.peers.path("controller/unbind"))?;
            fs::create_dir(guard.peers.path("controller/unbind"))?;
        }
        if fault == "interrupted" {
            guard.journal(&[], &[])?;
            guard.run("boot-guard", "success")?.checked()?;
            let mut record = guard.report()?;
            record["status"] = json!("resetting");
            record["attempted"] = json!(true);
            guard.save(&record)?;
        } else if fault == "finalize" {
            guard.run("boot-guard", "success")?.checked()?;
            guard.run("finalize", "timeout")?.checked()?;
            assert_eq!(guard.report()?["service_result"], "timeout");
        } else {
            assert!(
                !guard.run("boot-guard", "success")?.status.success(),
                "{fault}"
            );
        }
        assert_eq!(guard.report()?["attempted"], true);
        assert!(
            !guard.run("boot-guard", "success")?.status.success(),
            "{fault}"
        );
        assert!(
            guard.report()?["error"]
                .as_str()
                .unwrap()
                .contains("no second attempt")
        );
    }
    Ok(())
}

/// Missing cursors and journal errors cannot authorize a mutation, and a direct rebind needs durable intent.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_keyboard_guard_refuses_incomplete_evidence() -> Result<()> {
    for journal_failure in [false, true] {
        let guard = Guard::new()?;
        guard.present(true)?;
        if journal_failure {
            fs::write(guard.peers.path("journal-failed"), "")?;
        } else {
            guard.journal(&[json!({"MESSAGE":failure()["MESSAGE"]})], &[])?;
        }
        assert!(!guard.run("boot-guard", "success")?.status.success());
        assert_eq!(guard.report()?["attempted"], false);
        assert_eq!(fs::read(guard.peers.path("controller/unbind"))?, b"");
    }
    let guard = Guard::new()?;
    guard.present(true)?;
    assert!(!guard.run("rebind", "success")?.status.success());
    assert_eq!(fs::read(guard.peers.path("controller/unbind"))?, b"");
    Ok(())
}

/// All controller attributes must be safe before the guard disconnects any input.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_keyboard_rebind_prepares_controls_and_rejects_replay() -> Result<()> {
    for (attribute, damage) in [("bind", "missing"), ("bind", "symlink"), ("unbind", "fifo")] {
        let guard = Guard::new()?;
        guard.present(true)?;
        guard.run("boot-guard", "success")?.checked()?;
        let mut record = guard.report()?;
        record["status"] = json!("resetting");
        record["attempted"] = json!(true);
        guard.save(&record)?;
        let path = guard.peers.path("controller").join(attribute);
        fs::remove_file(&path)?;
        let victim = guard.peers.path("controller/untouched");
        fs::write(&victim, "unchanged")?;
        match damage {
            "symlink" => symlink(&victim, &path)?,
            "fifo" => rustix::fs::mknodat(
                rustix::fs::CWD,
                &path,
                rustix::fs::FileType::Fifo,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                0,
            )?,
            _ => {}
        }
        assert!(!guard.run("rebind", "success")?.status.success());
        assert_eq!(fs::read(&victim)?, b"unchanged");
        if attribute == "bind" {
            assert_eq!(fs::read(guard.peers.path("controller/unbind"))?, b"");
        }
    }
    let guard = Guard::new()?;
    guard.present(true)?;
    guard.journal(&[failure()], &[healthy()])?;
    guard.run("boot-guard", "success")?.checked()?;
    let mut record = guard.report()?;
    record["status"] = json!("resetting");
    guard.save(&record)?;
    fs::write(
        guard.peers.path("controller/unbind"),
        "must not reset again",
    )?;
    let result = guard.run("rebind", "success")?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already consumed"));
    assert_eq!(
        fs::read(guard.peers.path("controller/unbind"))?,
        b"must not reset again"
    );
    Ok(())
}
