//! Validated identifiers used to bind dom0 state to a boot, release or generation.

use super::absolute;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroU64, path::Path, str::FromStr};

macro_rules! identity {
    ($(#[$docs:meta])* $name:ident, $description:literal, $valid:expr) => {
        $(#[$docs])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(
            /// Text checked against the identity-specific constraint before construction.
            String,
        );

        impl $name {
            /// Return the validated identity text without normalizing its spelling.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = anyhow::Error;

            fn try_from(value: String) -> Result<Self> {
                ensure!(($valid)(&value), concat!("invalid ", $description));
                Ok(Self(value))
            }
        }

        impl FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(value: &str) -> Result<Self> {
                value.to_owned().try_into()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

identity! {
    /// Host or guest boot UUID used to reject observations from a different boot.
    BootId, "boot ID", |s: &str| s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

identity! {
    /// Absolute release location without parent traversal or control characters.
    ReleasePath, "release path", |s: &str| absolute(s).is_ok()
}

identity! {
    /// Machine configuration token: 1–128 ASCII letters, digits, underscores or hyphens.
    MachineProfile, "machine profile", |s: &str| !s.is_empty()
        && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

identity! {
    /// SHA-256 of the canonical manifest/topology snapshot as 64 lowercase hex digits.
    ReleaseFingerprint, "release fingerprint", |s: &str| s.len() == 64
        && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

identity! {
    /// SHA-256 of protected approval inputs, excluding mutable deployment status.
    ApprovalFingerprint, "approval fingerprint", |s: &str| s.len() == 64
        && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ReleasePath {
    /// Borrow the validated absolute release location as a filesystem path.
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// Reject non-UTF-8 or invalid release paths before constructing the identity.
    pub fn from_path(path: &Path) -> Result<Self> {
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 release path"))?
            .parse()
    }
}

/// Positive storage generation identifier; zero cannot name an admitted guest system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GenerationId(
    /// Positive storage generation number; zero is never a valid generation.
    NonZeroU64,
);

impl FromStr for GenerationId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(s.parse()?))
    }
}

impl fmt::Display for GenerationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
