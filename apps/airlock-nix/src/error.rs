use crate::identity::VmName;
use crate::state::LockHolder;
use serde::Serialize;
use std::{
    io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, Error>;

/// Errors remain distinguishable until the CLI/protocol boundary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid {field}: expected {expected}")]
    InvalidField {
        field: &'static str,
        expected: &'static str,
    },
    #[error("review differs from {field}")]
    BindingMismatch { field: &'static str },
    #[error("stale review: deployed baseline changed")]
    StaleReview,
    #[error("VM impact exceeds approval: {}", display_names(.names))]
    ScopeExceeded { names: Vec<VmName> },
    #[error("reboot requirement exceeds the recorded approval")]
    RebootNotApproved,
    #[error("invalid controller job: {reason}")]
    InvalidJob { reason: &'static str },
    #[error("operation lock is held: {}: {holder}", path.display())]
    Busy { path: PathBuf, holder: LockHolder },
    #[error("record changed since it was observed: {}", path.display())]
    ConcurrentUpdate { path: PathBuf },
    #[error("state directory is writable by others or its identity changed: {}", path.display())]
    InvalidStateDirectory { path: PathBuf },
    #[error("expected a regular file: {}", path.display())]
    NotRegularFile { path: PathBuf },
    #[error("operation lock was unlinked, replaced or has multiple hard links: {}", path.display())]
    LockReplaced { path: PathBuf },
    #[error("{operation} {}: {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid JSON record {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot serialize record {}: {source}", path.display())]
    Serialization {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("record {} was published, but directory durability is unconfirmed: {source}", path.display())]
    PublishedButNotDurable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("record {} exceeds the {limit}-byte limit", path.display())]
    RecordTooLarge { path: PathBuf, limit: u64 },
}

impl Error {
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

#[derive(Debug, Serialize)]
pub struct ErrorReport {
    schema: &'static str,
    status: &'static str,
    code: &'static str,
    error: String,
    details: ErrorDetails,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ErrorDetails {
    InvalidField {
        field: &'static str,
        expected: &'static str,
    },
    BindingMismatch {
        field: &'static str,
    },
    StaleReview,
    ScopeExceeded {
        names: Vec<VmName>,
    },
    RebootNotApproved,
    InvalidJob {
        reason: &'static str,
    },
    Path {
        path: DiagnosticPath,
    },
    Busy {
        path: DiagnosticPath,
        holder: LockHolder,
    },
    Io {
        operation: &'static str,
        path: DiagnosticPath,
        os_error: Option<i32>,
        error_kind: String,
    },
    Json {
        path: DiagnosticPath,
        line: usize,
        column: usize,
    },
    Publication {
        path: DiagnosticPath,
        os_error: Option<i32>,
        published: bool,
        durability: Durability,
    },
    RecordTooLarge {
        path: DiagnosticPath,
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
    display: String,
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
