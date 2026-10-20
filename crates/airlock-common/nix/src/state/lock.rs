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
    /// Purpose category used to explain why an operation holds exclusion.
    pub enum OperationKind ("operation kind") {
        /// Import committed source into the controlled build workspace.
        SourceImport => "source_import",
        /// Present source and capture a human approval decision.
        Review => "review",
        /// Evaluate and build the approved source.
        Build => "build",
        /// Apply an approved, checked release.
        Deployment => "deployment",
        /// Continue or roll back a recorded deployment.
        Recovery => "recovery",
        /// Perform serialized storage or state maintenance.
        Maintenance => "maintenance",
    }
}

/// Named entity and reason for exclusive work, with optional job and review context.
///
/// This describes a kernel lock holder; it does not itself acquire exclusion.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "OwnerWire", into = "OwnerWire")]
pub struct OperationOwner {
    /// Human-readable entity responsible for holding this operation lock.
    name: String,
    /// Explanation of why this operation needs exclusive access.
    reason: String,
    /// Kind of work for which the named owner is holding exclusion.
    operation: OperationKind,
    /// Controller job associated with the operation, if available.
    job_id: Option<JobId>,
    /// Human review associated with the operation, if available.
    review_id: Option<ReviewId>,
    /// Latest reported phase of the operation; diagnostic rather than authorization.
    phase: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerWire {
    /// Human-readable entity responsible for holding this operation lock.
    name: String,
    /// Explanation of why this operation needs exclusive access.
    reason: String,
    /// Kind of work for which the named owner is holding exclusion.
    operation: OperationKind,
    /// Controller job associated with the operation, if available.
    job_id: Option<JobId>,
    /// Human review associated with the operation, if available.
    review_id: Option<ReviewId>,
    /// Latest reported phase of the operation; diagnostic rather than authorization.
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
    /// Reject blank, control-containing or over-1024-byte owner/reason text.
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

    /// Attach a controller job identity to the owner diagnostic.
    pub fn with_job(mut self, id: JobId) -> Self {
        self.job_id = Some(id);
        self
    }

    /// Attach a protected review identity to the owner diagnostic.
    pub fn with_review(mut self, id: ReviewId) -> Self {
        self.review_id = Some(id);
        self
    }

    /// Attach a nonblank phase of at most 1024 bytes without control characters.
    pub fn with_phase(mut self, phase: String) -> Result<Self> {
        self.set_phase(phase)?;
        Ok(self)
    }

    pub(super) fn set_phase(&mut self, phase: String) -> Result<()> {
        validate_text(&phase)?;
        self.phase = Some(phase);
        Ok(())
    }

    /// Return the human-readable entity responsible for this operation.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return why this operation needs exclusion.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Return the category of work holding the lock.
    pub fn operation(&self) -> OperationKind {
        self.operation
    }

    /// Return the associated controller job, if known.
    pub fn job_id(&self) -> Option<&JobId> {
        self.job_id.as_ref()
    }

    /// Return the associated human review, if known.
    pub fn review_id(&self) -> Option<&ReviewId> {
        self.review_id.as_ref()
    }

    /// Return the latest reported operation phase, if known.
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
    /// Validated named entity, purpose and optional job/review context.
    owner: OperationOwner,
    /// Process identity checked against the current boot and process start time.
    pid: u32,
    /// Wall-clock acquisition time in seconds since the Unix epoch.
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

    /// Borrow the validated entity and purpose matched to the kernel holder.
    pub fn owner(&self) -> &OperationOwner {
        &self.owner
    }

    /// Return the process ID matched against boot and process start identity.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Return acquisition time as seconds since the Unix epoch.
    pub fn acquired_unix_seconds(&self) -> u64 {
        self.acquired_unix_seconds
    }
}

/// Owner information accompanying a contended kernel lock, never authority to proceed.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "verification", content = "details", rename_all = "snake_case")]
pub enum LockHolder {
    /// Metadata matched the live holder, including its descriptor and acquisition nonce.
    Verified(
        /// Owner metadata matched against the live kernel lock acquisition.
        Box<LockOwnerDetails>,
    ),
    /// The lock is held, but its owner metadata could not be verified.
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

/// Read-only kernel exclusion state, distinguishing unavailable inspection from unlocked.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LockObservation {
    /// No current kernel holder was observed, or the lock file does not exist.
    Unlocked,
    /// Kernel exclusion is present; owner details may still be unknown.
    Held {
        /// Verified owner or an explicit unknown holder; never inferred from stale JSON.
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
    /// Open descriptor holding kernel exclusion until the guard is dropped.
    file: File,
    /// Stable lock-file name checked against the held descriptor.
    path: PathBuf,
    /// Advisory owner identity and any publication error.
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

    /// Reject a removed, replaced, non-regular or multiply linked lock path.
    pub fn check_path(&self) -> Result<()> {
        check_named(&self.file, &self.path)
    }

    /// Return the reserved lock-file path; callers must not unlink or replace it.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Borrow this guard's named operation, even if publishing diagnostics failed.
    pub fn owner(&self) -> &OperationOwner {
        self.diagnostics.owner()
    }

    /// Return the last owner-identification or publication failure, if any.
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
