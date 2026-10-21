//! Immutable Nix artifacts distinguish whole store objects from their children.
//! These checks validate spelling, not existence, filesystem type or trust.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// A top-level Nix store object, such as a built system or closure.
///
/// The path contains a 32-character Nix base32 hash and a nonempty store name.
/// Use this for metadata/export operations that require an object root. Use
/// [`StoreArtifact`] when child paths are allowed, or [`StoreFile`] when a child
/// path is required. Construction performs no I/O and grants no admission or
/// activation authority.
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

/// An immutable artifact at a Nix store object root or within that object.
///
/// Use this for closure descriptors, which may be separate store outputs or
/// per-guest directories inside a shared prepared output. Unlike [`StorePath`],
/// this does not establish that the path names a whole object for export or GC.
/// Empty, traversal and control-character child components are rejected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StoreArtifact(
    /// Absolute artifact spelling rooted in a validated Nix store object.
    String,
);

impl StoreArtifact {
    /// Borrow the validated artifact spelling without resolving or reading it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for StoreArtifact {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        let object = value.strip_prefix("/nix/store/").unwrap_or("");
        if object.contains('/') {
            StoreFile::try_from(value.clone())?;
        } else {
            StorePath::try_from(value.clone())?;
        }

        Ok(Self(value))
    }
}

impl FromStr for StoreArtifact {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        value.to_owned().try_into()
    }
}

impl From<StoreArtifact> for String {
    fn from(value: StoreArtifact) -> Self {
        value.0
    }
}

impl fmt::Display for StoreArtifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts_allow_prepared_children_without_weakening_object_paths() {
        let root = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-descriptors";
        let child = format!("{root}/sys-gui");
        for path in [root, child.as_str()] {
            let artifact: StoreArtifact = path.parse().unwrap();
            assert_eq!(artifact.as_str(), path);
            let encoded = serde_json::to_string(&artifact).unwrap();
            assert_eq!(
                serde_json::from_str::<StoreArtifact>(&encoded).unwrap(),
                artifact
            );
        }
        assert!(child.parse::<StorePath>().is_err());
        assert!(root.parse::<StoreFile>().is_err());

        for suffix in [
            "/",
            "//sys-gui",
            "/.",
            "/../sys-gui",
            "/sys-gui/..",
            "/sys\ngui",
        ] {
            assert!(format!("{root}{suffix}").parse::<StoreArtifact>().is_err());
        }
        for path in ["/tmp/descriptors/sys-gui", "/nix/store/invalid/sys-gui"] {
            assert!(path.parse::<StoreArtifact>().is_err());
        }
    }
}
