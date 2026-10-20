//! Early-boot evidence remains private and survives journalctl failure.

use crate::common;
use anyhow::Result;
use std::{fs, os::unix::fs::PermissionsExt, process::Command};

/// Save complete journal output with private permissions, including on upstream failure.
#[test]
fn saves_evidence_even_when_journalctl_fails() -> Result<()> {
    for code in [0, 7] {
        let temporary = tempfile::tempdir()?;
        let peer = common::peer_link(temporary.path(), "journalctl")?;
        let output = common::run(
            Command::new(common::binary(
                "AIRLOCK_BOOT_JOURNAL_BIN",
                "airlock-boot-journal",
            ))
            .arg(peer)
            .arg("--root")
            .arg(temporary.path())
            .env("AIRLOCK_TEST_EXIT_CODE", code.to_string()),
            None,
        )?;
        assert_eq!(output.status.success(), code == 0);

        let files = fs::read_dir(temporary.path().join("var/log/airlock-boot"))?
            .collect::<std::io::Result<Vec<_>>>()?;
        assert_eq!(files.len(), 1);
        assert!(
            files[0]
                .file_name()
                .to_string_lossy()
                .ends_with("-initrd.log")
        );
        assert_eq!(fs::read(files[0].path())?, b"early boot evidence\n");
        assert_eq!(files[0].metadata()?.permissions().mode() & 0o777, 0o600);
    }
    Ok(())
}

/// A redirected log directory cannot receive writes through the recorder.
#[test]
fn rejects_redirected_log_directory() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let other = temporary.path().join("other");
    fs::create_dir(&other)?;
    std::os::unix::fs::symlink(&other, temporary.path().join("var"))?;

    let output = common::run(
        Command::new(common::binary(
            "AIRLOCK_BOOT_JOURNAL_BIN",
            "airlock-boot-journal",
        ))
        .arg(common::peer())
        .arg("--root")
        .arg(temporary.path()),
        None,
    )?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
    assert_eq!(fs::read_dir(other)?.count(), 0);
    Ok(())
}

/// Neither a FIFO nor a multiply linked file can redirect private log publication.
#[test]
fn rejects_fifo_and_hardlink_without_hanging() -> Result<()> {
    for fifo in [true, false] {
        let temporary = tempfile::tempdir()?;
        let directory = temporary.path().join("var/log/airlock-boot");
        fs::create_dir_all(&directory)?;
        let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
        let target = directory.join(format!("{boot}-initrd.log", boot = boot.trim()));
        let original = temporary.path().join("unrelated");
        fs::write(&original, b"keep")?;
        if fifo {
            rustix::fs::mknodat(
                rustix::fs::CWD,
                &target,
                rustix::fs::FileType::Fifo,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                0,
            )?;
        } else {
            fs::hard_link(&original, &target)?;
        }

        let output = common::run(
            Command::new(common::binary(
                "AIRLOCK_BOOT_JOURNAL_BIN",
                "airlock-boot-journal",
            ))
            .arg(common::peer())
            .arg("--root")
            .arg(temporary.path()),
            None,
        )?;
        assert!(!output.status.success());
        if !fifo {
            assert!(String::from_utf8_lossy(&output.stderr).contains("not a regular file"));
        }
        assert_eq!(fs::read(original)?, b"keep");
    }
    Ok(())
}
