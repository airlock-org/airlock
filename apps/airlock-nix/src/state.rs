//! Local storage capabilities. Observations never acquire a writer lock.
//!
//! A store reserves separate lock/record namespaces. Publication is private and
//! requires the matching resource guard and a current, unforgeable snapshot.
//! The directory owner must keep this namespace trusted and stable.

mod linux_owner;
mod lock;
mod store;

pub use lock::{
    LockHolder, LockObservation, LockOwnerDetails, OperationKind, OperationLock, OperationOwner,
};
pub use store::{Record, RecordSnapshot, RecordWriter, ResourceKey, StateStore};

use crate::{
    Error, Result,
    error::io_error,
    wire::{validate_json, validate_json_reader},
};
use rustix::fs::{Mode, OFlags, open};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, Permissions},
    io::{Read, Seek, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
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
    read_file_json(&file, path, limit)
}

fn read_file_json<T: DeserializeOwned>(file: &File, path: &Path, limit: u64) -> Result<T> {
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

/// Stop serialization before its output can exceed the record's disk budget.
/// Keep the limit failure separate from underlying I/O errors, including errno.
struct BoundedWriter<'a> {
    file: &'a mut File,
    remaining: u64,
    exceeded: bool,
}

impl Write for BoundedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.exceeded || bytes.len() as u64 > self.remaining {
            self.exceeded = true;
            return Err(std::io::Error::other("record byte limit exceeded"));
        }
        let written = self.file.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Unique temporary file, file fsync, atomic rename, directory fsync. A failure
/// after rename is reported even though the destination may already be updated.
/// Private: only resource-bound writers may publish records.
fn publish_json<T: Serialize>(
    path: &Path,
    value: &T,
    permissions: RecordPermissions,
    limit: u64,
    before_publish: impl FnOnce() -> Result<()>,
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
    {
        let mut output = BoundedWriter {
            file: temporary.as_file_mut(),
            remaining: limit,
            exceeded: false,
        };
        let result = serde_json::to_writer_pretty(&mut output, value)
            .map_err(|source| serialization_error(path, source))
            .and_then(|()| {
                output
                    .write_all(b"\n")
                    .map_err(|error| io_error("write record", path, error))
            });
        if output.exceeded {
            return Err(Error::RecordTooLarge {
                path: path.to_owned(),
                limit,
            });
        }
        result?;
    }
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
    before_publish()?;
    temporary
        .persist(path)
        .map_err(|error| io_error("publish record", path, error.error))?;
    directory
        .sync_all()
        .map_err(|source| Error::PublishedButNotDurable {
            path: path.to_owned(),
            source,
        })?;
    Ok(())
}
