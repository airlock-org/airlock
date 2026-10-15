use super::{linux_owner::OwnerDiagnostics, open_file};
use crate::{
    Error, Result,
    error::io_error,
    identity::{JobId, ReviewId},
    wire::string_enum,
};
use rustix::fs::{Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
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
        self.set_phase(phase)?;
        Ok(self)
    }
    pub(super) fn set_phase(&mut self, phase: String) -> Result<()> {
        validate_text(&phase)?;
        self.phase = Some(phase);
        Ok(())
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
    pub(super) fn new(owner: OperationOwner, pid: u32, acquired_unix_seconds: u64) -> Self {
        Self {
            owner,
            pid,
            acquired_unix_seconds,
        }
    }

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

/// Nonblocking kernel exclusion. Only a StateStore can select the path. Its
/// private, non-clonable descriptor is released on drop and closed across exec.
#[derive(Debug)]
#[must_use = "the operation lock is released when its guard is dropped"]
pub struct OperationLock {
    file: File,
    path: PathBuf,
    diagnostics: OwnerDiagnostics,
}

impl OperationLock {
    pub(super) fn try_acquire(path: &Path, owner: OperationOwner) -> Result<Self> {
        let mut file = open_file(path, OFlags::RDWR | OFlags::CREATE, Mode::RUSR | Mode::WUSR)?;
        match file.try_lock() {
            Ok(()) => {
                check_named(&file, path)?;
                // Identification/publication are best effort after exclusion.
                let mut diagnostics = OwnerDiagnostics::capture(&mut file, owner);
                diagnostics.publish(&file);
                Ok(Self {
                    file,
                    path: path.to_owned(),
                    diagnostics,
                })
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

    /// Invalid phases and a replaced lock path fail; diagnostic I/O is best effort.
    pub fn update_phase(&mut self, phase: String) -> Result<()> {
        self.check_path()?;
        self.diagnostics.update_phase(&self.file, phase)
    }

    pub(super) fn observe(path: &Path) -> Result<LockObservation> {
        super::linux_owner::observe(path)
    }

    pub fn check_path(&self) -> Result<()> {
        check_named(&self.file, &self.path)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn owner(&self) -> &OperationOwner {
        self.diagnostics.owner()
    }
    pub fn diagnostic_error(&self) -> Option<&str> {
        self.diagnostics.error()
    }
}

pub(super) fn check_named(file: &File, path: &Path) -> Result<()> {
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
