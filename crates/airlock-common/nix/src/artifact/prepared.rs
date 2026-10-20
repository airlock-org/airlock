//! Receive the validator's generated Nix tree without executing its Rust authoring program.

use super::util;
use crate::identity::GitRevision;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    os::unix::fs::PermissionsExt,
    path::{Component, Path},
};

/// Require an artifact-local file name before joining it to the destination directory.
fn relative(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
            && !path.components().any(|part| part.as_os_str() == ".git"),
        "prepared source contains an unsafe path"
    );
    Ok(())
}

/// Verify every extracted file against the preparation receipt, including executable permissions.
fn verify(directory: &Path, revision: &GitRevision, count: usize) -> Result<()> {
    let receipt: Value =
        serde_json::from_slice(&fs::read(directory.join("airlock-prepared.json"))?)?;
    ensure!(
        receipt["schema"] == "airlock.prepared-source.v1",
        "unsupported prepared source receipt"
    );
    ensure!(
        receipt["revision"] == revision.as_str() && receipt["dirty"] == false,
        "prepared source differs from the validated clean commit"
    );
    let files = receipt["artifact_files"]
        .as_object()
        .context("prepared file inventory")?;
    ensure!(
        files.len() + 1 == count,
        "prepared source has missing or extra files"
    );

    let mut inventory = BTreeMap::new();
    for (name, expected) in files {
        let name = Path::new(name);
        relative(name)?;
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file(),
            "prepared inventory entry is not a regular file"
        );
        let mut hash = Sha256::new();
        hash.update(metadata.permissions().mode().to_le_bytes());
        hash.update(fs::read(&path)?);
        let hash = format!("{value:x}", value = hash.finalize());
        ensure!(
            expected.as_str() == Some(&hash),
            "prepared source file checksum mismatch"
        );
        inventory.insert(name.to_string_lossy().into_owned(), hash);
    }
    let hash = format!(
        "{value:x}",
        value = Sha256::digest(serde_json::to_vec(&inventory)?)
    );
    ensure!(
        receipt["artifact_sha256"] == hash,
        "prepared source inventory checksum mismatch"
    );

    Ok(())
}

/// Verify and atomically unpack a validator-approved candidate's generated build tree.
/// The expected archive checksum must come from the authenticated validation record.
/// Reject links, devices, duplicate entries, path traversal and oversized payloads. Candidate
/// source is copied as data; this operation never runs Cargo, Nix or an authoring executable.
pub fn unpack(
    archive: &Path,
    destination: &Path,
    revision: &GitRevision,
    sha256: &str,
) -> Result<()> {
    let metadata = fs::symlink_metadata(archive)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 640 * 1024 * 1024,
        "prepared archive must be a bounded regular file"
    );
    ensure!(
        sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid prepared archive checksum"
    );
    ensure!(
        util::hash(archive)? == sha256,
        "prepared archive checksum mismatch"
    );
    ensure!(
        !destination.exists(),
        "prepared source destination already exists"
    );
    let parent = destination
        .parent()
        .context("prepared destination parent")?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    let mut seen = BTreeSet::new();
    let mut bytes = 0_u64;
    let mut count = 0;
    for entry in tar::Archive::new(File::open(archive)?).entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        relative(&path)?;
        ensure!(seen.insert(path.clone()), "duplicate prepared source entry");
        ensure!(
            seen.len() <= 100_000,
            "prepared source has too many entries"
        );
        let kind = entry.header().entry_type();
        ensure!(
            kind.is_dir() || kind.is_file(),
            "prepared source links and special files are forbidden"
        );
        ensure!(
            entry.header().mode()? & !0o777 == 0,
            "prepared source has special permissions"
        );
        bytes = bytes
            .checked_add(entry.size())
            .context("prepared source size overflow")?;
        ensure!(
            bytes <= 512 * 1024 * 1024,
            "prepared source exceeds size limit"
        );
        ensure!(
            entry.unpack_in(staging.path())?,
            "prepared source escaped destination"
        );
        if kind.is_file() {
            count += 1;
        }
    }
    verify(staging.path(), revision, count)?;
    fs::rename(staging.path(), destination)?;

    Ok(())
}
