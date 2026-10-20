//! Isolate upstream boot-menu generation from dangling or already sealed profile history.
//! Prepared entries publish only after installation succeeds, preserving the current default.

use super::{boot_artifacts, fs::*};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

fn copy_profiles(
    source: &Path,
    target: &Path,
    sealed: &BTreeSet<PathBuf>,
    selected: &Path,
) -> Result<()> {
    mkdir_all(target)?;
    for path in children(source)? {
        let name = path.file_name().context("profile has no name")?;
        if path.is_symlink() && path.try_exists()? {
            let canonical = fs::canonicalize(&path)?;
            if name != "system" && sealed.contains(&canonical) && canonical != selected {
                continue;
            }
            let copied = target.join(name);
            symlink(fs::read_link(&path)?, &copied)?;
            let metadata = fs::symlink_metadata(&path)?;
            use std::os::unix::fs::MetadataExt;
            let timestamps = rustix::fs::Timestamps {
                last_access: rustix::time::Timespec {
                    tv_sec: metadata.atime(),
                    tv_nsec: metadata.atime_nsec(),
                },
                last_modification: rustix::time::Timespec {
                    tv_sec: metadata.mtime(),
                    tv_nsec: metadata.mtime_nsec(),
                },
            };
            rustix::fs::utimensat(
                rustix::fs::CWD,
                &copied,
                &timestamps,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )?;
        } else if path.is_dir() && name == "system-profiles" {
            copy_profiles(&path, &target.join(name), sealed, selected)?;
        }
    }
    Ok(())
}

pub(super) fn publish_loader(staged: &Path, loader: &Path) -> Result<()> {
    let old = fs::read_to_string(loader.join("loader.conf"))?;
    let choices: Vec<_> = old
        .lines()
        .filter(|line| {
            matches!(
                line.split_whitespace().next(),
                Some("default" | "preferred")
            )
        })
        .collect();
    ensure!(
        choices
            .iter()
            .any(|line| line.split_whitespace().next() == Some("default")),
        "boot preparation requires existing default"
    );
    let protected: BTreeSet<_> = choices
        .iter()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|name| name.trim_end_matches(".conf"))
        .collect();
    let entries = loader.join("entries");
    mkdir_all(&entries)?;
    for path in children(&staged.join("entries"))? {
        if path.extension().is_some_and(|s| s == "conf") {
            atomic(
                &entries.join(path.file_name().context("missing entry name")?),
                &fs::read(&path)?,
                0o644,
            )?;
        }
    }
    for path in children(&entries)? {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("invalid entry name")?;
        if path.extension().is_some_and(|s| s == "conf")
            && !staged.join("entries").join(name).try_exists()?
            && !name.starts_with("airlock-")
            && !protected.contains(name.trim_end_matches(".conf"))
        {
            remove(&path)?;
        }
    }
    sync_dir(&entries)?;
    let text = fs::read_to_string(staged.join("loader.conf"))?;
    let lines = choices
        .into_iter()
        .chain(text.lines().filter(|line| {
            !matches!(
                line.split_whitespace().next(),
                Some("default" | "preferred")
            )
        }))
        .collect::<Vec<_>>()
        .join("\n");
    atomic(
        &loader.join("loader.conf"),
        format!("{lines}\n").as_bytes(),
        0o644,
    )
}

pub(super) fn install(system: &str, profiles: &Path, loader: &Path, pool: &Path) -> Result<()> {
    entry(system)?;
    let temporary = tempfile::tempdir()?;
    let copied = temporary.path().join("profiles");
    let mut sealed = BTreeSet::new();
    for path in children(&pool.join("host/boots"))? {
        let record: Value = read(&path)?;
        if record["retired"] == true
            || !pool
                .join("host/artifacts")
                .join(path.file_name().context("missing boot record name")?)
                .try_exists()?
        {
            continue;
        }
        boot_artifacts::verify(pool, loader.parent().context("loader has no ESP")?, &record)?;
        sealed.insert(PathBuf::from(text(&record, "system")?));
    }
    copy_profiles(
        profiles,
        &copied,
        &sealed,
        &fs::canonicalize(profiles.join("system"))?,
    )?;
    ensure!(
        copied.join("system").try_exists()?,
        "selected host profile unavailable"
    );
    let staged = temporary.path().join("loader");
    command(
        "cp",
        &["-a", &loader.to_string_lossy(), &staged.to_string_lossy()],
        None,
    )?;
    command(
        "mount",
        &[
            "--bind",
            &copied.to_string_lossy(),
            &profiles.to_string_lossy(),
        ],
        None,
    )?;
    let result = (|| -> Result<()> {
        command(
            "mount",
            &[
                "--bind",
                &staged.to_string_lossy(),
                &loader.to_string_lossy(),
            ],
            None,
        )?;
        let result = command(
            &format!("{system}/bin/switch-to-configuration"),
            &["boot"],
            None,
        );
        let unmount = command("umount", &[&loader.to_string_lossy()], None);
        result?;
        unmount?;
        Ok(())
    })();
    let unmount = command("umount", &[&profiles.to_string_lossy()], None);
    result?;
    unmount?;
    publish_loader(&staged, loader)
}
