//! Source attribution for authored Rust operations.
use serde::{Deserialize, Serialize};
use std::panic::Location;

/// Where a Rust operation was written, and what it was doing.
/// Rusix includes this information in generated output so Nix evaluation errors
/// can be linked back to Rust source. This source attribution is also called
/// *provenance*.
///
/// IDs are deterministic for the same file, line, column and purpose, but can
/// change after edits. Reusing an expression can produce several uses of one ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Marker linking generated error traces and text ranges to this Rust operation.
    /// Spelled `rn-` followed by 16 lowercase hexadecimal digits, embedded
    /// unchanged in generated comments, error contexts and source-map metadata.
    pub id: String,
    /// Rust source path as recorded by the caller; it may be relative to the build.
    pub file: String,
    /// One-based source line at which the operation was captured.
    pub line: u32,
    /// One-based source column recorded by Rust's caller tracking.
    pub column: u32,
    /// Operation or configuration path that gives this location semantic meaning.
    pub purpose: String,
}

impl Origin {
    /// Capture the calling Rust operation; tracked helpers forward their caller.
    #[track_caller]
    pub fn caller(purpose: impl Into<String>) -> Self {
        let location = Location::caller();
        Self::new(location.file(), location.line(), location.column(), purpose)
    }

    /// Create source attribution from an explicit Rust location and operation name.
    /// The ID is derived from these inputs; it is not an identifier assigned by Nix.
    pub fn new(file: &str, line: u32, column: u32, purpose: impl Into<String>) -> Self {
        let purpose = purpose.into();
        // Specified FNV-1a, rather than Rust's implementation-dependent DefaultHasher.
        let key = format!("{file}\0{line}\0{column}\0{purpose}");
        let hash = key.bytes().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        Self {
            id: format!("rn-{hash:016x}"),
            file: file.into(),
            line,
            column,
            purpose,
        }
    }
}
