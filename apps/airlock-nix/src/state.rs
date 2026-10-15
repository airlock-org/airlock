//! Local primitives only; no production paths are selected by this module.
//!
//! These do not replace Python's durable state or verified lock-owner sidecars.
//! In particular, this initial lock guard exposes its owner to its caller but
//! does not publish an on-disk owner record or identify another process's owner.

use crate::{
    Error, Result,
    error::io_error,
    identity::{JobId, ReviewId},
    wire::{validate_json, validate_json_reader},
};
use rustix::fs::{Mode, OFlags, open};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, Permissions},
    io::{Read, Seek, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

fn open_file(path: &Path, flags: OFlags, mode: Mode) -> Result<File> {
    let descriptor = open(
        path,
        flags | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        mode,
    )
    .map_err(|error| io_error("open file", path, error.into()))?;
    let file = File::from(descriptor);
    if !file
        .metadata()
        .map_err(|error| io_error("inspect file", path, error))?
        .is_file()
    {
        return Err(Error::NotRegularFile {
            path: path.to_owned(),
        });
    }
    Ok(file)
}

/// Read one bounded regular-file snapshot. Final-component symlinks, devices,
/// directories and FIFOs are rejected. Never repair or skip a damaged record.
pub fn read_json<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let file = open_file(path, OFlags::RDONLY, Mode::empty())?;
    let read_limit = limit.checked_add(1).ok_or(Error::InvalidField {
        field: "record limit",
        expected: "a byte limit smaller than u64::MAX",
    })?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("read record", path, error))?;
    if bytes.len() as u64 > limit {
        return Err(Error::RecordTooLarge {
            path: path.to_owned(),
            limit,
        });
    }
    parse_json(&bytes, path)
}

/// Parse one complete record without ambiguous duplicate keys, including in
/// unknown metadata. Callers receiving transport bytes can use this boundary
/// without the filesystem reader. The caller must bound those input bytes.
pub fn parse_json<T: DeserializeOwned>(bytes: &[u8], origin: &Path) -> Result<T> {
    validate_json(bytes)
        .and_then(|()| serde_json::from_slice(bytes))
        .map_err(|source| Error::Json {
            path: origin.to_owned(),
            source,
        })
}

#[derive(Clone, Copy, Debug)]
pub enum RecordPermissions {
    Private,
    Public,
    PreserveExisting,
}

fn serialization_error(path: &Path, source: serde_json::Error) -> Error {
    if source.is_io() {
        io_error("write or validate record", path, source.into())
    } else {
        Error::Serialization {
            path: path.to_owned(),
            source,
        }
    }
}

/// Unique temporary file, file fsync, atomic rename, directory fsync. A failure
/// after rename is reported even though the destination may already be updated.
/// The parent directory must already exist. The caller owns the directory,
/// keeps its namespace stable, and serializes competing logical updates.
pub fn atomic_json<T: Serialize>(
    path: &Path,
    value: &T,
    permissions: RecordPermissions,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = File::from(
        open(
            parent,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| io_error("open state directory", parent, error.into()))?,
    );
    let existing = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Some(metadata),
        Ok(_) => {
            return Err(Error::NotRegularFile {
                path: path.to_owned(),
            });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_error("inspect destination record", path, error)),
    };
    let mode = match permissions {
        RecordPermissions::Private => 0o600,
        RecordPermissions::Public => 0o644,
        RecordPermissions::PreserveExisting => {
            existing.map_or(0o600, |metadata| metadata.permissions().mode() & 0o777)
        }
    };
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| io_error("create temporary record", path, error))?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), value)
        .map_err(|source| serialization_error(path, source))?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .map_err(|error| io_error("write record", path, error))?;
    temporary
        .as_file_mut()
        .rewind()
        .map_err(|error| io_error("rewind record", path, error))?;
    validate_json_reader(temporary.as_file_mut())
        .map_err(|source| serialization_error(path, source))?;
    // Incomplete temporary files remain private, even for public destinations.
    temporary
        .as_file()
        .set_permissions(Permissions::from_mode(mode))
        .map_err(|error| io_error("set record permissions", path, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync record", path, error))?;
    temporary
        .persist(path)
        .map_err(|error| io_error("publish record", path, error.error))?;
    directory
        .sync_all()
        .map_err(|source| Error::PublishedButNotDurable {
            path: path.to_owned(),
            source,
        })
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    SourceImport,
    Review,
    Build,
    Deployment,
    Recovery,
    Maintenance,
}

#[derive(Clone, Debug)]
pub struct OperationOwner {
    name: String,
    reason: String,
    operation: OperationKind,
    job_id: Option<JobId>,
    review_id: Option<ReviewId>,
}

impl OperationOwner {
    pub fn new(name: String, reason: String, operation: OperationKind) -> Result<Self> {
        if [name.as_str(), reason.as_str()].iter().any(|value| {
            value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control)
        }) {
            return Err(Error::InvalidField {
                field: "lock owner/reason",
                expected: "nonblank text up to 1024 bytes, without control characters",
            });
        }
        Ok(Self {
            name,
            reason,
            operation,
            job_id: None,
            review_id: None,
        })
    }

    pub fn with_job(mut self, job_id: JobId) -> Self {
        self.job_id = Some(job_id);
        self
    }
    pub fn with_review(mut self, review_id: ReviewId) -> Self {
        self.review_id = Some(review_id);
        self
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn operation(&self) -> OperationKind {
        self.operation
    }
    pub fn job_id(&self) -> Option<&JobId> {
        self.job_id.as_ref()
    }
    pub fn review_id(&self) -> Option<&ReviewId> {
        self.review_id.as_ref()
    }
}

/// The kernel lock is held by an unexposed File and released on drop. It is not
/// Clone, does not unlink its lock file, and is not inherited across exec.
#[derive(Debug)]
#[must_use = "the operation lock is released when its guard is dropped"]
pub struct OperationLock {
    file: File,
    path: PathBuf,
    owner: OperationOwner,
}

impl OperationLock {
    /// The caller creates and owns the parent directory. Acquisition is always
    /// nonblocking; waiting/retry policy belongs to a future runtime adapter.
    pub fn try_acquire(path: &Path, owner: OperationOwner) -> Result<Self> {
        let file = open_file(path, OFlags::RDWR | OFlags::CREATE, Mode::RUSR | Mode::WUSR)?;
        match file.try_lock() {
            Ok(()) => {
                let guard = Self {
                    file,
                    path: path.to_owned(),
                    owner,
                };
                guard.check_path()?;
                Ok(guard)
            }
            Err(std::fs::TryLockError::WouldBlock) => Err(Error::Busy {
                path: path.to_owned(),
            }),
            Err(std::fs::TryLockError::Error(error)) => {
                Err(io_error("acquire operation lock", path, error))
            }
        }
    }

    /// Detect namespace replacement. A trusted owner must still prevent anyone
    /// unlinking/renaming the lock path while it protects an operation.
    pub fn check_path(&self) -> Result<()> {
        let held = self
            .file
            .metadata()
            .map_err(|error| io_error("inspect held lock", &self.path, error))?;
        let named = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::LockReplaced {
                    path: self.path.clone(),
                });
            }
            Err(error) => return Err(io_error("inspect lock path", &self.path, error)),
        };
        if !named.is_file()
            || held.nlink() != 1
            || (held.dev(), held.ino()) != (named.dev(), named.ino())
        {
            return Err(Error::LockReplaced {
                path: self.path.clone(),
            });
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn owner(&self) -> &OperationOwner {
        &self.owner
    }
}
