//! EFI cache reclamation authenticates every sealed boot artifact and preserves recovery bytes across filesystem boundaries.

use crate::{common, host_store::pe};
use anyhow::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, os::unix::fs::symlink, path::PathBuf, process::Command};

/// One sealed boot selection and two disposable duplicate cache files.
struct Boot {
    // Owns the ESP, descriptors, lock state and quarantine output.
    temporary: tempfile::TempDir,
    // Private EFI tree used for both read-only inspection and namespace apply.
    esp: PathBuf,
    // Exact original bytes of every authenticated file.
    files: BTreeMap<String, Vec<u8>>,
    // Authenticated descriptor can be updated only for the intentional malformed-PE case.
    record: Value,
}

impl Boot {
    /// Encode real Xen PE configuration without requiring firmware or a loop-mounted disk.
    fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        let esp = temporary.path().join("esp");
        for name in ["EFI/airlock", "EFI/nixos", "loader/entries"] {
            fs::create_dir_all(esp.join(name))?;
        }
        fs::create_dir(temporary.path().join("descriptors"))?;
        let files: BTreeMap<String, Vec<u8>> = BTreeMap::from([
            (
                "EFI/airlock/xen.efi".into(),
                pe("kernel=kernel.efi init=/nix/store/system/init\nramdisk=initrd.efi\n"),
            ),
            ("EFI/airlock/kernel.efi".into(), b"kernel".to_vec()),
            ("EFI/airlock/initrd.efi".into(), b"initrd".to_vec()),
            (
                "loader/entries/airlock-xen.conf".into(),
                b"title Airlock\nefi /EFI/airlock/xen.efi\n".to_vec(),
            ),
        ]);
        let mut fingerprints = serde_json::Map::new();
        for (name, bytes) in &files {
            fs::write(esp.join(name), bytes)?;
            fingerprints.insert(name.clone(), json!({"size": bytes.len(), "sha256": format!("{hash:x}", hash = Sha256::digest(bytes))}));
        }
        let record = json!({"schema": "airlock.host-boot-artifacts.v1", "entries": [{"entry": "airlock-xen.conf", "files": fingerprints}]});
        fs::write(
            temporary.path().join("descriptors/system.json"),
            serde_json::to_vec(&record)?,
        )?;
        fs::write(esp.join("EFI/nixos/initrd.efi"), "duplicate cache")?;
        fs::write(esp.join("EFI/nixos/partial.tmp"), "incomplete copy")?;
        Ok(Self {
            temporary,
            esp,
            files,
            record,
        })
    }

    /// Inspect the native command without touching a system boot partition or acquiring mutation authority.
    fn command(&self) -> Command {
        let mut command = Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        command
            .args(["boot-cache", "--esp"])
            .arg(&self.esp)
            .arg("--descriptors")
            .arg(self.temporary.path().join("descriptors"));
        command
    }

    /// Compare every retained boot artifact against its exact original bytes.
    fn intact(&self) -> Result<()> {
        for (name, bytes) in &self.files {
            assert_eq!(&fs::read(self.esp.join(name))?, bytes);
        }
        Ok(())
    }
}

/// Planning authenticates sealed files and refuses unknown entries, links, damaged payloads and structurally invalid Xen PE files.
#[test]
fn boot_cache_planning_refuses_unverified_boot_state() -> Result<()> {
    let boot = Boot::new()?;
    let plan = common::json(&mut boot.command(), None)?;
    assert_eq!(plan["sealed_entries"], 1);
    assert_eq!(plan["bytes"], "duplicate cacheincomplete copy".len());
    assert_eq!(fs::read_dir(boot.esp.join("EFI/nixos"))?.count(), 2);
    boot.intact()?;
    let extra = boot.esp.join("EFI/nixos/.extra-files");
    fs::create_dir(&extra)?;
    common::run(&mut boot.command(), None)?.checked()?;
    fs::write(extra.join("unknown"), "keep")?;
    assert!(!common::run(&mut boot.command(), None)?.status.success());

    for damage in ["entry", "payload", "link", "parent-link", "bad-pe"] {
        let mut boot = Boot::new()?;
        match damage {
            "entry" => fs::write(
                boot.esp.join("loader/entries/other.conf"),
                "efi /EFI/nixos/native.efi\n",
            )?,
            "payload" => fs::write(boot.esp.join("EFI/airlock/initrd.efi"), "corrupt")?,
            "link" => {
                let path = boot.esp.join("EFI/nixos/partial.tmp");
                fs::remove_file(&path)?;
                symlink(boot.esp.join("EFI/airlock/initrd.efi"), path)?;
            }
            "parent-link" => {
                fs::rename(boot.esp.join("EFI/airlock"), boot.esp.join("EFI/saved"))?;
                symlink("saved", boot.esp.join("EFI/airlock"))?;
            }
            _ => {
                let bytes = b"MZ".iter().copied().chain([0; 62]).collect::<Vec<_>>();
                fs::write(boot.esp.join("EFI/airlock/xen.efi"), &bytes)?;
                boot.record["entries"][0]["files"]["EFI/airlock/xen.efi"] = json!({"size": bytes.len(), "sha256": format!("{hash:x}", hash = Sha256::digest(bytes))});
                fs::write(
                    boot.temporary.path().join("descriptors/system.json"),
                    serde_json::to_vec(&boot.record)?,
                )?;
            }
        }
        assert!(
            !common::run(&mut boot.command(), None)?.status.success(),
            "{damage}"
        );
        assert!(boot.esp.join("EFI/nixos/initrd.efi").exists());
    }
    Ok(())
}

/// Applying requires another filesystem, verifies copied recovery bytes and never changes the sealed boot files.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_boot_cache_quarantine_preserves_recovery() -> Result<()> {
    for separate in [false, true] {
        let boot = Boot::new()?;
        let mut command = Command::new(common::peer());
        command.env("AIRLOCK_TEST_PEER_MODE", "boot-cache-maintenance");
        command.arg(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        command
            .arg(&boot.esp)
            .arg(boot.temporary.path().join("descriptors"))
            .arg(boot.temporary.path().join("recovery"))
            .arg(if separate { "separate" } else { "same" });
        let mounts = if separate {
            vec!["/var/test-esp"]
        } else {
            vec![]
        };
        let report = common::json(
            &mut common::namespace_tmpfs(&command, boot.temporary.path(), &mounts)?,
            None,
        )?;
        assert_eq!(report["success"], separate);
        if separate {
            assert_eq!(report["cache"], json!([]));
            let receipt: Value = serde_json::from_str(report["stdout"].as_str().unwrap())?;
            assert_eq!(receipt["status"], "complete");
            let relative = receipt["destination"]
                .as_str()
                .unwrap()
                .strip_prefix("/var/lib/airlock/")
                .unwrap();
            let saved = boot.temporary.path().join(relative);
            assert_eq!(fs::read(saved.join("initrd.efi"))?, b"duplicate cache");
            let durable: Value = serde_json::from_slice(&fs::read(saved.join("receipt.json"))?)?;
            assert_eq!(durable["status"], "complete");
        } else {
            assert_eq!(report["cache"].as_array().unwrap().len(), 2);
            assert!(
                report["stdout"]
                    .as_str()
                    .unwrap()
                    .contains("another filesystem")
            );
        }
        boot.intact()?;
    }
    Ok(())
}
