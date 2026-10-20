//! Immutable Nix artifacts are named by store-object paths; files within them
//! have a distinct type. These checks validate spelling, not existence or trust.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// A top-level Nix store object, such as a built system or closure.
///
/// The path contains a 32-character Nix base32 hash and a nonempty store name.
/// Use this for metadata/export operations that require an object root; use
/// [`StoreFile`] for an artifact inside an object. Construction performs no I/O
/// and grants no admission or activation authority.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StorePath(
    /// Absolute object-root path with validated hash and store-name characters.
    String,
);

impl StorePath {
    /// Borrow the validated object-root spelling without resolving or reading it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for StorePath {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        let object = value.strip_prefix("/nix/store/").unwrap_or("");
        let (hash, name) = object.split_once('-').unwrap_or(("", ""));
        if hash.len() != 32
            || !hash
                .bytes()
                .all(|b| b"0123456789abcdfghijklmnpqrsvwxyz".contains(&b))
            || name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+._?=-".contains(&b))
        {
            return Err(Error::InvalidField {
                field: "Nix store object",
                expected: "/nix/store/<32-character Nix base32 hash>-<nonempty store name>, without child components",
            });
        }

        Ok(Self(value))
    }
}

impl FromStr for StorePath {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        value.to_owned().try_into()
    }
}

impl From<StorePath> for String {
    fn from(value: StorePath) -> Self {
        value.0
    }
}

impl fmt::Display for StorePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A file path beneath one immutable Nix store object, such as a root image.
///
/// Empty, `.` and `..` child components and control characters are rejected.
/// Construction does not read the file, follow symlinks or establish trust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StoreFile(
    /// Absolute artifact path beneath a validated object, excluding traversal.
    String,
);

impl StoreFile {
    /// Borrow the validated artifact spelling without resolving or reading it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for StoreFile {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        let rest = value.strip_prefix("/nix/store/").unwrap_or("");
        let (object, suffix) = rest.split_once('/').unwrap_or(("", ""));
        StorePath::try_from(format!("/nix/store/{object}"))?;
        if suffix.is_empty()
            || suffix.chars().any(char::is_control)
            || !suffix
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
        {
            return Err(Error::InvalidField {
                field: "Nix store file",
                expected: "a file beneath a valid store object, without empty/traversal components or control characters",
            });
        }

        Ok(Self(value))
    }
}

impl FromStr for StoreFile {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        value.to_owned().try_into()
    }
}

impl From<StoreFile> for String {
    fn from(value: StoreFile) -> Self {
        value.0
    }
}

impl fmt::Display for StoreFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
