//! Boot the packaged static init in a disposable Linux VM with a real split-device EROFS store.
//! Tiny activation/init fixtures observe the handoff without building a complete Qubes machine.

use super::*;
use std::{collections::BTreeMap, io::Write, os::unix::fs::PermissionsExt};

/// Require an explicitly selected kernel or module tree rather than provisioning one inside a test.
fn supplied(variable: &str) -> Result<PathBuf> {
    let path =
        PathBuf::from(std::env::var_os(variable).with_context(|| format!("set {variable}"))?);
    ensure!(path.exists(), "missing {variable}: {path:?}");
    Ok(path)
}

/// Write a shell fixture whose interpreter is the boot image's independent static BusyBox.
fn script(path: &Path, body: &str) -> Result<()> {
    fs::write(path, body)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Copy only the supplied kernel's required modules, decompressing them for the image's modprobe.
fn modules(root: &Path, source: &Path) -> Result<String> {
    let versions =
        fs::read_dir(source.join("lib/modules"))?.collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        versions.len() == 1,
        "select exactly one kernel module version"
    );
    let version = versions[0]
        .file_name()
        .into_string()
        .map_err(|_| anyhow::anyhow!("module version must be UTF-8"))?;
    let source = versions[0].path();
    let destination = root.join("lib/modules").join(&version);
    fs::create_dir_all(&destination)?;
    let mut needed = BTreeSet::new();
    for name in ["virtio_pci", "virtio_blk", "erofs"] {
        let output = common::run(
            Command::new(common::tool("modprobe")?)
                .arg("--show-depends")
                .arg("--dirname")
                .arg(source.ancestors().nth(3).context("module tree root")?)
                .arg("--set-version")
                .arg(&version)
                .arg(name),
            None,
        )?
        .checked()?;
        for line in std::str::from_utf8(&output)?.lines() {
            if let Some(path) = line
                .strip_prefix("insmod ")
                .and_then(|line| line.split_whitespace().next())
            {
                needed.insert(PathBuf::from(path));
            }
        }
    }

    for module in needed {
        let relative = module.strip_prefix(&source)?;
        let mut target = destination.join(relative);
        let bytes = if target.extension().is_some_and(|value| value == "xz") {
            target.set_extension("");
            common::run(
                Command::new(common::tool("xz")?)
                    .args(["--decompress", "--stdout"])
                    .arg(&module),
                None,
            )?
            .checked()?
        } else {
            fs::read(&module)?
        };
        fs::create_dir_all(target.parent().context("kernel module parent")?)?;
        fs::write(target, bytes)?;
    }
    for metadata in [
        "modules.builtin",
        "modules.builtin.modinfo",
        "modules.order",
    ] {
        let path = source.join(metadata);
        if path.is_file() {
            fs::copy(path, destination.join(metadata))?;
        }
    }
    common::run(
        Command::new(common::tool("depmod")?)
            .arg("--basedir")
            .arg(root)
            .arg(&version),
        None,
    )?
    .checked()?;
    Ok(version)
}

/// List initramfs entries without following symlinks or including host filesystem contents.
fn entries(root: &Path, directory: &Path, names: &mut Vec<u8>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        names.extend_from_slice(path.strip_prefix(root)?.as_os_str().as_encoded_bytes());
        names.push(0);
        if entry.file_type()?.is_dir() {
            entries(root, &path, names)?;
        }
    }
    Ok(())
}

/// Package the extracted boot filesystem as an initramfs so no root-disk driver precedes our init.
fn initramfs(root: &Path, output: &Path) -> Result<()> {
    let mut names = Vec::new();
    entries(root, root, &mut names)?;
    let mut input = tempfile::tempfile()?;
    input.write_all(&names)?;
    use std::io::{Seek, SeekFrom};
    input.seek(SeekFrom::Start(0))?;

    let status = Command::new(common::tool("cpio")?)
        .args([
            "--create",
            "--format=newc",
            "--owner=0:0",
            "--null",
            "--quiet",
        ])
        .current_dir(root)
        .stdin(input)
        .stdout(fs::File::create(output)?)
        .status()?;
    ensure!(status.success(), "initramfs archive failed: {status}");
    Ok(())
}

/// Index a real tar-backed EROFS store whose activation and init expose the two handoff outcomes.
fn store(
    directory: &Path,
    mkfs: &Path,
    system_name: &str,
    fail: bool,
) -> Result<(PathBuf, PathBuf)> {
    let system = directory.join(if fail { "failing-system" } else { "system" });
    fs::create_dir(&system)?;
    script(
        &system.join("activate"),
        if fail {
            r#"#!/bin/busybox sh
exit 7
"#
        } else {
            r#"#!/bin/busybox sh
set -eu
printf 'activated' > /run/activated
"#
        },
    )?;
    script(
        &system.join("init"),
        r#"#!/bin/busybox sh
set -eu
test "$$" = 1
test "$1" = forwarded-init-argument
test "$(/bin/busybox cat /run/activated)" = activated
printf 'AIRLOCK_VM_INIT_OK pid=%s argv=%s\n' "$$" "$1"
/bin/busybox poweroff -f
"#,
    )?;

    let stem = if fail { "rejected" } else { "accepted" };
    let blob = directory.join(format!("{stem}.tar"));
    let mut archive = tar::Builder::new(fs::File::create(&blob)?);
    archive.append_dir_all(system_name, &system)?;
    let file = archive.into_inner()?;
    file.set_len(file.metadata()?.len().div_ceil(4096) * 4096)?;
    let metadata = directory.join(format!("{stem}.erofs"));
    common::run(
        Command::new(mkfs).arg("--tar=i").arg(&metadata).arg(&blob),
        None,
    )?
    .checked()?;
    Ok((metadata, blob))
}

/// Real mounts and PID 1 survive a successful handoff; failed activation cannot run the selected init.
#[test]
#[ignore = "explicit packaged root image, AIRLOCK_VM_INIT_BOOT_KERNEL, AIRLOCK_VM_INIT_BOOT_MODULES and AIRLOCK_EROFS_MKFS"]
fn package_vm_init_boots_split_erofs_and_gates_handoff() -> Result<()> {
    let kernel = supplied("AIRLOCK_VM_INIT_BOOT_KERNEL")?;
    let kernel_modules = supplied("AIRLOCK_VM_INIT_BOOT_MODULES")?;
    let mkfs = supplied("AIRLOCK_EROFS_MKFS")?;
    let packages = Packages::new("early-init")?;
    let logical = packages.build("root-build", "airlock-nix-closure-root-image-test")?;
    let output = packages.physical(&logical)?;
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path();
    let root = directory.join("root");
    fs::create_dir(&root)?;
    common::run(
        Command::new(common::tool("debugfs")?)
            .args(["-R", &format!("rdump / {root}", root = root.display())])
            .arg(output.join("root.img")),
        None,
    )?
    .checked()?;
    modules(&root, &kernel_modules)?;
    script(
        &root.join("init"),
        r#"#!/bin/busybox sh
set -eu
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sysfs /sys
/bin/busybox mount -t devtmpfs devtmpfs /dev
/sbin/modprobe virtio_pci
/sbin/modprobe virtio_blk
exec /sbin/init "$@"
"#,
    )?;
    let ramdisk = directory.join("initrd");
    initramfs(&root, &ramdisk)?;

    let system_name = "00000000000000000000000000000000-vm-init-test";
    let mut outcomes = BTreeMap::new();
    for fail in [false, true] {
        let (metadata, blob) = store(directory, &mkfs, system_name, fail)?;
        let log_name = if fail {
            "activation-failure"
        } else {
            "handoff"
        };
        let logs = packages.logs.join(log_name);
        let mut command = Command::new(common::tool("qemu-system-x86_64")?);
        command
            .args([
                "-accel",
                "kvm",
                "-m",
                "256",
                "-display",
                "none",
                "-serial",
                "stdio",
                "-monitor",
                "none",
                "-no-reboot",
            ])
            .arg("-kernel")
            .arg(&kernel)
            .arg("-initrd")
            .arg(&ramdisk)
            .arg("-append")
            .arg(format!(
                "console=ttyS0 panic=1 airlock.system=/nix/store/{system_name} \
                 airlock.storedev=/dev/vda airlock.storeblobdev=/dev/vdb -- forwarded-init-argument"
            ));
        for disk in [metadata, blob] {
            command.arg("-drive").arg(format!(
                "file={disk},format=raw,if=virtio,readonly=on",
                disk = disk.display()
            ));
        }
        system::run(&mut command, &logs, Duration::from_secs(120))?;
        let console = fs::read_to_string(logs.join("stdout.log"))?;
        ensure!(
            console.contains("mounted read-only Nix store from /dev/vda"),
            "store was not mounted: {console}"
        );
        if fail {
            ensure!(
                console.contains("activation failed with exit status: 7"),
                "activation failure was not observed: {console}"
            );
            ensure!(
                !console.contains("AIRLOCK_VM_INIT_OK"),
                "init ran after failed activation"
            );
        } else {
            ensure!(
                console.contains("AIRLOCK_VM_INIT_OK pid=1 argv=forwarded-init-argument"),
                "init handoff failed: {console}"
            );
        }
        outcomes.insert(log_name, json!({"store_mounted":true, "init_ran":!fail}));
    }
    fs::write(
        packages.logs.join("boot.json"),
        serde_json::to_vec_pretty(&outcomes)?,
    )?;
    Ok(())
}
