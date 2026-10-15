use std::{io, path::PathBuf};

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
    #[error("VM impact exceeds approval: {names}")]
    ScopeExceeded { names: String },
    #[error("reboot requirement exceeds the recorded approval")]
    RebootNotApproved,
    #[error("invalid controller job: {reason}")]
    InvalidJob { reason: &'static str },
    #[error("operation lock is held: {}", path.display())]
    Busy { path: PathBuf },
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
            Self::Io { .. } => "io_error",
            Self::Json { .. } => "invalid_record",
            Self::RecordTooLarge { .. } => "record_too_large",
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
