//! Desktop helper streaming and cancellation retain their failure coverage under Cargo.

use crate::common;
use airlock_vm_service::support::{self, Input};
use anyhow::Result;
use std::{
    fs::{self, File},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

/// Build logging preserves literal arguments, stdin/stdout and child failure without adding JSON receipts.
#[test]
fn build_progress_keeps_tool_streams_and_reports_failure() -> Result<()> {
    let service = common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service");
    let input = br#"{"status":"tool reply"}"#.to_vec();
    let success = common::run(
        Command::new(&service)
            .args(["storage", "build-progress", "--label", "fixture copy", "--"])
            .arg(common::peer())
            .arg("cat"),
        Some(input.clone()),
    )?;
    assert!(success.status.success());
    assert_eq!(success.stdout, input);
    assert!(String::from_utf8_lossy(&success.stderr).contains("completed"));

    let directory = tempfile::tempdir()?;
    let peer = common::peer_link(directory.path(), "journalctl")?;
    let failure = common::run(
        Command::new(&service)
            .args([
                "storage",
                "build-progress",
                "--label",
                "fixture failure",
                "--",
            ])
            .arg(peer)
            .env("AIRLOCK_TEST_EXIT_CODE", "37"),
        None,
    )?;
    assert!(!failure.status.success());
    assert_eq!(failure.stdout, b"early boot evidence\n");
    let log = String::from_utf8_lossy(&failure.stderr);
    assert!(log.contains("failed") && !log.contains("completed"));
    Ok(())
}

/// A detached pipe holder must be cleaned up even when an assertion fails.
struct Escaped {
    // The fixture writes only its own process ID here after leaving the helper group.
    pidfile: PathBuf,
}

impl Drop for Escaped {
    fn drop(&mut self) {
        if let Ok(text) = fs::read_to_string(&self.pidfile)
            && let Ok(raw) = text.parse::<i32>()
            && let Some(pid) = rustix::process::Pid::from_raw(raw)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    }
}

/// The header, exact payload and EOF reach the helper in order.
#[test]
fn stream_preserves_header_payload_and_eof() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let payload = temporary.path().join("capture");
    fs::write(&payload, vec![b'y'; 128 * 1024])?;
    let input = Input::file(b"header\n".to_vec(), File::open(payload)?, 128 * 1024)?;

    let result = support::run(
        Command::new(common::peer()).arg("summary"),
        Some(input),
        Duration::from_secs(3),
    )?
    .checked()?;
    assert_eq!(result, b"131079\nheader\n\n");
    Ok(())
}

/// Premature EOF cannot produce a successful transfer receipt.
#[test]
fn short_payload_is_rejected() -> Result<()> {
    let temporary = tempfile::NamedTempFile::new()?;
    fs::write(temporary.path(), b"short")?;
    let input = Input::file(
        b"header\n".to_vec(),
        File::open(temporary.path())?,
        128 * 1024,
    )?;

    let error = support::run(
        Command::new(common::peer()).arg("cat"),
        Some(input),
        Duration::from_secs(3),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("declared length"));
    Ok(())
}

/// An early zero exit is still failure when request bytes remain unsent.
#[test]
fn early_success_without_consuming_input_is_rejected() {
    assert!(
        support::run(
            Command::new(common::peer()).arg("exit"),
            Some(Input::bytes(vec![b'x'; 1024 * 1024])),
            Duration::from_secs(3)
        )
        .is_err()
    );
}

/// Device-like inputs are rejected before a helper can start.
#[test]
fn payload_must_be_a_regular_file() -> Result<()> {
    let error = Input::file(Vec::new(), File::open("/dev/null")?, 1)
        .err()
        .unwrap();
    assert!(error.to_string().contains("regular file"));
    Ok(())
}

/// A detached descendant retaining stdin cannot extend the process deadline.
#[test]
fn escaped_pipe_owner_cannot_defeat_timeout() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let escaped = Escaped {
        pidfile: temporary.path().join("pid"),
    };
    let started = Instant::now();
    let error = support::run(
        Command::new(common::peer())
            .arg("escape")
            .arg(&escaped.pidfile),
        Some(Input::bytes(vec![b'x'; 1024 * 1024])),
        Duration::from_millis(500),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("timed out"));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        escaped.pidfile.exists(),
        "the escape scenario did not start"
    );
    Ok(())
}

/// Cancellation targets the unreaped leader even after it joins another process group.
#[test]
fn leader_that_changes_group_is_cancelled() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let escaped = Escaped {
        pidfile: temporary.path().join("pid"),
    };
    let error = support::run(
        Command::new(common::peer())
            .arg("change-group")
            .arg(&escaped.pidfile),
        Some(Input::bytes(vec![b'x'; 1024 * 1024])),
        Duration::from_millis(500),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("timed out"), "{error:#}");
    let pid = fs::read_to_string(&escaped.pidfile)?;
    assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
    fs::remove_file(&escaped.pidfile)?;
    Ok(())
}

/// The GPU observer returns at its deadline rather than waiting again for a blocked helper.
#[test]
fn observer_timeout_is_bounded() -> Result<()> {
    let started = Instant::now();
    let result = airlock_vm_service::gpu::observe(
        Command::new(common::peer()).arg("hold"),
        Duration::from_millis(50),
    )?;
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(result.timeout);
    assert!(result.returncode.is_none());
    Ok(())
}

/// Complete exit status cannot certify oversized output, independently on either captured pipe.
#[test]
fn observer_rejects_oversized_successful_output() -> Result<()> {
    for stream in ["stdout", "stderr"] {
        let result = airlock_vm_service::gpu::observe(
            Command::new(common::peer()).args(["large-output", stream]),
            Duration::from_secs(2),
        )?;
        assert!(result.returncode.is_none());
        assert!(result.error.as_deref().unwrap().contains("capture limit"));
        assert!(result.stdout.len() <= 1024 * 1024);
        assert!(result.stderr.len() <= 1024 * 1024);
    }
    Ok(())
}

/// A successful leader cannot leave an inherited output pipe held open by a live descendant.
#[test]
fn observer_cancels_orphaned_output_owner() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let pidfile = temporary.path().join("child.pid");
    let result = airlock_vm_service::gpu::observe(
        Command::new(common::peer())
            .arg("orphan-output")
            .arg(&pidfile),
        Duration::from_secs(2),
    )?;
    assert_eq!(result.returncode, Some(0));
    assert!(result.error.is_none());
    let child = PathBuf::from(format!(
        "/proc/{pid}/stat",
        pid = fs::read_to_string(pidfile)?
    ));
    common::until(|| {
        Ok(!child.exists() || fs::read_to_string(&child)?.split_whitespace().nth(2) == Some("Z"))
    })?;
    Ok(())
}
