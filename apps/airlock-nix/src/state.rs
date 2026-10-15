//! Local primitives only; no production paths are selected by this module.
//!
//! These do not replace Python's durable state or verified lock-owner sidecars.
//! In particular, this initial lock guard exposes its owner to its caller but
//! does not publish an on-disk owner record or identify another process's owner.

use crate::{
    Error, Result,
    error::io_error,
    identity::{JobId, ReviewId},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions, Permissions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

/// Read one bounded snapshot. Never repair or skip a damaged record.
pub fn read_json<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let file = File::open(path).map_err(|error| io_error("open record", path, error))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("read record", path, error))?;
    if bytes.len() as u64 > limit {
        return Err(Error::RecordTooLarge {
            path: path.to_owned(),
            limit,
        });
    }
    serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: path.to_owned(),
        source,
    })
}

#[derive(Clone, Copy, Debug)]
pub enum RecordPermissions {
    Private,
    Public,
    PreserveExisting,
}

/// Unique temporary file, file fsync, atomic rename, directory fsync. A failure
/// after rename is reported even though the destination may already be updated.
/// The caller owns the directory and serializes competing logical updates.
pub fn atomic_json<T: Serialize>(
    path: &Path,
    value: &T,
    permissions: RecordPermissions,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create state directory", parent, error))?;
    let mode = match permissions {
        RecordPermissions::Private => 0o600,
        RecordPermissions::Public => 0o644,
        RecordPermissions::PreserveExisting => match fs::metadata(path) {
            Ok(metadata) => metadata.permissions().mode() & 0o777,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0o600,
            Err(error) => return Err(io_error("read record permissions", path, error)),
        },
    };
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| io_error("create temporary record", path, error))?;
    temporary
        .as_file()
        .set_permissions(Permissions::from_mode(mode))
        .map_err(|error| io_error("set record permissions", path, error))?;
    serde_json::to_writer_pretty(&mut temporary, value).map_err(|source| Error::Json {
        path: path.to_owned(),
        source,
    })?;
    temporary
        .write_all(b"\n")
        .map_err(|error| io_error("write record", path, error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync record", path, error))?;
    temporary
        .persist(path)
        .map_err(|error| io_error("publish record", path, error.error))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error("sync state directory", parent, error))
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
    _file: File,
    path: PathBuf,
    owner: OperationOwner,
}

impl OperationLock {
    /// The caller creates and owns the parent directory. Acquisition is always
    /// nonblocking; waiting/retry policy belongs to a future runtime adapter.
    pub fn try_acquire(path: &Path, owner: OperationOwner) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|error| io_error("open operation lock", path, error))?;
        match file.try_lock() {
            Ok(()) => Ok(Self {
                _file: file,
                path: path.to_owned(),
                owner,
            }),
            Err(std::fs::TryLockError::WouldBlock) => Err(Error::Busy {
                path: path.to_owned(),
            }),
            Err(std::fs::TryLockError::Error(error)) => {
                Err(io_error("acquire operation lock", path, error))
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn owner(&self) -> &OperationOwner {
        &self.owner
    }
}
