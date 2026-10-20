//! Prepared image stores remain usable without source packages and keep host recovery independent.

use crate::{
    common::{self, store::Store},
    host_store::pe,
    storage_integrity::snapshot,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// Own two prepared system variants sharing one immutable package and a private installed pool.
struct Seed {
    // Supplies inert EROFS indexing and the real native image/storage command.
    store: Store,
    // Authoritative per-variant declarations passed through the native JSON boundary.
    specs: Value,
    // Separately cacheable output describing the sealed pool.
    descriptors: PathBuf,
    // Final pool location, already reflected in generated metadata.
    installed: PathBuf,
}

impl Seed {
    /// Prepare sparse source files and two variants without evaluating or building a Nix system.
    fn new() -> Result<Self> {
        let store = Store::new()?;
        let root = store.temporary.path();
        let installed = root.join("installed-pool");
        let descriptors = root.join("descriptors");
        let root_source = format!("/nix/store/{hash}-root/root.img", hash = "d".repeat(32));
        let root_image = store.physical.join("d".repeat(32) + "-root/root.img");
        fs::create_dir_all(root_image.parent().unwrap())?;
        fs::copy(&store.root, &root_image)?;
        let mut specs = Vec::new();
        for (index, hash) in ["b", "c"].into_iter().enumerate() {
            let system = format!("/nix/store/{hash}-system", hash = hash.repeat(32));
            let package = store.physical.join(system.rsplit('/').next().unwrap());
            fs::create_dir(&package)?;
            fs::write(package.join("payload"), system.repeat(20))?;
            let closure = root.join(format!("closure-{index}"));
            fs::create_dir(&closure)?;
            fs::write(closure.join("manifest.json"), "{}")?;
            fs::write(
                closure.join("store-paths"),
                format!("{common}\n{system}\n", common = store.system),
            )?;
            specs.push(
                json!({"name": format!("variant-{index}"), "domain": "sys-gui", "system": system,
                "root_source": root_source, "kernel_name": "test-kernel", "closure_source": closure,
                "physical_store_root": store.physical}),
            );
        }
        let seed = Self {
            store,
            specs: json!(specs),
            descriptors,
            installed,
        };
        seed.prepare(&seed.store.pool, &seed.descriptors)?;
        Ok(seed)
    }

    /// Produce both outputs with the same logical runtime location.
    fn prepare(&self, pool: &Path, descriptors: &Path) -> Result<()> {
        let path = self.store.temporary.path().join("specs.json");
        fs::write(&path, serde_json::to_vec(&self.specs)?)?;
        common::run(
            self.store
                .command(&["image", "prepare-pool"])
                .arg(pool)
                .arg(descriptors)
                .arg(path)
                .arg(&self.installed),
            None,
        )?
        .checked()?;
        Ok(())
    }

    /// Install the sealed pool and remove all source packages before boot-adoption assertions.
    fn install(&self) -> Result<()> {
        fs::rename(&self.store.pool, &self.installed)?;
        fs::remove_dir_all(&self.store.physical)?;
        Ok(())
    }

    /// Bind runtime adoption to the selected variant's exact identity and devices.
    fn request(&self, index: usize) -> Value {
        let spec = &self.specs[index];
        json!({"artifact": self.descriptors.join(format!("variant-{index}")),
            "source": self.descriptors.join(format!("variant-{index}")), "pool_root": self.installed,
            "domain": "sys-gui", "system": spec["system"], "root": spec["root_source"], "kernel": "test-kernel",
            "device": "/dev/xvde", "blob_device": "/dev/xvdf", "mountpoint": "/nix/store", "view_backend": "erofs-objectmap"})
    }

    /// Use the real prepared descriptor verifier and keep rejection diagnostics for corruption cases.
    fn verify(&self, request: &Value) -> Result<airlock_vm_service::support::Output> {
        common::run(
            &mut self
                .store
                .command(&["image", "verify-prepared", "--request", "-"]),
            Some(serde_json::to_vec(request)?),
        )
    }

    /// Fully validate stored bytes independently from the cheaper trusted-image boot-adoption check.
    fn outputs(&self) -> Result<airlock_vm_service::support::Output> {
        common::run(
            self.store
                .command(&["image", "verify-outputs"])
                .arg(&self.installed)
                .arg(&self.descriptors),
            None,
        )
    }
}

/// Rebuilding deterministic outputs can reuse either cache half despite changed source timestamps.
#[test]
fn prepared_image_outputs_are_independently_reusable() -> Result<()> {
    let seed = Seed::new()?;
    let old_pool = seed.store.temporary.path().join("cached-pool");
    let old_descriptors = seed.store.temporary.path().join("cached-descriptors");
    fs::rename(&seed.store.pool, &old_pool)?;
    fs::rename(&seed.descriptors, &old_descriptors)?;
    let original = snapshot(&seed.store.physical)?;
    for path in original.keys() {
        File::options()
            .write(true)
            .open(seed.store.physical.join(path))?
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(3))?;
    }
    seed.prepare(&seed.store.pool, &seed.descriptors)?;
    let content = |root: &Path| -> Result<BTreeMap<PathBuf, Vec<u8>>> {
        Ok(snapshot(root)?
            .into_iter()
            .map(|(path, (bytes, _))| (path, bytes))
            .collect())
    };
    assert_eq!(content(&old_pool)?, content(&seed.store.pool)?);
    assert_eq!(content(&old_descriptors)?, content(&seed.descriptors)?);
    fs::write(
        old_descriptors.join("cached-validation.json"),
        "{\"status\":\"passed\"}",
    )?;
    for (pool, descriptors) in [
        (&seed.store.pool, &old_descriptors),
        (&old_pool, &seed.descriptors),
    ] {
        let result = common::json(
            seed.store
                .command(&["image", "verify-outputs"])
                .arg(pool)
                .arg(descriptors),
            None,
        )?;
        assert_eq!(result["status"], "passed");
    }
    Ok(())
}

/// Both sealed variants adopt without original packages, repacking or mutating stored blobs.
#[test]
fn prepared_image_adoption_needs_no_source_or_repacking() -> Result<()> {
    let seed = Seed::new()?;
    seed.install()?;
    let before: BTreeMap<_, _> = snapshot(&seed.installed)?
        .into_iter()
        .filter(|(path, _)| {
            path.extension()
                .is_some_and(|extension| extension == "blob")
        })
        .collect();
    for index in 0..2 {
        let result: Value = serde_json::from_slice(&seed.verify(&seed.request(index))?.checked()?)?;
        assert_eq!(result, json!(index + 1));
    }
    assert_eq!(
        fs::read_dir(seed.installed.join("objects/metadata"))?.count(),
        3
    );
    let after: BTreeMap<_, _> = snapshot(&seed.installed)?
        .into_iter()
        .filter(|(path, _)| {
            path.extension()
                .is_some_and(|extension| extension == "blob")
        })
        .collect();
    assert_eq!(before, after);
    Ok(())
}

/// Boot-adoption checks metadata and identity, while complete validation additionally authenticates trusted image payload bytes.
#[test]
fn prepared_image_rejects_corruption_at_the_correct_boundary() -> Result<()> {
    for damage in [
        "truncated",
        "metadata",
        "missing",
        "identity",
        "escape",
        "manifest",
        "payload",
    ] {
        let seed = Seed::new()?;
        seed.install()?;
        let mut request = seed.request(0);
        let files = snapshot(&seed.installed)?;
        let blob = seed.installed.join(
            files
                .keys()
                .find(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "blob")
                })
                .unwrap(),
        );
        match damage {
            "truncated" | "payload" => {
                fs::set_permissions(&blob, fs::Permissions::from_mode(0o600))?;
                let mut bytes = fs::read(&blob)?;
                if damage == "truncated" {
                    bytes.pop();
                } else {
                    bytes[0] ^= 1;
                }
                fs::write(blob, bytes)?;
            }
            "metadata" => {
                let descriptor: Value = serde_json::from_slice(&fs::read(
                    seed.descriptors.join("variant-0/prepared.json"),
                )?)?;
                let relative = descriptor["files"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .find(|path| path.ends_with("store.blob-map.json"))
                    .unwrap();
                fs::write(seed.installed.join(relative), "{}")?;
            }
            "missing" => fs::remove_file(blob)?,
            "identity" => {
                request["system"] = json!("/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-wrong")
            }
            "escape" => {
                let path = seed.descriptors.join("variant-0/prepared.json");
                let mut descriptor: Value = serde_json::from_slice(&fs::read(&path)?)?;
                descriptor["files"]["../outside"] = json!({"size": 0, "sha256": ""});
                fs::write(path, serde_json::to_vec(&descriptor)?)?;
            }
            _ => {
                let path = seed
                    .installed
                    .join("domains/sys-gui/generations/1/manifest.json");
                let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
                manifest["transaction"] = json!("different-build-transaction");
                fs::write(path, serde_json::to_vec(&manifest)?)?;
            }
        }
        if damage == "payload" {
            assert!(seed.verify(&request)?.status.success());
            assert!(!seed.outputs()?.status.success());
        } else {
            assert!(!seed.verify(&request)?.status.success(), "{damage}");
        }
    }
    let store = Store::new()?;
    let result = common::json(
        &mut store.command(&["image", "verify-prepared", "--request", "-"]),
        Some(serde_json::to_vec(
            &json!({"artifact": store.temporary.path()}),
        )?),
    )?;
    assert!(result.is_null());
    Ok(())
}

/// Failed host preparation reopens only copied lock files and never changes the immutable seed.
#[test]
fn failed_host_image_preparation_keeps_input_immutable() -> Result<()> {
    let store = Store::new()?;
    let root = store.temporary.path();
    let seed = root.join("seed");
    let pool = seed.join("pool");
    fs::create_dir_all(pool.join("objects"))?;
    let names = [
        "admission.lock",
        "objects/segments.lock",
        "objects/sealed.blob",
        "index.erofs",
    ];
    for name in names {
        fs::write(pool.join(name), "seed")?;
        fs::set_permissions(pool.join(name), fs::Permissions::from_mode(0o444))?;
    }
    fs::set_permissions(pool.join("objects"), fs::Permissions::from_mode(0o555))?;
    fs::set_permissions(&pool, fs::Permissions::from_mode(0o555))?;
    fs::write(root.join("registration"), "invalid registration")?;
    fs::write(root.join("choices.json"), "{}")?;
    let output = root.join("output");
    let result = common::run(
        store
            .command(&["image", "prepare-host"])
            .arg(seed)
            .arg(&output)
            .arg(root.join("registration"))
            .arg(&store.system)
            .arg(root.join("choices.json")),
        None,
    )?;
    assert!(!result.status.success());
    for name in names {
        let path = output.join("pool").join(name);
        let lock = name.ends_with(".lock");
        assert_eq!(
            path.metadata()?.permissions().mode() & 0o777,
            if lock { 0o600 } else { 0o444 }
        );
        if lock {
            rustix::fs::flock(
                &File::options().read(true).write(true).open(path)?,
                rustix::fs::FlockOperation::NonBlockingLockExclusive,
            )?;
        } else {
            assert_eq!(fs::read(path)?, b"seed");
        }
        assert_eq!(
            pool.join(name).metadata()?.permissions().mode() & 0o777,
            0o444
        );
    }
    Ok(())
}

/// Removing the bootstrap store keeps only an independent, verified recovery closure and refuses unpublished or misbound entries.
#[test]
fn bootstrap_store_removal_preserves_only_verified_recovery() -> Result<()> {
    for damage in ["", "unpublished", "wrong-system"] {
        let store = Store::new()?;
        let root = store.temporary.path().join("image");
        let host = "/nix/store/00000000000000000000000000000000-host";
        let recovery = "/nix/store/11111111111111111111111111111111-recovery";
        for logical in [host, recovery] {
            let path = root.join(logical.trim_start_matches('/'));
            fs::create_dir_all(&path)?;
            fs::write(path.join("payload"), logical)?;
        }
        let pool = root.join("var/lib/airlock/closure-store");
        fs::create_dir_all(pool.join("host/boots"))?;
        fs::write(
            pool.join("generation.json"),
            serde_json::to_vec(&json!({"store": {"paths": [host, recovery]}}))?,
        )?;
        fs::write(
            pool.join("host/boots/host.json"),
            serde_json::to_vec(
                &json!({"manifest": "/var/lib/airlock/closure-store/generation.json", "system": host, "generation": 1}),
            )?,
        )?;
        let marker = root
            .join(recovery.trim_start_matches('/'))
            .join("etc/airlock");
        fs::create_dir_all(&marker)?;
        fs::write(marker.join("boot-role"), "recovery\n")?;
        fs::create_dir_all(root.join("boot/loader/entries"))?;
        fs::create_dir_all(root.join("boot/EFI/nixos"))?;
        fs::write(
            root.join("boot/loader/entries/xen-0123456789abcdef.conf"),
            "title Airlock (recovery)\nefi /EFI/nixos/xen-0123456789abcdef.efi\n",
        )?;
        let system = if damage == "wrong-system" {
            host
        } else {
            recovery
        };
        fs::write(
            root.join("boot/EFI/nixos/xen-0123456789abcdef.efi"),
            pe(&format!(
                "kernel=kernel init={system}/init\nramdisk=initrd\n"
            )),
        )?;
        for name in ["kernel", "initrd"] {
            fs::write(root.join("boot/EFI/nixos").join(name), "boot artifact")?;
        }
        if damage == "unpublished" {
            fs::create_dir(root.join("nix/store/unpublished"))?;
        }
        let paths = root.join("recovery-paths");
        fs::write(&paths, format!("{recovery}\n"))?;
        let result = common::run(
            store
                .command(&["image", "install-host"])
                .arg(&root)
                .arg(paths)
                .arg(recovery),
            None,
        )?;
        if damage.is_empty() {
            result.checked()?;
            assert_eq!(fs::read_dir(root.join("nix/store"))?.count(), 0);
            let retained = root.join("var/lib/airlock/recovery-store");
            assert_eq!(fs::read_dir(&retained)?.count(), 1);
            assert!(
                retained
                    .join(Path::new(recovery).file_name().unwrap())
                    .is_dir()
            );
        } else {
            assert!(!result.status.success());
            assert!(root.join(host.trim_start_matches('/')).exists());
            assert!(root.join(recovery.trim_start_matches('/')).exists());
        }
    }
    Ok(())
}

/// Execute the actual image-validation hook prefix against a valid pool, then corrupt metadata despite a cached success receipt.
pub(super) fn validation_hook(pre_vm: &str, check: &str) -> Result<()> {
    let seed = Seed::new()?;
    seed.install()?;
    let (prefix, remainder) = pre_vm
        .split_once(check)
        .ok_or_else(|| anyhow::anyhow!("image validation command missing from build hook"))?;
    assert!(!remainder.trim().is_empty());
    let quote = airlock_tests::process::quote;
    let checker = format!(
        "{service} storage image verify-outputs {pool} {descriptors}",
        service = quote(seed.store.service.to_str().unwrap()),
        pool = quote(seed.installed.to_str().unwrap()),
        descriptors = quote(seed.descriptors.to_str().unwrap())
    );
    let script = format!("{prefix}{checker}\necho ASSEMBLY_REACHED\n");
    let execute = || -> Result<airlock_vm_service::support::Output> {
        common::run(
            std::process::Command::new("bash").args(["-e", "-c", &script]),
            None,
        )
    };
    let success = execute()?.checked()?;
    assert!(String::from_utf8_lossy(&success).contains("ASSEMBLY_REACHED"));
    fs::write(
        seed.descriptors.join("cached-validation.json"),
        "{\"status\":\"passed\"}",
    )?;
    let metadata = fs::read_dir(seed.installed.join("objects/blob-metadata"))?
        .next()
        .unwrap()?
        .path();
    fs::write(metadata, "{}")?;
    let failed = execute()?;
    assert!(!failed.status.success());
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("ASSEMBLY_REACHED"));
    Ok(())
}

/// A prepared upstream-Qubes Python runtime package is exercised as a real offline API boundary, with Rust owning image assertions.
#[test]
#[ignore = "explicitly prepare AIRLOCK_PREPARE_RUNTIME_PACKAGE with upstream Qubes and ext4 tools"]
fn external_tool_packaged_runtime_uses_upstream_qubes_offline_api() -> Result<()> {
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::MetadataExt,
        process::Command,
    };
    let executable = std::env::var_os("AIRLOCK_PREPARE_RUNTIME_PACKAGE").ok_or_else(|| {
        anyhow::anyhow!("set AIRLOCK_PREPARE_RUNTIME_PACKAGE to the prepared runtime wrapper")
    })?;
    for (role, label, color) in [
        ("sys-gui", "red", None),
        ("sys-gui", "khaki", Some("0x6b7a3d")),
        ("sys-ctrl", "light-gray", Some("0xd3d3d3")),
    ] {
        let seed = Seed::new()?;
        let root = seed.store.temporary.path();
        let image = root.join("runtime-root.img");
        let mut file = File::create(&image)?;
        file.set_len(16 * 1024 * 1024)?;
        file.seek(SeekFrom::Start(0))?;
        file.flush()?;
        common::run(
            Command::new(common::tool("mkfs.ext4")?)
                .args(["-q", "-F"])
                .arg(&image),
            None,
        )?
        .checked()?;
        let mut spec = seed.specs[0].clone();
        spec["domain"] = json!(role);
        spec["root_source"] = json!(image);
        let mut selected = vec![spec.clone()];
        if role == "sys-ctrl" {
            let closure = PathBuf::from(spec["closure_source"].as_str().unwrap());
            let registration = fs::read_to_string(closure.join("store-paths"))?
                .lines()
                .map(|path| format!("{path}\n{hash}\n42\n\n0\n", hash = "0".repeat(64)))
                .collect::<String>();
            fs::write(closure.join("registration"), registration)?;
            let mut builder = spec.clone();
            for (key, value) in [
                ("name", json!("builder")),
                ("domain", json!("sys-ctrl-builder")),
                ("root_source", json!("")),
                ("kernel_name", json!("")),
                ("builder", json!(true)),
            ] {
                builder[key] = value;
            }
            selected.push(builder);
        }
        let specs = root.join("runtime-specs.json");
        fs::write(&specs, serde_json::to_vec(&selected)?)?;
        let pool = root.join("runtime-pool");
        let descriptors = root.join("runtime-descriptors");
        common::run(
            seed.store
                .command(&["image", "prepare-pool"])
                .arg(&pool)
                .arg(&descriptors)
                .arg(specs)
                .arg("/var/lib/airlock/closure-store"),
            None,
        )?
        .checked()?;
        let kernel = root.join("kernel");
        fs::create_dir(&kernel)?;
        for name in ["vmlinuz", "modules.img"] {
            fs::write(kernel.join(name), "fixture")?;
        }
        let mut domain = json!({"class": "StandaloneVM", "label": label, "netvm": null,
            "prefs": {"autostart": false, "guivm": null, "virt_mode": "hvm"}, "features": {"qrexec": true},
            "services": {"test-service": false}, "devices": {"pci": [{"backend": "dom0", "id": "00_08.1"}]},
            "backend": {"type": "nix-closure", "system": {"source": spec["system"]}, "root": {"source": image}, "kernel": {"name": "test-kernel"},
                "state": {"persistent_size": 32 * 1024 * 1024}, "store": {"source": descriptors.join(spec["name"].as_str().unwrap())}}});
        if let Some(color) = color {
            domain["features"]["airlock-label-color"] = json!(color);
        }
        let topology = json!({"kernels": {"test-kernel": {"source": kernel}}, "prefs": {"default_netvm": null}, "domains": {(role): domain}});
        let config = root.join("topology.json");
        fs::write(&config, serde_json::to_vec(&topology)?)?;
        let output = root.join("runtime");
        common::run(
            Command::new(&executable)
                .arg(&config)
                .arg(&pool)
                .arg(&output),
            None,
        )?
        .checked()?;
        let xml = fs::read_to_string(output.join("qubes/qubes.xml"))?;
        // Input filenames also start with "runtime"; only the staging Qubes root must disappear.
        assert!(!xml.contains(output.join("qubes").to_str().unwrap()));
        assert!(xml.contains("/var/lib/qubes"));
        assert!(xml.contains("airlock-root-source") && xml.contains(image.to_str().unwrap()));
        assert!(xml.contains("airlock-closure") && xml.contains("erofs-blob-map"));
        if let Some(color) = color {
            assert!(xml.contains(color));
        }
        if role == "sys-ctrl" {
            assert!(xml.contains("builder-store") && xml.contains("builder-data"));
        }
        // The runtime adapter must leave physical PCI assignment for boot-time authority.
        let mut reader = quick_xml::Reader::from_str(&xml);
        loop {
            match reader.read_event()? {
                quick_xml::events::Event::Start(element)
                | quick_xml::events::Event::Empty(element) => {
                    assert_ne!(element.name().as_ref(), b"device")
                }
                quick_xml::events::Event::Eof => break,
                _ => {}
            }
        }
        let installed = output.join(format!("qubes/appvms/{role}/root.img"));
        assert_eq!(fs::read(&installed)?, fs::read(&image)?);
        assert!(installed.metadata()?.blocks() * 512 < installed.metadata()?.len());
        assert_eq!(
            output
                .join(format!("qubes/appvms/{role}/private.img"))
                .metadata()?
                .len(),
            32 * 1024 * 1024
        );
        assert_eq!(
            fs::read_link(output.join(format!("pool-selections/domains/{role}/current")))?,
            Path::new("generations/1")
        );
        let again = common::run(
            Command::new(&executable).arg(config).arg(pool).arg(output),
            None,
        )?;
        assert!(!again.status.success());
        assert!(String::from_utf8_lossy(&again.stderr).contains("must not already exist"));
    }
    Ok(())
}
