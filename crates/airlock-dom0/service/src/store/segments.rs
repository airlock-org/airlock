//! Append-only segments hold canonical tar fragments; sealed segments never accept writes.
//! Each published extent records its segment, bounds and content digest.

use super::fs::*;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

/// Checked physical identity of one immutable fragment inside a pool segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Extent {
    /// Numeric segment identity, independent of its mount location.
    pub(super) segment_id: String,
    /// Canonical backing file inside the pool.
    pub(super) segment_blob: PathBuf,
    /// Byte offset from the beginning of that backing file.
    pub(super) segment_offset: u64,
    /// Number of bytes belonging to this fragment.
    pub(super) size: u64,
    /// Digest of exactly the recorded byte range.
    pub(super) sha256: String,
}

pub(super) fn segment_id(value: &str) -> Result<&str> {
    ensure!(
        value.len() >= 6 && value.bytes().all(|b| b.is_ascii_digit()),
        "invalid segment id"
    );
    Ok(value)
}

pub(super) fn blob(root: &Path, id: &str) -> Result<PathBuf> {
    let path = root
        .join("objects/segments")
        .join(format!("{}.blob", segment_id(id)?));
    pool_path(root, &path)?;
    Ok(path)
}

pub(super) fn metadata(root: &Path, id: &str) -> Result<PathBuf> {
    let path = root
        .join("objects/segments")
        .join(format!("{}.json", segment_id(id)?));
    pool_path(root, &path)?;
    Ok(path)
}

pub(super) fn object_metadata(root: &Path, entry: &str) -> Result<PathBuf> {
    let path = root
        .join("objects/blob-metadata")
        .join(format!("{}.json", component(entry)?));
    pool_path(root, &path)?;
    Ok(path)
}

pub(super) fn ids(root: &Path) -> Result<Vec<String>> {
    let mut ids = std::collections::BTreeSet::new();
    for path in children(&root.join("objects/segments"))? {
        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("json" | "blob")
        ) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .context("invalid segment filename")?;
            if segment_id(stem).is_ok() {
                ids.insert(stem.to_owned());
            }
        }
    }
    let mut ids: Vec<_> = ids.into_iter().collect();
    ids.sort_by_key(|id| id.parse::<u64>().unwrap_or(u64::MAX));
    Ok(ids)
}

fn write_meta(root: &Path, id: &str, value: &Value) -> Result<()> {
    write(
        &metadata(root, id)?,
        value,
        if value["sealed"] == true {
            0o444
        } else {
            0o600
        },
    )
}

fn create(root: &Path, target: u64, transaction: &str, oversized: bool) -> Result<String> {
    let next = ids(root)?
        .iter()
        .map(|id| id.parse::<u64>())
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .context("segment ids exhausted")?;
    let id = format!("{next:06}");
    let path = blob(root, &id)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.sync_all()?;
    sync_dir(path.parent().context("missing segment parent")?)?;
    write_meta(
        root,
        &id,
        &json!({
            "schema": "airlock.closure-store.segment.v1",
            "id": id,
            "blob": path,
            "status": "open",
            "sealed": false,
            "created_at": now(),
            "created_by_transaction": transaction,
            "target_bytes": target,
            "used_bytes": 0,
            "object_count": 0,
            "objects": [],
            "oversized": oversized,
            "append_only_while_open": true,
            "immutable_after_seal": true,
        }),
    )?;
    failpoint("segment-after-create")?;
    if !oversized {
        atomic(
            &root.join("objects/current-segment"),
            format!("{id}\n").as_bytes(),
            0o644,
        )?;
    }
    Ok(id)
}

pub(super) fn seal(root: &Path, id: &str, transaction: &str, reason: &str) -> Result<()> {
    let mut value: Value = read(&metadata(root, id)?)?;
    if value["sealed"] == true {
        return Ok(());
    }
    failpoint("segment-before-seal")?;
    let path = blob(root, id)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
    value["status"] = json!("sealed");
    value["sealed"] = json!(true);
    value["sealed_at"] = json!(now());
    value["sealed_by_transaction"] = json!(transaction);
    value["seal_reason"] = json!(reason);
    value["used_bytes"] = json!(fs::metadata(&path)?.len());
    write_meta(root, id, &value)?;
    let current = root.join("objects/current-segment");
    if current.try_exists()? && fs::read_to_string(&current)?.trim() == id {
        remove(&current)?;
    }
    failpoint("segment-after-seal")
}

fn current(root: &Path) -> Result<Option<String>> {
    let reference = root.join("objects/current-segment");
    pool_path(root, &reference)?;
    if !reference.try_exists()? {
        return Ok(None);
    }
    let id = fs::read_to_string(&reference)?.trim().to_owned();
    if !id.is_empty() && metadata(root, &id)?.try_exists()? && blob(root, &id)?.try_exists()? {
        let value: Value = read(&metadata(root, &id)?)?;
        if value["status"] == "open" && value["sealed"] != true && value["oversized"] != true {
            return Ok(Some(id));
        }
    }
    remove(&reference)?;
    Ok(None)
}

pub(super) fn append(
    root: &Path,
    source: &Path,
    entry: &str,
    transaction: &str,
    target: u64,
) -> Result<Extent> {
    ensure!(target >= 512, "segment target must be at least 512 bytes");
    let size = fs::metadata(source)?.len();
    ensure!(size > 0 && size % 512 == 0, "unaligned tar fragment");
    let _lock = PoolLock::acquire(&root.join("objects/segments.lock"))?;
    let oversized = size > target;
    let mut id = if oversized {
        create(root, target, transaction, true)?
    } else {
        current(root)?.map_or_else(|| create(root, target, transaction, false), Ok)?
    };
    let mut value: Value = read(&metadata(root, &id)?)?;
    let end = fs::metadata(blob(root, &id)?)?.len();
    if !oversized
        && number(&value, "object_count")? > 0
        && end.div_ceil(ALIGNMENT) * ALIGNMENT + size > target
    {
        seal(root, &id, transaction, "target-size-rollover")?;
        id = create(root, target, transaction, false)?;
        value = read(&metadata(root, &id)?)?;
    }

    let path = blob(root, &id)?;
    let mut backing = OpenOptions::new().read(true).write(true).open(&path)?;
    let end = backing.seek(SeekFrom::End(0))?;
    let offset = end.div_ceil(ALIGNMENT) * ALIGNMENT;
    backing.write_all(&vec![0; (offset - end) as usize])?;
    let mut source = File::open(source)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 1024 * 1024];
    let mut copied = 0;
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        backing.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        copied += count as u64;
    }
    ensure!(copied == size, "fragment changed during append");
    backing.sync_all()?;
    let sha256 = hex(&digest.finalize());
    value["updated_at"] = json!(now());
    value["used_bytes"] = json!(offset + size);
    value["object_count"] = json!(
        number(&value, "object_count")?
            .checked_add(1)
            .context("segment object count overflow")?
    );
    value["objects"]
        .as_array_mut()
        .context("invalid segment objects")?
        .push(json!({"entry":entry,"offset":offset,"size":size,"sha256":sha256}));
    write_meta(root, &id, &value)?;
    if oversized || offset + size >= target {
        seal(
            root,
            &id,
            transaction,
            if oversized {
                "oversized-object"
            } else {
                "target-size-reached"
            },
        )?;
    }
    Ok(Extent {
        segment_id: id,
        segment_blob: path,
        segment_offset: offset,
        size,
        sha256,
    })
}

pub(super) fn extent(root: &Path, meta: &Value, verify: bool) -> Result<Extent> {
    let location = meta["locations"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["status"] == "current"))
        .unwrap_or(meta);
    let id = text(location, "segment_id")?;
    let path = blob(root, &id)?;
    let offset = number(location, "segment_offset")?;
    let size = number(location, "size")?;
    let digest = text(location, "sha256")?;
    pool_path(root, &path)?;
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid fragment digest"
    );
    let mut file = File::open(&path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && size > 0
            && offset
                .checked_add(size)
                .is_some_and(|end| end <= metadata.len()),
        "fragment extent outside segment"
    );
    let key = VerificationKey {
        identity: (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        ),
        region: (offset, size, digest.clone()),
    };
    let cached = verify
        && VERIFICATION.with_borrow_mut(|scope| {
            scope.as_mut().is_some_and(|scope| {
                if scope.checked.contains(&key) {
                    scope.hits += 1;
                    true
                } else {
                    false
                }
            })
        });
    if verify && !cached {
        file.seek(SeekFrom::Start(offset))?;
        let mut digest_actual = Sha256::new();
        let mut remaining = size;
        let mut buffer = [0; 1024 * 1024];
        while remaining > 0 {
            let count = file.read(&mut buffer[..remaining.min(1024 * 1024) as usize])?;
            ensure!(count > 0, "segment extent truncated");
            digest_actual.update(&buffer[..count]);
            remaining -= count as u64;
        }
        let after = file.metadata()?;
        ensure!(
            (
                metadata.dev(),
                metadata.ino(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec()
            ) == (
                after.dev(),
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec()
            ),
            "segment changed during verification"
        );
        ensure!(
            hex(&digest_actual.finalize()) == digest,
            "segment extent checksum mismatch"
        );
        VERIFICATION.with_borrow_mut(|scope| {
            if let Some(scope) = scope {
                scope.bytes += size;
                scope.checked.insert(key);
            }
        });
    }
    Ok(Extent {
        segment_id: id,
        segment_blob: path,
        segment_offset: offset,
        size,
        sha256: digest,
    })
}

pub(super) use airlock_nix_core::artifact::create_tar_fragment as tar_fragment;

pub(super) fn reusable(root: &Path, path: &str) -> Result<Option<Value>> {
    let name = entry(path)?;
    let object = root.join("objects/metadata").join(format!("{name}.json"));
    let fragment = object_metadata(root, name)?;
    if !object.try_exists()? || !fragment.try_exists()? {
        return Ok(None);
    }
    let mut value: Value = read(&object)?;
    let fragment: Value = read(&fragment)?;
    ensure!(
        value["schema"] == "airlock.closure-store.object.v2"
            && value["path"] == path
            && fragment["path"] == path
            && value["entry"] == name
            && fragment["entry"] == name
            && value["tree_hash"].is_string()
            && value["tree_hash"] == fragment["tree_hash"],
        "object admission identity mismatch"
    );
    ensure!(
        extent(root, &value["tar_fragment"], false)? == extent(root, &fragment, false)?,
        "object extent identity mismatch"
    );
    value["admitted"] = json!(false);
    Ok(Some(value))
}

pub(super) fn admit_fragment(
    root: &Path,
    logical: &str,
    fragment: &Path,
    tree_hash: &str,
    transaction: &str,
    target: u64,
) -> Result<Value> {
    let name = entry(logical)?;
    let cached = super::image_cache::fragment(logical)?;
    if let Some(cached) = &cached {
        ensure!(
            cached.identity.tree_hash == tree_hash && cached.path == fragment,
            "cached fragment admission mismatch"
        );
    }
    let meta = object_metadata(root, name)?;
    let existing = if meta.try_exists()? {
        Some(read::<Value>(&meta)?)
    } else {
        None
    };
    let (mut value, admitted) = if let Some(value) = existing {
        ensure!(
            value["tree_hash"] == tree_hash,
            "existing object hash mismatch"
        );
        let extent = extent(root, &value, true)?;
        if let Some(cached) = &cached {
            ensure!(
                extent.sha256 == cached.identity.sha256 && extent.size == cached.identity.size,
                "cached object extent mismatch"
            );
        }
        (value, false)
    } else {
        let location = append(root, fragment, name, transaction, target)?;
        if let Some(cached) = &cached {
            ensure!(
                location.sha256 == cached.identity.sha256 && location.size == cached.identity.size,
                "cached fragment checksum mismatch"
            );
        }
        failpoint("object-after-append")?;
        let mut location_json = serde_json::to_value(&location)?;
        location_json["status"] = json!("current");
        location_json["created_at"] = json!(now());
        location_json["transaction"] = json!(transaction);
        let mut value = serde_json::to_value(location)?;
        for (key, field) in json!({
            "schema": "airlock.closure-store.object-tar-fragment.v1",
            "path": logical,
            "entry": name,
            "tree_hash": tree_hash,
            "format": "posix-tar-fragment-v1",
            "admitted_at": now(),
            "transaction": transaction,
            "locations": [location_json],
        })
        .as_object()
        .context("invalid object fields")?
        {
            value[key] = field.clone();
        }
        write(&meta, &value, 0o644)?;
        failpoint("object-after-metadata")?;
        (value, true)
    };

    let object_path = root.join("objects/metadata").join(format!("{name}.json"));
    let old = if object_path.try_exists()? {
        read::<Value>(&object_path)?
    } else {
        Value::Null
    };
    ensure!(
        old.is_null() || old["tree_hash"] == tree_hash,
        "object tree identity mismatch"
    );

    let object = json!({
        "schema": "airlock.closure-store.object.v2",
        "path": logical,
        "entry": name,
        "tree_hash": tree_hash,
        "tar_fragment": value,
        "tree_materialized": false,
        "admitted_at": old.get("admitted_at").cloned().unwrap_or_else(|| json!(now())),
    });
    write(&object_path, &object, 0o644)?;
    value = object;
    value["admitted"] = json!(admitted || old.is_null());
    Ok(value)
}

pub(super) fn admit(
    root: &Path,
    logical: &str,
    physical: &Path,
    transaction: &str,
    target: u64,
) -> Result<Value> {
    if let Some(cached) = super::image_cache::fragment(logical)? {
        // These descriptors are explicit Nix build inputs, not a sender-supplied runtime cache.
        // Verify bytes while appending; unchanged packages need no source rehash or tar rebuild.
        if let Some(value) = reusable(root, logical)? {
            let extent = extent(root, &value["tar_fragment"], true)?;
            ensure!(
                value["tree_hash"] == cached.identity.tree_hash
                    && extent.sha256 == cached.identity.sha256
                    && extent.size == cached.identity.size,
                "cached object identity mismatch"
            );
            return Ok(value);
        }
        return admit_fragment(
            root,
            logical,
            &cached.path,
            &cached.identity.tree_hash,
            transaction,
            target,
        );
    }

    let source = physical.join(entry(logical)?);
    let digest = tree_hash(&source)?;
    if image_build()
        && let Some(value) = reusable(root, logical)?
    {
        // Image preparation visits the same immutable packages for several views.
        // Recheck the source tree and backing bytes, without serializing another tar.
        ensure!(
            value["tree_hash"] == digest,
            "existing object hash mismatch"
        );
        extent(root, &value["tar_fragment"], true)?;
        return Ok(value);
    }

    let temporary = tempfile::NamedTempFile::new_in(root.join("tmp"))?;
    tar_fragment(&source, temporary.path())?;
    admit_fragment(
        root,
        logical,
        temporary.path(),
        &digest,
        transaction,
        target,
    )
}

pub(super) fn eof(root: &Path, transaction: &str, target: u64) -> Result<Value> {
    let path = object_metadata(root, ".tar-eof")?;
    if path.try_exists()? {
        let value = read(&path)?;
        extent(root, &value, true)?;
        return Ok(value);
    }
    let mut temporary = tempfile::NamedTempFile::new_in(root.join("tmp"))?;
    temporary.write_all(&[0; 1024])?;
    let mut value = serde_json::to_value(append(
        root,
        temporary.path(),
        ".tar-eof",
        transaction,
        target,
    )?)?;
    value["schema"] = json!("airlock.closure-store.tar-eof.v1");
    value["admitted_at"] = json!(now());
    value["transaction"] = json!(transaction);
    write(&path, &value, 0o644)?;
    Ok(value)
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VerificationKey {
    // Device and inode identify the open backing file rather than a replaceable pathname.
    identity: (u64, u64, u64, i64, i64, i64, i64),
    // Byte range and expected digest distinguish independent extents in the same segment.
    region: (u64, u64, String),
}

#[derive(Default)]
struct Verification {
    // Only extents checked within this admission-locked operation may be reused.
    checked: std::collections::BTreeSet<VerificationKey>,
    // Actual payload bytes reread during the current maintenance operation.
    bytes: u64,
    // Repeated checks avoided because open-file identity and expected range match.
    hits: u64,
}

thread_local! {
    static VERIFICATION: std::cell::RefCell<Option<Verification>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn with_verification(operation: impl FnOnce() -> Result<Value>) -> Result<Value> {
    struct Reset(
        // Previous operation scope restored if validation or publication fails.
        Option<Verification>,
    );

    impl Drop for Reset {
        fn drop(&mut self) {
            VERIFICATION.set(self.0.take());
        }
    }

    let _reset = Reset(VERIFICATION.replace(Some(Verification::default())));
    let started = std::time::Instant::now();
    let mut result = operation()?;
    VERIFICATION.with_borrow(|scope| {
        if let Some(scope) = scope {
            result["verification"] = json!({"payload_bytes_hashed": scope.bytes, "cache_hits": scope.hits, "elapsed_seconds": started.elapsed().as_secs_f64()});
        }
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Image reuse preserves object metadata while still rejecting changed source trees and backing bytes.
    #[test]
    fn image_reuse_rechecks_objects_without_republishing_them() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("pool");
        ensure_pool(&root)?;
        let physical = temporary.path().join("source");
        let logical = format!("/nix/store/{hash}-package", hash = "a".repeat(32));
        let source = physical.join(entry(&logical)?);
        fs::create_dir_all(&source)?;
        fs::write(source.join("payload"), b"immutable package")?;

        with_image_build(|| {
            let first = admit(&root, &logical, &physical, "first", SEGMENT_TARGET)?;
            let object = root
                .join("objects/metadata")
                .join(format!("{name}.json", name = entry(&logical)?));
            let inode = fs::metadata(&object)?.ino();
            let second = admit(&root, &logical, &physical, "second", SEGMENT_TARGET)?;
            assert_eq!(second["admitted"], false);
            assert_eq!(fs::metadata(&object)?.ino(), inode);
            assert_eq!(first["tar_fragment"], second["tar_fragment"]);

            fs::write(source.join("payload"), b"changed package")?;
            assert!(admit(&root, &logical, &physical, "changed", SEGMENT_TARGET).is_err());
            fs::write(source.join("payload"), b"immutable package")?;

            let extent = extent(&root, &second["tar_fragment"], false)?;
            let mut backing = OpenOptions::new().write(true).open(extent.segment_blob)?;
            backing.seek(SeekFrom::Start(extent.segment_offset))?;
            backing.write_all(&[0xff])?;
            backing.sync_all()?;
            assert!(admit(&root, &logical, &physical, "corrupt", SEGMENT_TARGET).is_err());
            Ok(json!({}))
        })?;
        Ok(())
    }
}
