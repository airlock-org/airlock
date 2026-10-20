//! Canonical object hashing and deterministic tar fragments shared by artifact peers.

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

pub(super) fn component(value: &str) -> Result<&str> {
    ensure!(
        !value.is_empty() && value != "." && value != ".." && !value.contains(['/', '\0']),
        "invalid pool path component"
    );
    Ok(value)
}

pub(super) fn entry(value: &str) -> Result<&str> {
    component(
        value
            .strip_prefix("/nix/store/")
            .context("not a Nix store root")?,
    )
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 1024 * 1024];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        digest.update(&buffer[..size]);
    }
    Ok(hex(&digest.finalize()))
}

pub(super) fn children(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.try_exists()? {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(path)?
        .map(|item| item.map(|i| i.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}

pub(super) fn descendants(path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for child in children(path)? {
        if fs::symlink_metadata(&child)?.is_dir() {
            paths.extend(descendants(&child)?);
        }
        paths.push(child);
    }
    paths.sort();
    Ok(paths)
}

pub(super) fn tree_hash(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    let mut digest = Sha256::new();
    if metadata.is_symlink() {
        digest.update(b"L\0");
        digest.update(fs::read_link(path)?.as_os_str().as_encoded_bytes());
    } else if metadata.is_file() {
        digest.update(format!("F\0{:o}\0", metadata.mode() & 0o7777));
        digest.update(hash(path)?.as_bytes());
    } else {
        ensure!(metadata.is_dir(), "unsupported store object type");
        digest.update(b"D\0");
        for child in descendants(path)? {
            let relative = child
                .strip_prefix(path)?
                .to_str()
                .context("non-UTF8 store filename")?;
            let meta = fs::symlink_metadata(&child)?;
            if meta.is_symlink() {
                digest.update(format!(
                    "L\0{relative}\0{}\0",
                    fs::read_link(&child)?
                        .to_str()
                        .context("non-UTF8 symlink")?
                ));
            } else if meta.is_dir() {
                digest.update(format!("D\0{relative}\0{:o}\0", meta.mode() & 0o7777));
            } else {
                ensure!(meta.is_file(), "unsupported store object type");
                digest.update(format!("F\0{relative}\0{:o}\0", meta.mode() & 0o7777));
                digest.update(hash(&child)?.as_bytes());
                digest.update(b"\0");
            }
        }
    }
    Ok(hex(&digest.finalize()))
}
