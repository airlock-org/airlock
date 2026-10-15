use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

// Private strings and fallible serde conversions make validation apply to both
// Rust callers and JSON input. Equal-looking identities remain different types.
macro_rules! hex_identity {
    ($name:ident, $length:literal, $field:literal, $expected:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(value: String) -> Result<Self> {
                if value.len() == $length
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    Ok(Self(value))
                } else {
                    Err(Error::InvalidField {
                        field: $field,
                        expected: $expected,
                    })
                }
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(value: &str) -> Result<Self> {
                Self::try_from(value.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

hex_identity!(
    JobId,
    32,
    "controller job ID",
    "32 lowercase hexadecimal characters"
);
hex_identity!(
    ReviewId,
    64,
    "review ID",
    "64 lowercase hexadecimal characters"
);
hex_identity!(
    GitRevision,
    40,
    "Git revision",
    "40 lowercase hexadecimal characters (Git SHA-1)"
);
hex_identity!(
    LockHash,
    64,
    "flake.lock hash",
    "64 lowercase hexadecimal characters (SHA-256)"
);
hex_identity!(
    ReviewFingerprint,
    64,
    "review fingerprint",
    "64 lowercase hexadecimal characters (SHA-256)"
);
hex_identity!(
    BaselineFingerprint,
    64,
    "baseline fingerprint",
    "64 lowercase hexadecimal characters (SHA-256)"
);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VmName(String);

impl VmName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for VmName {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        // Match dom0's guest-admission DOMAIN grammar, rather than treating
        // arbitrary path components or command options as deployment names.
        if value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && value.len() <= 31
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            Ok(Self(value))
        } else {
            Err(Error::InvalidField {
                field: "VM name",
                expected: "1–31 ASCII characters, starting with a letter; then letters, digits, underscores, dots or hyphens",
            })
        }
    }
}

impl FromStr for VmName {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value.to_owned())
    }
}

impl From<VmName> for String {
    fn from(value: VmName) -> Self {
        value.0
    }
}

impl fmt::Display for VmName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A guest target cannot refer to the privileged dom0 administrative domain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "VmName", into = "VmName")]
pub struct GuestName(VmName);

impl GuestName {
    pub fn as_vm_name(&self) -> &VmName {
        &self.0
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<VmName> for GuestName {
    type Error = Error;

    fn try_from(name: VmName) -> Result<Self> {
        if name.as_str() == "dom0" {
            return Err(Error::InvalidField {
                field: "guest target",
                expected: "a VM name other than dom0",
            });
        }
        Ok(Self(name))
    }
}

impl FromStr for GuestName {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value.parse::<VmName>()?)
    }
}

impl From<GuestName> for VmName {
    fn from(value: GuestName) -> Self {
        value.0
    }
}

impl fmt::Display for GuestName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub revision: GitRevision,
    pub lock_hash: LockHash,
}
