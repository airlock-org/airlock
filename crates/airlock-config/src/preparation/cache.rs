//! Retain prepared source used by builds or named evidence, and prune disposable snapshots.
//! Locks and pins live beside artifacts so they never change the content-addressed trees.

use super::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions, TryLockError},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// A kernel-held shared lock protecting one prepared tree until this value is dropped.
/// Keep it alive for the whole operation consuming the tree. Process exit releases the lock.
#[derive(Debug)]
pub struct Lease {
    /// Open descriptor whose shared lock prevents pruning this artifact.
    _lock: File,
}

/// One disposable artifact selected by a cache pruning operation.
#[derive(Debug, Serialize)]
pub struct PruneEntry {
    /// Content-addressed directory selected for removal.
    pub path: PathBuf,
    /// Regular-file and symlink bytes that removal would reclaim, before filesystem sharing.
    pub bytes: u64,
}

/// A dry-run or applied retention decision; unknown cache contents are never removed.
#[derive(Debug, Serialize)]
pub struct PruneReport {
    /// Old disposable trees selected for removal, or removed when `applied` is true.
    pub candidates: Vec<PruneEntry>,
    /// Trees protected by a named pin, an active lease, or an unrecognized receipt.
    pub protected: Vec<PathBuf>,
    /// Disposable trees retained by recency or a grace period.
    pub retained: Vec<PathBuf>,
    /// Whether candidate directories were removed rather than merely reported.
    pub applied: bool,
}

/// Open stable coordination metadata without truncating a lock held by another process.
fn lock_file(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?)
}

/// Serialize publication and retention changes while allowing concurrent preparation and use.
pub(super) fn coordination(cache: &Path, exclusive: bool) -> Result<File> {
    fs::create_dir_all(cache.join(".locks"))?;
    let lock = lock_file(&cache.join(".cache.lock"))?;
    if exclusive {
        lock.lock()?;
    } else {
        lock.lock_shared()?;
    }

    Ok(lock)
}

/// Accept only the lowercase SHA-256 directory names emitted by source preparation.
fn identity(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

/// Verify ownership by matching the receipt to its directory name before managing a tree.
fn managed(tree: &Path) -> Result<bool> {
    let Some(name) = tree.file_name().and_then(|name| name.to_str()) else {
        return Ok(false);
    };
    if !identity(name) || !fs::symlink_metadata(tree)?.is_dir() {
        return Ok(false);
    }

    let receipt = tree.join("airlock-prepared.json");
    if !receipt.is_file() || fs::symlink_metadata(&receipt)?.file_type().is_symlink() {
        return Ok(false);
    }

    let bytes = fs::read(receipt)?;
    let hash = format!("{hash:x}", hash = Sha256::digest(&bytes));
    if hash != name {
        return Ok(false);
    }

    let Ok(data) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(false);
    };
    Ok(data["schema"] == "airlock.prepared-source.v1")
}

/// Acquire protection for an already published tree, atomically with respect to pruning.
/// Unknown trees and paths outside this cache are rejected. Explicit users of returned
/// preparation paths should acquire a lease, or pin the tree when retaining it between processes.
pub fn lease(cache: &Path, tree: &Path) -> Result<Lease> {
    let cache = fs::canonicalize(cache)?;
    let _coordination = coordination(&cache, false)?;
    let tree = fs::canonicalize(tree)?;
    if tree.parent() != Some(cache.as_path()) || !managed(&tree)? {
        return Err("prepared tree does not belong to this cache".into());
    }

    let name = tree.file_name().ok_or("artifact lacks identity")?;
    let lock = lock_file(&cache.join(".locks").join(name))?;
    lock.lock_shared()?;
    lock.set_modified(SystemTime::now())?;

    Ok(Lease { _lock: lock })
}

/// Validate pin names as single, readable filesystem components.
fn pin_path(cache: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("pin names must contain 1–64 letters, digits, hyphens or underscores".into());
    }

    Ok(cache.join(".pins").join(name))
}

/// Retain one tree under a durable name, such as a deployment request or validation run.
/// Reusing a name replaces that pin atomically. Pins never modify the retained artifact.
pub fn pin(cache: &Path, tree: &Path, name: &str) -> Result<()> {
    let cache = fs::canonicalize(cache)?;
    let _coordination = coordination(&cache, true)?;
    let tree = fs::canonicalize(tree)?;
    if tree.parent() != Some(cache.as_path()) || !managed(&tree)? {
        return Err("prepared tree does not belong to this cache".into());
    }

    let path = pin_path(&cache, name)?;
    fs::create_dir_all(cache.join(".pins"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(cache.join(".pins"))?;

    writeln!(
        temporary,
        "{identity}",
        identity = tree.file_name().unwrap().to_string_lossy()
    )?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    File::open(cache.join(".pins"))?.sync_all()?;
    Ok(())
}

/// Release a named retention pin without removing any artifact.
pub fn unpin(cache: &Path, name: &str) -> Result<()> {
    let _coordination = coordination(cache, true)?;
    fs::remove_file(pin_path(cache, name)?)?;
    File::open(cache.join(".pins"))?.sync_all()?;
    Ok(())
}

/// Read durable retention roots, rejecting malformed pin metadata before any deletion.
fn pinned(cache: &Path) -> Result<BTreeSet<String>> {
    let directory = cache.join(".pins");
    if !directory.exists() {
        return Ok(BTreeSet::new());
    }

    let mut names = BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let text = fs::read_to_string(entry.path())?;
        let name = text.trim();
        if !identity(name) {
            return Err("invalid prepared artifact pin".into());
        }

        names.insert(name.to_owned());
    }

    Ok(names)
}

/// Count removable bytes without following source symlinks outside the artifact.
fn size(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Ok(metadata.len());
    }

    fs::read_dir(path)?.try_fold(0, |total, entry| Ok(total + size(&entry?.path())?))
}

/// Preview or remove old disposable snapshots, retaining `keep` most recently used trees.
/// Named pins and active leases are always protected in addition to that count.
/// Only receipt-identified directories are eligible; build stores and unrelated files
/// are untouched. This explicit operation has no grace period; automatic preparation
/// cleanup additionally retains trees used within the last 24 hours.
pub fn prune(cache: &Path, keep: usize, apply: bool) -> Result<PruneReport> {
    let cache = fs::canonicalize(cache)?;
    let _coordination = coordination(&cache, true)?;
    prune_locked(&cache, keep, apply, Duration::ZERO)
}

/// Attempt cleanup without delaying preparation behind another cache operation.
/// The caller holds a lease on the just-published tree and has released coordination locks.
pub(super) fn automatic(cache: &Path) {
    if std::env::var("AIRLOCK_PREPARED_AUTO_PRUNE").as_deref() == Ok("0") {
        return;
    }

    let attempt = || -> Result<()> {
        let coordination = lock_file(&cache.join(".cache.lock"))?;
        match coordination.try_lock() {
            Ok(()) => {
                prune_locked(cache, 3, true, Duration::from_secs(24 * 60 * 60))?;
                Ok(())
            }
            Err(TryLockError::WouldBlock) => Ok(()),
            Err(TryLockError::Error(error)) => Err(error.into()),
        }
    };

    if let Err(error) = attempt() {
        eprintln!("prepared source cache cleanup skipped: {error}");
    }
}

/// Apply one retention decision while the caller exclusively holds cache coordination.
fn prune_locked(cache: &Path, keep: usize, apply: bool, grace: Duration) -> Result<PruneReport> {
    let pins = pinned(cache)?;
    let now = SystemTime::now();
    let mut report = PruneReport {
        candidates: Vec::new(),
        protected: Vec::new(),
        retained: Vec::new(),
        applied: apply,
    };
    let mut disposable = Vec::new();
    for entry in fs::read_dir(cache)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !identity(name) {
            continue;
        }
        if !managed(&path)? || pins.contains(name) {
            report.protected.push(path);
            continue;
        }

        let lock_path = cache.join(".locks").join(name);
        // Old caches predate last-use metadata; their receipt time is the fallback.
        let existed = lock_path.exists();
        let last_use = if existed {
            fs::metadata(&lock_path)?.modified()?
        } else {
            fs::metadata(path.join("airlock-prepared.json"))?.modified()?
        };
        let lock = lock_file(&lock_path)?;
        if !existed {
            lock.set_modified(last_use)?;
        }

        match lock.try_lock() {
            Ok(()) => disposable.push((last_use, path, lock)),
            Err(TryLockError::WouldBlock) => report.protected.push(path),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }

    disposable.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    for (index, (last_use, path, _lock)) in disposable.into_iter().enumerate() {
        // A future timestamp is treated as recent if the system clock moved backwards.
        let age = now.duration_since(last_use).unwrap_or_default();
        if index < keep || age < grace {
            report.retained.push(path);
            continue;
        }

        let bytes = size(&path)?;
        if apply {
            fs::remove_dir_all(&path)?;
        }

        report.candidates.push(PruneEntry { path, bytes });
    }

    Ok(report)
}
