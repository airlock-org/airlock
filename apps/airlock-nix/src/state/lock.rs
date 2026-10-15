use super::{open_file, parse_json};
use crate::{
    Error, Result,
    error::io_error,
    identity::{JobId, ReviewId},
    wire::string_enum,
};
use rustix::fs::{Mode, OFlags, major, minor};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom},
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    },
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

string_enum! {
    pub enum OperationKind ("operation kind") {
        SourceImport => "source_import", Review => "review", Build => "build",
        Deployment => "deployment", Recovery => "recovery", Maintenance => "maintenance",
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "OwnerWire", into = "OwnerWire")]
pub struct OperationOwner {
    name: String,
    reason: String,
    operation: OperationKind,
    job_id: Option<JobId>,
    review_id: Option<ReviewId>,
    phase: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerWire {
    name: String,
    reason: String,
    operation: OperationKind,
    job_id: Option<JobId>,
    review_id: Option<ReviewId>,
    phase: Option<String>,
}

fn validate_text(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(Error::InvalidField {
            field: "lock owner/reason/phase",
            expected: "nonblank text up to 1024 bytes, without control characters",
        });
    }
    Ok(())
}

impl OperationOwner {
    pub fn new(name: String, reason: String, operation: OperationKind) -> Result<Self> {
        validate_text(&name)?;
        validate_text(&reason)?;
        Ok(Self {
            name,
            reason,
            operation,
            job_id: None,
            review_id: None,
            phase: None,
        })
    }
    pub fn with_job(mut self, id: JobId) -> Self {
        self.job_id = Some(id);
        self
    }
    pub fn with_review(mut self, id: ReviewId) -> Self {
        self.review_id = Some(id);
        self
    }
    pub fn with_phase(mut self, phase: String) -> Result<Self> {
        validate_text(&phase)?;
        self.phase = Some(phase);
        Ok(self)
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
    pub fn phase(&self) -> Option<&str> {
        self.phase.as_deref()
    }
}

impl TryFrom<OwnerWire> for OperationOwner {
    type Error = Error;
    fn try_from(value: OwnerWire) -> Result<Self> {
        let mut owner = Self::new(value.name, value.reason, value.operation)?;
        owner.job_id = value.job_id;
        owner.review_id = value.review_id;
        if let Some(phase) = value.phase {
            owner = owner.with_phase(phase)?;
        }
        Ok(owner)
    }
}

impl From<OperationOwner> for OwnerWire {
    fn from(value: OperationOwner) -> Self {
        Self {
            name: value.name,
            reason: value.reason,
            operation: value.operation,
            job_id: value.job_id,
            review_id: value.review_id,
            phase: value.phase,
        }
    }
}

/// Descriptive observations only: never an authorization or a lease.
#[derive(Clone, Debug, Serialize)]
pub struct LockOwnerDetails {
    owner: OperationOwner,
    pid: u32,
    acquired_unix_seconds: u64,
}

impl LockOwnerDetails {
    pub fn owner(&self) -> &OperationOwner {
        &self.owner
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn acquired_unix_seconds(&self) -> u64 {
        self.acquired_unix_seconds
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "verification", content = "details", rename_all = "snake_case")]
pub enum LockHolder {
    Verified(Box<LockOwnerDetails>),
    Unknown,
}

impl fmt::Display for LockHolder {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verified(details) => {
                write!(
                    output,
                    "{}: {} (PID {})",
                    details.owner.name, details.owner.reason, details.pid
                )?;
                if let Some(phase) = &details.owner.phase {
                    write!(output, "; phase {phase}")?;
                }
                Ok(())
            }
            Self::Unknown => output.write_str("owner details unavailable or unverifiable"),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LockObservation {
    Unlocked,
    Held {
        holder: LockHolder,
    },
    /// The kernel table was unavailable; this must not be interpreted as free.
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerRecord {
    schema: String,
    owner: OperationOwner,
    pid: u32,
    process_start: u64,
    boot_id: String,
    lock_fd: i32,
    lock_position: u64,
    acquired_unix_seconds: u64,
}

/// Nonblocking kernel exclusion. Only a StateStore can select the path. Its
/// private, non-clonable descriptor is released on drop and closed across exec.
#[derive(Debug)]
#[must_use = "the operation lock is released when its guard is dropped"]
pub struct OperationLock {
    file: File,
    path: PathBuf,
    owner: OperationOwner,
    record: Option<OwnerRecord>,
    diagnostic_error: Option<String>,
}

impl OperationLock {
    pub(super) fn try_acquire(path: &Path, owner: OperationOwner) -> Result<Self> {
        let file = open_file(path, OFlags::RDWR | OFlags::CREATE, Mode::RUSR | Mode::WUSR)?;
        match file.try_lock() {
            Ok(()) => {
                let mut guard = Self {
                    file,
                    path: path.to_owned(),
                    owner,
                    record: None,
                    diagnostic_error: None,
                };
                guard.check_path()?;
                // Diagnostic failure must never turn successful exclusion into
                // an operation failure. A fresh descriptor starts at position
                // zero, which cannot verify an old record's nonzero nonce.
                match guard.identify() {
                    Ok(record) => {
                        guard.record = Some(record);
                        guard.publish_owner();
                    }
                    Err(error) => guard.diagnostic_error = Some(error.to_string()),
                }
                Ok(guard)
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                let holder = match Self::observe(path) {
                    Ok(LockObservation::Held { holder }) => holder,
                    _ => LockHolder::Unknown,
                };
                Err(Error::Busy {
                    path: path.to_owned(),
                    holder,
                })
            }
            Err(std::fs::TryLockError::Error(error)) => {
                Err(io_error("acquire operation lock", path, error))
            }
        }
    }

    fn identify(&mut self) -> std::io::Result<OwnerRecord> {
        let mut random = [0_u8; 4];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        // Keep this within ordinary filesystem seek limits; no file allocation
        // occurs because diagnostic writes use positional I/O at offset zero.
        let position = u64::from(u32::from_ne_bytes(random)) + 1;
        self.file.seek(SeekFrom::Start(position))?;
        let pid = std::process::id();
        Ok(OwnerRecord {
            schema: "airlock.nix-lock-owner.v1".to_owned(),
            owner: self.owner.clone(),
            pid,
            process_start: process_start(pid)
                .ok_or_else(|| std::io::Error::other("process identity unavailable"))?,
            boot_id: boot_id().ok_or_else(|| std::io::Error::other("boot identity unavailable"))?,
            lock_fd: self.file.as_raw_fd(),
            lock_position: position,
            acquired_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)?
                .as_secs(),
        })
    }

    fn publish_owner(&mut self) {
        let Some(record) = &self.record else {
            return;
        };
        let result = (|| -> std::io::Result<()> {
            let bytes = serde_json::to_vec(record)?;
            // Never rename/unlink a lock file. Positional writes preserve the
            // acquisition nonce in the kernel descriptor's seek position.
            // Readers may see an incomplete diagnostic and report Unknown.
            self.file.set_len(0)?;
            self.file.write_all_at(&bytes, 0)?;
            Ok(())
        })();
        self.diagnostic_error = result.err().map(|error| error.to_string());
    }

    /// Phase publication is best effort. Only invalid input or loss of the
    /// actual lock namespace fails the operation; diagnostic_error reports I/O.
    pub fn update_phase(&mut self, phase: String) -> Result<()> {
        validate_text(&phase)?;
        self.check_path()?;
        self.owner.phase = Some(phase.clone());
        if let Some(record) = &mut self.record {
            record.owner.phase = Some(phase);
        }
        self.publish_owner();
        Ok(())
    }

    /// Read-only kernel/owner inspection: no file creation, repair or lock probe.
    /// Linux procfs must expose the holder in this PID namespace. Unavailable
    /// procfs or stale metadata never establishes an unlocked/verified owner.
    pub(super) fn observe(path: &Path) -> Result<LockObservation> {
        let file = match open_file(path, OFlags::RDONLY, Mode::empty()) {
            Ok(file) => file,
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LockObservation::Unlocked);
            }
            Err(error) => return Err(error),
        };
        let metadata = file
            .metadata()
            .map_err(|error| io_error("inspect lock", path, error))?;
        check_named(&file, path)?;
        let Some(held) = kernel_holder(&metadata) else {
            return Ok(LockObservation::Unknown);
        };
        let Some(pid) = held else {
            return Ok(LockObservation::Unlocked);
        };
        let holder = verify_owner(&file, path, &metadata, pid)
            .map_or(LockHolder::Unknown, |details| {
                LockHolder::Verified(Box::new(details))
            });
        Ok(LockObservation::Held { holder })
    }

    pub fn check_path(&self) -> Result<()> {
        check_named(&self.file, &self.path)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn owner(&self) -> &OperationOwner {
        &self.owner
    }
    pub fn diagnostic_error(&self) -> Option<&str> {
        self.diagnostic_error.as_deref()
    }
}

fn check_named(file: &File, path: &Path) -> Result<()> {
    let held = file
        .metadata()
        .map_err(|error| io_error("inspect held lock", path, error))?;
    let named = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::LockReplaced {
                path: path.to_owned(),
            });
        }
        Err(error) => return Err(io_error("inspect lock path", path, error)),
    };
    if !named.is_file()
        || held.nlink() != 1
        || (held.dev(), held.ino()) != (named.dev(), named.ino())
    {
        return Err(Error::LockReplaced {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn bounded_text(path: &Path, limit: u64) -> Option<String> {
    let file = open_file(path, OFlags::RDONLY, Mode::empty()).ok()?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit)
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}

fn process_start(pid: u32) -> Option<u64> {
    let stat = bounded_text(Path::new(&format!("/proc/{pid}/stat")), 8192)?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

fn boot_id() -> Option<String> {
    let boot = bounded_text(Path::new("/proc/sys/kernel/random/boot_id"), 128)?;
    let boot = boot.trim();
    (!boot.is_empty()).then(|| boot.to_owned())
}

fn matches_inode(text: &str, metadata: &Metadata) -> bool {
    let fields: Vec<_> = text.split(':').collect();
    fields.len() == 3
        && u32::from_str_radix(fields[0], 16).ok() == Some(major(metadata.dev()))
        && u32::from_str_radix(fields[1], 16).ok() == Some(minor(metadata.dev()))
        && fields[2].parse::<u64>().ok() == Some(metadata.ino())
}

// Outer None: visibility unavailable; inner None: no kernel flock on this inode.
fn kernel_holder(metadata: &Metadata) -> Option<Option<u32>> {
    let locks = bounded_text(Path::new("/proc/locks"), 4 * 1024 * 1024)?;
    for line in locks.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() == 8 && fields[1] == "FLOCK" && matches_inode(fields[5], metadata) {
            // Multiple shared holders cannot be identified as one named writer.
            return Some(Some(if fields[3] == "WRITE" {
                fields[4].parse().ok()?
            } else {
                0
            }));
        }
    }
    Some(None)
}

fn verify_owner(
    file: &File,
    path: &Path,
    metadata: &Metadata,
    pid: u32,
) -> Option<LockOwnerDetails> {
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    let record: OwnerRecord = parse_json(&bytes, path).ok()?;
    if pid == 0
        || record.pid != pid
        || record.schema != "airlock.nix-lock-owner.v1"
        || record.lock_fd < 0
        || record.lock_position == 0
        || process_start(pid)? != record.process_start
        || boot_id()? != record.boot_id
    {
        return None;
    }
    let descriptor = PathBuf::from(format!("/proc/{pid}/fd/{}", record.lock_fd));
    let actual = fs::metadata(descriptor).ok()?;
    if (actual.dev(), actual.ino()) != (metadata.dev(), metadata.ino()) {
        return None;
    }
    let info = bounded_text(
        Path::new(&format!("/proc/{pid}/fdinfo/{}", record.lock_fd)),
        65536,
    )?;
    let position: u64 = info
        .lines()
        .find_map(|line| line.strip_prefix("pos:"))?
        .trim()
        .parse()
        .ok()?;
    if position != record.lock_position {
        return None;
    }
    let held = info
        .lines()
        .filter_map(|line| line.strip_prefix("lock:"))
        .any(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            fields.len() == 8
                && fields[1] == "FLOCK"
                && fields[3] == "WRITE"
                && fields[4].parse::<u32>().ok() == Some(pid)
                && matches_inode(fields[5], metadata)
        });
    if !held || process_start(pid)? != record.process_start || check_named(file, path).is_err() {
        return None;
    }
    Some(LockOwnerDetails {
        owner: record.owner,
        pid,
        acquired_unix_seconds: record.acquired_unix_seconds,
    })
}
