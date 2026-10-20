//! Sealed host boot selections are checksum-bound to retained pool generations and independent boot artifacts.

use crate::{
    common::{self, store::Store},
    storage_integrity::snapshot,
};
use anyhow::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

/// One admitted host, exact boot metadata and a disposable EFI partition.
struct Host {
    // Owns the immutable system tree and retained pool generation.
    store: Store,
    // Retained registration and image digests validated before selecting a boot entry.
    record: Value,
    // Private EFI files; no real firmware variables or host boot partition are accessed.
    esp: PathBuf,
}

impl Host {
    /// Admit a host generation and construct the minimum valid Xen PE boot entry.
    fn new() -> Result<Self> {
        let store = Store::new()?;
        let receipt = common::json(
            store
                .command(&["pool", "admit", "--pool-root"])
                .arg(&store.pool)
                .args([
                    "--domain",
                    "dom0",
                    "--system",
                    &store.system,
                    "--closure-source",
                ])
                .arg(&store.closure)
                .arg("--physical-store-root")
                .arg(&store.physical)
                .args(["--verify-mode", "tree", "--activate", "false"]),
            None,
        )?;
        let manifest = PathBuf::from(receipt["manifest"].as_str().unwrap());
        let registration = manifest.parent().unwrap().join("registration");
        fs::write(
            &registration,
            format!(
                "{system}\n{hash}\n42\n\n0\n",
                system = store.system,
                hash = "0".repeat(64)
            ),
        )?;
        let mut record = json!({
            "schema": "airlock.host-store.v1", "system": store.system, "generation": receipt["generation"],
            "manifest": manifest, "registration": registration, "closure_hash": receipt["closure_hash"],
            "metadata_image": receipt["canonical_metadata_image"], "blob_map": receipt["canonical_blob_map"]
        });
        for field in ["registration", "metadata_image", "blob_map"] {
            let bytes = fs::read(record[field].as_str().unwrap())?;
            record[format!("{field}_sha256")] =
                json!(format!("{hash:x}", hash = Sha256::digest(bytes)));
        }
        store.run(&[
            "pool",
            "mark-running",
            "--pool-root",
            store.pool_text(),
            "--domain",
            "dom0",
            "--generation",
            &receipt["generation"].to_string(),
            "--volume",
            "host-retained",
        ])?;
        let boots = store.pool.join("host/boots");
        fs::create_dir_all(&boots)?;
        let name = PathBuf::from(&store.system).file_name().unwrap().to_owned();
        fs::write(
            boots.join(name).with_extension("json"),
            serde_json::to_vec(&record)?,
        )?;
        let esp = store.temporary.path().join("esp");
        fs::create_dir_all(esp.join("loader/entries"))?;
        fs::create_dir_all(esp.join("EFI/nixos"))?;
        fs::write(
            esp.join("loader/loader.conf"),
            "default xen-host.conf\ntimeout 5\n",
        )?;
        fs::write(esp.join("EFI/nixos/kernel"), b"kernel")?;
        fs::write(esp.join("EFI/nixos/initrd"), b"initramfs")?;
        let config = format!(
            "kernel=kernel init={system}/init\nramdisk=initrd\n",
            system = store.system
        );
        fs::write(esp.join("EFI/nixos/xen.efi"), pe(&config))?;
        fs::write(
            esp.join("loader/entries/xen-host.conf"),
            format!(
                r"title NixOS
version Generation 7 test
efi /EFI/nixos/xen.efi
# airlock-generation 7
# airlock-role normal
# airlock-system {system}
# airlock-revision {revision}
# airlock-timestamp 1790778600
",
                system = store.system,
                revision = "a".repeat(40)
            ),
        )?;
        Ok(Self { store, record, esp })
    }

    /// Construct an offline host operation using only the private pool and EFI partition.
    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = self.store.command(&["host", "--offline", "--pool-root"]);
        command
            .arg(&self.store.pool)
            .arg("--efi-directory")
            .arg(&self.esp)
            .args(arguments);
        command
    }

    /// Run a successful command through its native JSON boundary.
    fn run(&self, arguments: &[&str]) -> Result<Value> {
        common::json(&mut self.command(arguments), None)
    }
}

/// Encode one bounded PE configuration section; scenarios corrupt its declared length or contents explicitly.
pub(super) fn pe(config: &str) -> Vec<u8> {
    let mut bytes = vec![0; 512 + config.len()];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&128_u32.to_le_bytes());
    bytes[128..132].copy_from_slice(b"PE\0\0");
    bytes[134..136].copy_from_slice(&1_u16.to_le_bytes());
    bytes[152..160].copy_from_slice(b".config\0");
    bytes[168..172].copy_from_slice(&(config.len() as u32).to_le_bytes());
    bytes[172..176].copy_from_slice(&512_u32.to_le_bytes());
    bytes[512..].copy_from_slice(config.as_bytes());
    bytes
}

/// Only the requested host record is read, and a retained running pin is mandatory for boot selection.
#[test]
fn host_boot_validation_requires_the_retained_generation() -> Result<()> {
    let host = Host::new()?;
    fs::write(
        host.store.pool.join("host/boots/newer.json"),
        b"{broken unrelated boot",
    )?;
    assert_eq!(host.run(&["validate", &host.store.system])?, host.record);
    fs::remove_file(host.store.pool.join("running/dom0/1/host-retained.json"))?;
    assert!(
        !common::run(&mut host.command(&["validate", &host.store.system]), None)?
            .status
            .success()
    );
    Ok(())
}

/// Changed registration, image or extent-map bytes cannot alter the selected boot configuration.
#[test]
fn host_boot_selection_checks_every_retained_digest() -> Result<()> {
    for field in ["registration", "metadata_image", "blob_map"] {
        let host = Host::new()?;
        let path = std::path::Path::new(host.record[field].as_str().unwrap());
        fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
        fs::write(path, b"corrupted")?;
        let before = fs::read(host.esp.join("loader/loader.conf"))?;
        assert!(
            !common::run(&mut host.command(&["select", &host.store.system]), None)?
                .status
                .success()
        );
        assert_eq!(fs::read(host.esp.join("loader/loader.conf"))?, before);
        assert!(!host.store.pool.join("host/selected.json").exists());
    }
    Ok(())
}

/// Sealing is idempotent and checksum-bound; the selected host remains protected from retirement.
#[test]
fn sealed_host_boot_is_independent_idempotent_and_protected() -> Result<()> {
    let host = Host::new()?;
    host.run(&["seal-boot", "--keep-default"])?;
    assert!(
        host.esp
            .join("loader/entries/airlock-xen-host.conf")
            .is_file()
    );
    assert!(!host.esp.join("loader/entries/xen-host.conf").exists());
    host.run(&["select", &host.store.system])?;
    assert!(
        fs::read_to_string(host.esp.join("loader/loader.conf"))?
            .contains("default airlock-xen-host.conf")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(host.store.pool.join("host/selected.json"))?)?,
        host.record
    );
    let before = snapshot(&host.esp)?;
    host.run(&["seal-boot", "--keep-default"])?;
    assert_eq!(snapshot(&host.esp)?, before);
    assert!(
        !common::run(&mut host.command(&["retire", &host.store.system]), None)?
            .status
            .success()
    );
    assert_eq!(snapshot(&host.esp)?, before);
    fs::write(host.esp.join("EFI/airlock/initrd"), b"damaged initramfs")?;
    assert!(
        !common::run(&mut host.command(&["select", &host.store.system]), None)?
            .status
            .success()
    );
    Ok(())
}

/// Ambiguous or oversized Xen configuration is rejected before publishing a sealed loader entry.
#[test]
fn host_boot_sealing_refuses_unbounded_or_ambiguous_configuration() -> Result<()> {
    for damage in ["oversized-config", "duplicate-kernel"] {
        let host = Host::new()?;
        let path = host.esp.join("EFI/nixos/xen.efi");
        let mut bytes = fs::read(&path)?;
        if damage == "oversized-config" {
            bytes[168..172].copy_from_slice(&(2_u32 * 1024 * 1024).to_le_bytes());
            bytes.resize(512 + 2 * 1024 * 1024, 0);
        } else {
            bytes = pe(&format!(
                "kernel=kernel init={system}/init\nkernel=kernel init={system}/init\nramdisk=initrd\n",
                system = host.store.system
            ));
        }
        fs::write(path, bytes)?;
        let before = fs::read(host.esp.join("loader/loader.conf"))?;
        assert!(
            !common::run(&mut host.command(&["seal-boot"]), None)?
                .status
                .success()
        );
        assert_eq!(fs::read(host.esp.join("loader/loader.conf"))?, before);
        assert!(
            !fs::read_dir(host.esp.join("loader/entries"))?
                .filter_map(std::result::Result::ok)
                .any(|entry| entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"airlock-"))
        );
    }
    Ok(())
}
