//! Bundle a committed machine checkout for submission through qrexec.

use super::{
    Error, Result, process,
    protocol::{ARCHIVE_LIMIT, identity},
    require,
};
use crate::identity::SourceIdentity;
use flate2::{Compression, write::GzEncoder};
use std::{fs::File, path::Path};

/// Create a bounded compressed Git bundle and its source manifest from a clean checkout.
/// The returned archive is read-only; the caller owns its containing temporary directory.
pub fn source_archive(repo: &Path, directory: &Path) -> Result<(SourceIdentity, File)> {
    let source = identity(repo)?;
    let repo_arg = repo
        .to_str()
        .ok_or_else(|| Error::Message("non-UTF-8 repository path".into()))?;
    require(
        process::text(
            "git",
            &["-C", repo_arg, "submodule", "status", "--recursive"],
        )?
        .is_empty(),
        "source bundles with submodules are unsupported",
    )?;

    let bundle = directory.join("repo.bundle");
    process::output(
        "git",
        &[
            "-C",
            repo_arg,
            "bundle",
            "create",
            bundle
                .to_str()
                .ok_or_else(|| Error::Message("non-UTF-8 source bundle path".into()))?,
            "HEAD",
            "--all",
        ],
    )?;
    require(
        identity(repo)? == source,
        "checkout changed while creating source bundle",
    )?;

    let manifest = serde_json::to_vec(
        &serde_json::json!({"schema":"airlock.sys-agent-machine-source.v1","revision":source.revision,"lock_hash":source.lock_hash,"submodules":[]}),
    )?;
    let archive = File::create(directory.join("source.tar.gz"))?;
    let gzip = GzEncoder::new(archive, Compression::fast());
    let mut tar = tar::Builder::new(gzip);
    tar.append_path_with_name(bundle, "repo.bundle")?;

    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    tar.append_data(&mut header, "manifest.json", manifest.as_slice())?;
    tar.into_inner()?.finish()?.sync_all()?;

    let file = File::open(directory.join("source.tar.gz"))?;
    require(
        file.metadata()?.len() <= ARCHIVE_LIMIT,
        "source archive exceeds upload limit",
    )?;
    Ok((source, file))
}
