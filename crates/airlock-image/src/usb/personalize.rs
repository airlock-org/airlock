//! Personalization installs a clean source checkout and local credentials offline.

use super::device;
use crate::support::{captured, output, read, reported, root_only, run};
use airlock_nix_core::identity::GitRevision;
use anyhow::{Context, Result, ensure};
use clap::Args;
use rustix::fs::{Gid, Mode, Uid, fchmod, fchown};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    process::Command,
};

#[derive(Args)]
pub(crate) struct Settings {
    // Committed source repository copied without local hooks or Git configuration.
    #[arg(long)]
    repo: PathBuf,
    // Exactly one public SSH key; private keys are rejected.
    #[arg(long)]
    public_key: PathBuf,
    // Optional machine-local AMD firmware table validated before copying.
    #[arg(long)]
    amd_vfct: Option<PathBuf>,
    // Explicit lab convenience; sets the literal password without a prompt.
    #[arg(long)]
    insecure_dev_password: bool,
}

pub(crate) struct Prepared {
    // Private Git copy keeps the selected source available even after USB erasure.
    repository: tempfile::TempDir,
    // Exact committed tree selected before any destructive operation.
    commit: GitRevision,
    // Validated public key bytes; the input file is never reopened during install.
    key: Vec<u8>,
    // Optional validated firmware bytes, independent of their original filesystem.
    firmware: Option<Vec<u8>>,
    // Only the password hash is retained for installation.
    password_hash: Vec<u8>,
}

impl Settings {
    pub(crate) fn prepare(&self) -> Result<Prepared> {
        let firmware = self.amd_vfct.as_deref().map(vfct).transpose()?;
        let mut key = read(&self.public_key, 64 * 1024)?;
        ensure!(
            !key.windows(b"PRIVATE KEY".len())
                .any(|w| w == b"PRIVATE KEY")
                && key
                    .split(|b| *b == b'\n')
                    .filter(|line| !line.is_empty())
                    .count()
                    == 1,
            "Supply exactly one SSH public key, never a private key."
        );

        if !key.ends_with(b"\n") {
            key.push(b'\n');
        }
        let validated = captured(
            Command::new("ssh-keygen").args(["-l", "-f", "/dev/stdin"]),
            Some(key.clone()),
        )?;
        ensure!(
            validated.status.success(),
            "Invalid SSH public key: {}",
            String::from_utf8_lossy(&validated.stderr)
        );

        let repo = fs::canonicalize(&self.repo)?;
        let mut git = Command::new("git");
        git.arg("-c")
            .arg(format!("safe.directory={}", repo.display()))
            .arg("-C")
            .arg(&repo);
        ensure!(
            output(git.arg("status").arg("--porcelain"))?.is_empty(),
            "Commit the repo before copying it."
        );
        let commit = output(
            Command::new("git")
                .arg("-c")
                .arg(format!("safe.directory={}", repo.display()))
                .arg("-C")
                .arg(&repo)
                .args(["rev-parse", "HEAD"]),
        )?;
        let commit: GitRevision = commit.parse()?;

        let repository = tempfile::Builder::new()
            .prefix("airlock-usb-source-")
            .tempdir_in("/run")?;
        reported(
            Command::new("git")
                .arg("-c")
                .arg(format!("safe.directory={}", repo.display()))
                .args(["clone", "--bare", "--no-local", "--"])
                .arg(&repo)
                .arg(repository.path().join("repo.git")),
            None,
            &[0],
        )?;
        output(
            Command::new("git")
                .arg("-C")
                .arg(repository.path().join("repo.git"))
                .args(["cat-file", "-e"])
                .arg(format!("{commit}^{{commit}}")),
        )?;

        let mut password = password(self.insecure_dev_password)?.into_bytes();
        ensure!(
            !password.contains(&b'\n') && !password.contains(&b'\r') && !password.contains(&0),
            "Password contains unsupported control characters"
        );
        password.push(b'\n');
        let hashed = captured(
            Command::new("openssl").args(["passwd", "-6", "-stdin"]),
            Some(password),
        )?;
        ensure!(hashed.status.success(), "Password hashing failed");
        let hash = String::from_utf8(hashed.stdout)?;
        let hash = hash.trim_end_matches('\n');
        ensure!(
            hash.starts_with("$6$")
                && hash.len() <= 256
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"$./".contains(&b)),
            "Invalid password hash output"
        );

        Ok(Prepared {
            repository,
            commit,
            key,
            firmware,
            password_hash: format!("{hash}\n").into_bytes(),
        })
    }
}

pub(crate) fn beneath(root: &Path, relative: &Path) -> Result<PathBuf> {
    let mut current = root.to_owned();
    for part in relative.components() {
        let Component::Normal(part) = part else {
            anyhow::bail!("Invalid personalization path");
        };
        current.push(part);
        ensure!(
            !current.is_symlink(),
            "Unexpected symlink: {}",
            current.display()
        );
    }

    Ok(current)
}

pub(crate) fn private_file(
    path: &Path,
    data: &[u8],
    uid: u32,
    gid: u32,
    firmware: bool,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    fchown(&file, Some(Uid::from_raw(uid)), Some(Gid::from_raw(gid)))?;
    file.write_all(data)?;
    if firmware {
        fchmod(&file, Mode::from_raw_mode(0o444))?;
    }
    file.sync_all()?;
    Ok(())
}

pub(crate) fn vfct(path: &Path) -> Result<Vec<u8>> {
    let data = read(path, 8 * 1024 * 1024)?;
    ensure!(data.len() >= 36, "AMD VFCT source is too short.");
    ensure!(
        &data[..4] == b"VFCT",
        "AMD VFCT source has the wrong ACPI signature."
    );
    ensure!(
        u32::from_le_bytes(data[4..8].try_into()?) as usize == data.len(),
        "AMD VFCT source length does not match its ACPI header."
    );
    ensure!(
        data.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == 0,
        "AMD VFCT source has an invalid ACPI checksum."
    );
    Ok(data)
}

fn prompt(message: &str) -> Result<String> {
    // The upstream terminal helper restores echo on cancellation as well as
    // ordinary failures; the Airlock process never changes terminal settings.
    let result = Command::new("systemd-ask-password")
        .args(["--timeout=0", "--echo=no", "--emoji=no", message])
        .stdin(File::open("/dev/tty")?)
        .stderr(std::process::Stdio::inherit())
        .output()?;
    ensure!(result.status.success(), "Password entry was cancelled");
    let mut value = String::from_utf8(result.stdout)?;
    while value.ends_with(['\n', '\r']) {
        value.pop();
    }
    Ok(value)
}

pub(crate) fn password(insecure: bool) -> Result<String> {
    if insecure {
        println!("DEVELOPMENT MODE: using the literal password for dom0 and sys-gui.");
        return Ok("password".into());
    }

    File::open("/dev/tty").context(
        "Open a terminal for the password prompt, or explicitly use --insecure-dev-password",
    )?;
    let value = prompt("New local dom0 password (sys-gui uses password): ")?;
    ensure!(!value.is_empty(), "Password cannot be empty.");
    ensure!(value == prompt("Confirm password: ")?, "Passwords differ.");
    Ok(value)
}

fn chown_tree(root: &Path) -> Result<()> {
    rustix::fs::chownat(
        rustix::fs::CWD,
        root,
        Some(Uid::from_raw(1000)),
        Some(Gid::from_raw(100)),
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )?;
    if fs::symlink_metadata(root)?.is_dir() {
        for entry in fs::read_dir(root)? {
            chown_tree(&entry?.path())?;
        }
    }
    Ok(())
}

impl Prepared {
    pub(crate) fn install(&self, root: &Path, selected: &device::Disk) -> Result<()> {
        root_only()?;
        let root = fs::canonicalize(root)?;
        ensure!(
            root != Path::new("/"),
            "Personalize offline, before the first USB boot."
        );
        device::check_mount(&root, Some(selected))?;
        let password_file = beneath(&root, Path::new("var/lib/airlock/usb/local-password.hash"))?;
        let repo_target = beneath(&root, Path::new("home/user/dev/airlock"))?;
        let key_target = beneath(&root, Path::new("home/user/.ssh/authorized_keys"))?;
        let vfct_target = beneath(&root, Path::new("var/lib/airlock/amd-lab/sys-gui.vfct"))?;
        ensure!(
            [&password_file, &repo_target, &key_target, &vfct_target]
                .iter()
                .all(|p| !p.exists()),
            "Already personalized (or interrupted); refusing to overwrite data."
        );

        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(password_file.parent().context("password parent")?)?;
        fs::create_dir_all(repo_target.parent().context("repo parent")?)?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(key_target.parent().context("key parent")?)?;

        reported(
            Command::new("git")
                .args(["clone", "--no-local", "--"])
                .arg(self.repository.path().join("repo.git"))
                .arg(&repo_target),
            None,
            &[0],
        )?;

        reported(
            Command::new("git")
                .arg("-C")
                .arg(&repo_target)
                .args(["checkout", "-B", "usb-dev"])
                .arg(self.commit.as_str()),
            None,
            &[0],
        )?;
        run(Command::new("git")
            .arg("-C")
            .arg(&repo_target)
            .args(["remote", "remove", "origin"]))?;

        private_file(&key_target, &self.key, 1000, 100, false)?;
        private_file(&password_file, &self.password_hash, 0, 0, false)?;
        if let Some(data) = &self.firmware {
            fs::create_dir_all(vfct_target.parent().context("firmware parent")?)?;
            private_file(&vfct_target, data, 0, 0, true)?;
            println!(
                "Installed the machine-local AMD VFCT at /var/lib/airlock/amd-lab/sys-gui.vfct."
            );
        }

        chown_tree(&root.join("home/user"))?;
        fs::set_permissions(
            key_target.parent().context("key parent")?,
            fs::Permissions::from_mode(0o700),
        )?;
        println!(
            "Personalized USB; writable repo at /home/user/dev/airlock ({}).",
            self.commit
        );

        Ok(())
    }
}
