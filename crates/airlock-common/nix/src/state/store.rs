use super::{
    LockObservation, OperationLock, OperationOwner, RecordPermissions, open_file, publish_json,
    read_file_json,
};
use crate::{Error, Result, error::io_error};
use rustix::fs::{Mode, OFlags, open};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, Metadata},
    marker::PhantomData,
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    str::FromStr,
};

/// One validated resource name, never a filesystem path. Record and operation
/// locks use separate prefixes in the store's reserved lock directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceKey(
    /// Validated single resource component; cannot select arbitrary filesystem paths.
    String,
);

impl FromStr for ResourceKey {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        if value.is_empty()
            || value.len() > 128
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(Error::InvalidField {
                field: "state resource key",
                expected: "1–128 ASCII letters, digits, underscores, dots or hyphens; starting with a letter or digit",
            });
        }

        Ok(Self(value.to_owned()))
    }
}

impl ResourceKey {
    /// Return the validated single resource name used within the reserved namespace.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct Directory {
    /// Open directory descriptor pinning the trusted inode.
    file: File,
    /// Canonical name rechecked against the pinned directory.
    path: PathBuf,
}

impl Directory {
    fn open(path: &Path) -> Result<Self> {
        let file = File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| io_error("open state directory", path, error.into()))?,
        );

        // Group/other writers could swap the inode or introduce aliases while
        // an operation runs. Ancestors must also be trusted by the caller.
        if file
            .metadata()
            .map_err(|error| io_error("inspect state directory", path, error))?
            .permissions()
            .mode()
            & 0o022
            != 0
        {
            return Err(Error::InvalidStateDirectory {
                path: path.to_owned(),
            });
        }

        Ok(Self {
            file,
            path: path.to_owned(),
        })
    }

    fn check(&self) -> Result<()> {
        let held = self
            .file
            .metadata()
            .map_err(|error| io_error("inspect state directory", &self.path, error))?;
        let named = fs::symlink_metadata(&self.path)
            .map_err(|error| io_error("inspect state directory path", &self.path, error))?;
        if !named.is_dir()
            || (held.dev(), held.ino()) != (named.dev(), named.ino())
            || named.permissions().mode() & 0o022 != 0
        {
            return Err(Error::InvalidStateDirectory {
                path: self.path.clone(),
            });
        }

        Ok(())
    }
}

/// One trusted, pre-existing root. Only explicit initialization creates the
/// `records` and `.locks` directories; opening/observing a store creates nothing.
#[derive(Debug)]
pub struct StateStore {
    /// Trusted existing directory containing the state namespaces.
    root: Directory,
    /// Reserved directory for atomically published JSON records.
    records: Directory,
    /// Separate directory for persistent lock inodes and their diagnostics.
    locks: Directory,
}

impl StateStore {
    /// Create reserved records/locks directories under an existing trusted root and open them.
    ///
    /// Reject unsafe directory permissions or identities; never create the root itself.
    pub fn initialize(root: &Path) -> Result<Self> {
        let directory = Directory::open(root)?;
        for name in ["records", ".locks"] {
            directory.check()?;
            let path = root.join(name);
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(io_error("create state directory", &path, error)),
            }
            Directory::open(&path)?;
        }

        directory
            .file
            .sync_all()
            .map_err(|error| io_error("sync initialized state directory", root, error))?;
        Self::open(root)
    }

    /// Open existing reserved directories without creating, repairing or locking state.
    ///
    /// The caller must keep ancestor directories trusted and the namespace stable.
    pub fn open(root: &Path) -> Result<Self> {
        let original = Directory::open(root)?;
        let path = fs::canonicalize(root)
            .map_err(|error| io_error("resolve state directory", root, error))?;
        let canonical = Directory::open(&path)?;

        let original_metadata = original
            .file
            .metadata()
            .map_err(|error| io_error("inspect state directory", root, error))?;
        let canonical_metadata = canonical
            .file
            .metadata()
            .map_err(|error| io_error("inspect state directory", &path, error))?;
        if (original_metadata.dev(), original_metadata.ino())
            != (canonical_metadata.dev(), canonical_metadata.ino())
        {
            return Err(Error::InvalidStateDirectory { path });
        }

        let store = Self {
            records: Directory::open(&path.join("records"))?,
            locks: Directory::open(&path.join(".locks"))?,
            root: canonical,
        };
        store.check()?;
        Ok(store)
    }

    fn check(&self) -> Result<()> {
        self.root.check()?;
        self.records.check()?;
        self.locks.check()
    }

    /// Acquire nonblocking exclusion for a named operation or report its current holder.
    ///
    /// Operation and record locks occupy separate namespaces.
    pub fn try_operation(&self, key: &ResourceKey, owner: OperationOwner) -> Result<OperationLock> {
        self.check()?;
        OperationLock::try_acquire(
            &self.locks.path.join(format!("operation-{}.lock", key.0)),
            owner,
        )
    }

    /// Observe the named operation's kernel lock without creating or acquiring it.
    pub fn operation_status(&self, key: &ResourceKey) -> Result<LockObservation> {
        self.check()?;
        OperationLock::observe(&self.locks.path.join(format!("operation-{}.lock", key.0)))
    }

    /// Choose one schema and byte limit per resource. Neither a key nor its
    /// record type can select a lock path as a publication destination.
    pub fn record<T>(&self, key: ResourceKey, limit: u64) -> Result<Record<'_, T>> {
        if limit == 0 || limit == u64::MAX {
            return Err(Error::InvalidField {
                field: "record limit",
                expected: "a nonzero byte limit smaller than u64::MAX",
            });
        }

        Ok(Record {
            store: self,
            path: self.records.path.join(format!("{}.json", key.0)),
            lock_path: self.locks.path.join(format!("record-{}.lock", key.0)),
            limit,
            schema: PhantomData,
        })
    }
}

/// Resource-bound JSON record capability carrying its schema and size limit.
///
/// Reading is lock-free; mutation requires [`Self::try_write`] and a current snapshot.
#[derive(Debug)]
pub struct Record<'store, T> {
    /// Trusted store whose namespace and permissions are rechecked before access.
    store: &'store StateStore,
    /// Reserved JSON destination derived from the validated resource key.
    path: PathBuf,
    /// Separate persistent lock inode serializing writers for this resource.
    lock_path: PathBuf,
    /// Maximum JSON record size in bytes, excluding zero and u64::MAX.
    limit: u64,
    /// Record schema carried at the type level without owning a value.
    schema: PhantomData<fn() -> T>,
}

impl<'store, T> Record<'store, T> {
    /// Observe this record's writer lock without acquiring exclusion.
    pub fn lock_status(&self) -> Result<LockObservation> {
        self.store.check()?;
        OperationLock::observe(&self.lock_path)
    }

    /// Acquire nonblocking writer exclusion for this resource, preserving its schema and limit.
    pub fn try_write(&self, owner: OperationOwner) -> Result<RecordWriter<'store, T>> {
        self.store.check()?;

        let lock = OperationLock::try_acquire(&self.lock_path, owner)?;
        Ok(RecordWriter {
            record: Self {
                store: self.store,
                path: self.path.clone(),
                lock_path: self.lock_path.clone(),
                limit: self.limit,
                schema: PhantomData,
            },
            lock,
        })
    }
}

impl<T: DeserializeOwned> Record<'_, T> {
    /// A lock-free immutable observation. The retained descriptor prevents inode
    /// reuse from making an old version look current after atomic replacement.
    pub fn read(&self) -> Result<RecordSnapshot<T>> {
        self.store.check()?;
        let file = match open_file(&self.path, OFlags::RDONLY, Mode::empty()) {
            Ok(file) => file,
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RecordSnapshot {
                    path: self.path.clone(),
                    version: None,
                    value: None,
                });
            }
            Err(error) => return Err(error),
        };

        let metadata = file
            .metadata()
            .map_err(|error| io_error("inspect record version", &self.path, error))?;
        let value = read_file_json(&file, &self.path, self.limit)?;
        let after = file
            .metadata()
            .map_err(|error| io_error("inspect record version", &self.path, error))?;

        // Atomic replacement can unlink our already-open snapshot while it is
        // read. Its contents remain valid; link-count/ctime changes alone must
        // not make an ordinary observation contend with a writer.
        if !same_contents(&metadata, &after) || metadata.nlink() > 1 || after.nlink() > 1 {
            return Err(Error::ConcurrentUpdate {
                path: self.path.clone(),
            });
        }

        Ok(RecordSnapshot {
            path: self.path.clone(),
            version: Some(Version {
                _file: file,
                metadata,
            }),
            value: Some(value),
        })
    }
}

#[derive(Debug)]
struct Version {
    /// Retained file descriptor preventing inode reuse while a snapshot lives.
    _file: File,
    /// Observed inode identity and content version used for conflict detection.
    metadata: Metadata,
}

/// Observations cannot be deserialized into writer tokens. A snapshot is tied
/// to its resource and exact file version, including an initially absent record.
#[derive(Debug)]
pub struct RecordSnapshot<T> {
    /// Resource path tying this observation to its original record.
    path: PathBuf,
    /// Retained file identity, or absent for an initially missing record.
    version: Option<Version>,
    /// Decoded immutable record contents, or absent if no record existed.
    value: Option<T>,
}

impl<T> RecordSnapshot<T> {
    /// Borrow the decoded snapshot, or return `None` for an observed missing record.
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }
}

/// Exclusive writer capability for one typed record.
///
/// A commit also needs a snapshot of the expected version; holding the lock alone
/// does not authorize overwriting a value observed before another update.
#[derive(Debug)]
pub struct RecordWriter<'store, T> {
    /// Resource-specific record capability held for this writer.
    record: Record<'store, T>,
    /// Kernel lock excluding other writers to this resource.
    lock: OperationLock,
}

impl<T> RecordWriter<'_, T> {
    /// Borrow the guard that keeps other writers out of this record resource.
    pub fn lock(&self) -> &OperationLock {
        &self.lock
    }

    /// Update the lock's diagnostic phase, validating its text and persistent inode.
    pub fn update_phase(&mut self, phase: String) -> Result<()> {
        self.lock.update_phase(phase)
    }

    fn check_expected(&self, expected: &RecordSnapshot<T>) -> Result<()> {
        self.record.store.check()?;
        self.lock.check_path()?;
        if expected.path != self.record.path {
            return Err(Error::BindingMismatch {
                field: "state record resource",
            });
        }

        let current = match fs::symlink_metadata(&self.record.path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(io_error(
                    "inspect current record version",
                    &self.record.path,
                    error,
                ));
            }
        };

        let matches = match (&expected.version, current) {
            (None, None) => true,
            (Some(version), Some(current)) => {
                current.is_file()
                    && current.nlink() == 1
                    && same_version(&version.metadata, &current)
            }
            _ => false,
        };
        if !matches {
            return Err(Error::ConcurrentUpdate {
                path: self.record.path.clone(),
            });
        }

        Ok(())
    }
}

impl<T: DeserializeOwned> RecordWriter<'_, T> {
    /// Read a fresh snapshot while retaining this resource's writer exclusion.
    pub fn read(&self) -> Result<RecordSnapshot<T>> {
        self.lock.check_path()?;
        self.record.read()
    }
}

impl<T: Serialize> RecordWriter<'_, T> {
    /// Compare-and-publish under the resource lock, rechecking immediately
    /// before rename. On conflict, reread and reconsider the update; do not
    /// blindly retry an old value against a fresh token.
    pub fn commit(
        &mut self,
        expected: &RecordSnapshot<T>,
        value: &T,
        permissions: RecordPermissions,
    ) -> Result<()> {
        self.check_expected(expected)?;

        publish_json(
            &self.record.path,
            value,
            permissions,
            self.record.limit,
            || self.check_expected(expected),
        )?;

        Ok(())
    }
}

fn same_version(left: &Metadata, right: &Metadata) -> bool {
    (
        left.dev(),
        left.ino(),
        left.len(),
        left.mtime(),
        left.mtime_nsec(),
        left.ctime(),
        left.ctime_nsec(),
    ) == (
        right.dev(),
        right.ino(),
        right.len(),
        right.mtime(),
        right.mtime_nsec(),
        right.ctime(),
        right.ctime_nsec(),
    )
}

fn same_contents(left: &Metadata, right: &Metadata) -> bool {
    (
        left.dev(),
        left.ino(),
        left.len(),
        left.mtime(),
        left.mtime_nsec(),
    ) == (
        right.dev(),
        right.ino(),
        right.len(),
        right.mtime(),
        right.mtime_nsec(),
    )
}
