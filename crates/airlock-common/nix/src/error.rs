use crate::identity::VmName;
use crate::state::LockHolder;
use serde::Serialize;
use std::{
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

/// Result of validating Airlock records or accessing protected local state.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors remain distinguishable until the CLI/protocol boundary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Input violates a field constraint before it can become a validated value.
    #[error("invalid {field}: expected {expected}")]
    InvalidField {
        /// Protocol field or binding whose value failed validation.
        field: &'static str,
        /// Human-readable constraint the supplied value must satisfy.
        expected: &'static str,
    },
    /// A reply, plan or observation refers to different reviewed material.
    #[error("review differs from {field}")]
    BindingMismatch {
        /// Protocol field or binding whose value failed validation.
        field: &'static str,
    },
    /// The deployed baseline changed after review began.
    #[error("stale review: deployed baseline changed")]
    StaleReview,
    /// The plan affects VMs beyond the approved scope.
    #[error("VM impact exceeds approval: {}", display_names(.names))]
    ScopeExceeded {
        /// VMs whose effects fall outside the recorded human approval.
        names: Vec<VmName>,
    },
    /// The plan may require a reboot that the recorded scope disallows.
    #[error("reboot requirement exceeds the recorded approval")]
    RebootNotApproved,
    /// A saved worker phase conflicts with its durable approval state.
    #[error("invalid controller job: {reason}")]
    InvalidJob {
        /// Why the persisted job state is inconsistent.
        reason: &'static str,
    },
    /// Another operation holds the kernel lock; diagnostics may identify it.
    #[error("operation lock is held: {}: {holder}", path.display())]
    Busy {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Observed kernel lock holder, with verified owner details when available.
        holder: LockHolder,
    },
    /// A record version changed before a conditional update could publish.
    #[error("record changed since it was observed: {}", path.display())]
    ConcurrentUpdate {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
    },
    /// The state namespace is writable by others or changed identity.
    #[error("state directory is writable by others or its identity changed: {}", path.display())]
    InvalidStateDirectory {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
    },
    /// A record or lock path names something other than a regular file.
    #[error("expected a regular file: {}", path.display())]
    NotRegularFile {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
    },
    /// The persistent lock inode was removed, replaced or given extra hard links.
    #[error("operation lock was unlinked, replaced or has multiple hard links: {}", path.display())]
    LockReplaced {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
    },
    /// A filesystem operation failed; its path and underlying error are retained.
    #[error("{operation} {}: {source}", path.display())]
    Io {
        /// Filesystem action being attempted when the error occurred.
        operation: &'static str,
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Underlying I/O or JSON error retained for diagnostic context.
        #[source]
        source: io::Error,
    },
    /// Stored bytes are invalid JSON or fail the requested record schema.
    #[error("invalid JSON record {}: {source}", path.display())]
    Json {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Underlying I/O or JSON error retained for diagnostic context.
        #[source]
        source: serde_json::Error,
    },
    /// The new value could not be encoded as an unambiguous record.
    #[error("cannot serialize record {}: {source}", path.display())]
    Serialization {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Underlying I/O or JSON error retained for diagnostic context.
        #[source]
        source: serde_json::Error,
    },
    /// The new record is visible, but directory persistence could not be confirmed.
    #[error("record {} was published, but directory durability is unconfirmed: {source}", path.display())]
    PublishedButNotDurable {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Underlying I/O or JSON error retained for diagnostic context.
        #[source]
        source: io::Error,
    },
    /// Reading or encoding the record exceeded its configured byte budget.
    #[error("record {} exceeds the {limit}-byte limit", path.display())]
    RecordTooLarge {
        /// Filesystem location involved in the failed state operation.
        path: PathBuf,
        /// Maximum permitted record size in bytes.
        limit: u64,
    },
}

impl Error {
    /// Return a stable machine-readable failure category for client branching.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidField { .. } => "invalid_field",
            Self::BindingMismatch { .. } => "binding_mismatch",
            Self::StaleReview => "stale_review",
            Self::ScopeExceeded { .. } | Self::RebootNotApproved => "scope_exceeded",
            Self::InvalidJob { .. } => "invalid_job",
            Self::Busy { .. } => "busy",
            Self::ConcurrentUpdate { .. } => "concurrent_update",
            Self::InvalidStateDirectory { .. } => "invalid_state_directory",
            Self::NotRegularFile { .. } => "invalid_file",
            Self::LockReplaced { .. } => "lock_replaced",
            Self::Io { .. } => "io_error",
            Self::Json { .. } => "invalid_record",
            Self::Serialization { .. } => "serialization_error",
            Self::PublishedButNotDurable { .. } => "durability_unconfirmed",
            Self::RecordTooLarge { .. } => "record_too_large",
        }
    }

    /// Preserve machine-readable context alongside the human diagnostic.
    pub fn report(&self) -> ErrorReport {
        let details = match self {
            Self::InvalidField { field, expected } => {
                ErrorDetails::InvalidField { field, expected }
            }
            Self::BindingMismatch { field } => ErrorDetails::BindingMismatch { field },
            Self::StaleReview => ErrorDetails::StaleReview,
            Self::ScopeExceeded { names } => ErrorDetails::ScopeExceeded {
                names: names.clone(),
            },
            Self::RebootNotApproved => ErrorDetails::RebootNotApproved,
            Self::InvalidJob { reason } => ErrorDetails::InvalidJob { reason },
            Self::Busy { path, holder } => ErrorDetails::Busy {
                path: path.as_path().into(),
                holder: holder.clone(),
            },
            Self::ConcurrentUpdate { path }
            | Self::InvalidStateDirectory { path }
            | Self::NotRegularFile { path }
            | Self::LockReplaced { path } => ErrorDetails::Path {
                path: path.as_path().into(),
            },
            Self::Io {
                operation,
                path,
                source,
            } => ErrorDetails::Io {
                operation,
                path: path.as_path().into(),
                os_error: source.raw_os_error(),
                error_kind: source.kind().to_string(),
            },
            Self::Json { path, source } | Self::Serialization { path, source } => {
                ErrorDetails::Json {
                    path: path.as_path().into(),
                    line: source.line(),
                    column: source.column(),
                }
            }
            Self::PublishedButNotDurable { path, source } => ErrorDetails::Publication {
                path: path.as_path().into(),
                os_error: source.raw_os_error(),
                published: true,
                durability: Durability::Unconfirmed,
            },
            Self::RecordTooLarge { path, limit } => ErrorDetails::RecordTooLarge {
                path: path.as_path().into(),
                limit_bytes: *limit,
            },
        };

        ErrorReport {
            schema: "airlock.nix-error.v1",
            status: "failed",
            code: self.code(),
            error: self.to_string(),
            details,
        }
    }
}

fn display_names(names: &[VmName]) -> String {
    names
        .iter()
        .map(VmName::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Serializable diagnostic envelope with stable classification and failure-specific context.
///
/// Construct it with [`Error::report`] to preserve paths and underlying error details.
#[derive(Debug, Serialize)]
pub struct ErrorReport {
    /// Wire format identifier used to reject unsupported record versions.
    schema: &'static str,
    /// Failure marker for machine-readable error consumers.
    status: &'static str,
    /// Stable error classification suitable for client branching.
    code: &'static str,
    /// Human-readable diagnostic preserving context from the underlying error.
    error: String,
    /// Structured context appropriate to this failure category.
    details: ErrorDetails,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ErrorDetails {
    InvalidField {
        /// Protocol field or binding whose value failed validation.
        field: &'static str,
        /// Human-readable constraint the supplied value must satisfy.
        expected: &'static str,
    },
    BindingMismatch {
        /// Protocol field or binding whose value failed validation.
        field: &'static str,
    },
    StaleReview,
    ScopeExceeded {
        /// VMs whose effects fall outside the recorded human approval.
        names: Vec<VmName>,
    },
    RebootNotApproved,
    InvalidJob {
        /// Why the persisted job state is inconsistent.
        reason: &'static str,
    },
    Path {
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
    },
    Busy {
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
        /// Observed kernel lock holder, with verified owner details when available.
        holder: LockHolder,
    },
    Io {
        /// Filesystem action being attempted when the error occurred.
        operation: &'static str,
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
        /// Platform errno when the underlying failure supplies one.
        os_error: Option<i32>,
        /// Portable I/O classification for consumers without platform errno support.
        error_kind: String,
    },
    Json {
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
        /// One-based JSON line number associated with the error.
        line: usize,
        /// JSON column reported by serde for locating the invalid data.
        column: usize,
    },
    Publication {
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
        /// Platform errno when the underlying failure supplies one.
        os_error: Option<i32>,
        /// True when atomic rename succeeded before durability confirmation failed.
        published: bool,
        /// Explicitly unconfirmed directory persistence after publication.
        durability: Durability,
    },
    RecordTooLarge {
        /// Filesystem location involved in the failed state operation.
        path: DiagnosticPath,
        /// Maximum permitted serialized record size in bytes.
        limit_bytes: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Durability {
    Unconfirmed,
}

/// Diagnostic text is readable even for non-UTF-8 filenames. Raw bytes are
/// supplied in that case so the displayed replacement characters lose no identity.
#[derive(Debug, Serialize)]
struct DiagnosticPath {
    /// Lossy UTF-8 display text usable in human diagnostics.
    display: String,
    /// Exact filename bytes included when display text cannot preserve identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_bytes: Option<Vec<u8>>,
}

impl From<&Path> for DiagnosticPath {
    fn from(path: &Path) -> Self {
        Self {
            display: path.to_string_lossy().into_owned(),
            raw_bytes: path
                .to_str()
                .is_none()
                .then(|| path.as_os_str().as_bytes().to_vec()),
        }
    }
}

pub(crate) fn io_error(
    operation: &'static str,
    path: &std::path::Path,
    source: io::Error,
) -> Error {
    Error::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}
