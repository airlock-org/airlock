//! Fixed native launchers keep their trusted command and arguments beside their executable.
//! Shared decoding lets image checks inspect the same contract that runtime launchers consume.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::Path,
};

/// An immutable executable invocation selected by package authoring, never by an RPC argument.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// Executable selected by the trusted package definition.
    pub program: String,
    /// Compatibility override exposed only by administrative tool packages.
    pub program_env: Option<String>,
    /// Fixed arguments; `@self@` and `@prefix@` refer to the original installed package.
    #[serde(default)]
    pub args: Vec<String>,
    /// Exact environment bindings supplied by the package.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Colon-separated prefixes retaining a nonempty inherited suffix.
    #[serde(default)]
    pub prefix_env: BTreeMap<String, String>,
    /// Environment bindings intentionally removed at this boundary.
    #[serde(default)]
    pub unset_env: Vec<String>,
    /// Whether caller arguments follow the fixed arguments.
    #[serde(default = "yes")]
    pub inherit_args: bool,
    /// Optional exact caller argument count enforced before execution.
    pub argument_count: Option<usize>,
    /// Optional authenticated Qubes caller allowlist; an empty list denies all callers.
    pub callers: Option<Vec<String>>,
    /// Whether a nonempty Qubes RPC argument must be rejected.
    #[serde(default)]
    pub empty_service_argument: bool,
}

/// Forward ordinary CLI arguments unless the package selects a fixed invocation.
fn yes() -> bool {
    true
}

impl Contract {
    /// Read the JSON beside a resolved executable, reporting its path on failure.
    ///
    /// Callers must resolve aliases before using this method. A bounded read rejects
    /// unexpectedly large contracts rather than allocating without a limit.
    pub fn read(executable: &Path) -> io::Result<Self> {
        let mut name = executable
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "launcher has no filename"))?
            .to_os_string();
        name.push(".json");
        let path = executable.with_file_name(name);
        Self::read_path(&path)
    }

    /// Decode a resolved contract path when an offline store view supplies alias resolution.
    pub fn read_path(path: &Path) -> io::Result<Self> {
        let context = |error: io::Error| {
            io::Error::new(
                error.kind(),
                format!(
                    "native command contract {path}: {error}",
                    path = path.display()
                ),
            )
        };
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(context)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(context)?;
        if bytes.len() > 1024 * 1024 {
            return Err(context(io::Error::new(
                io::ErrorKind::InvalidData,
                "contract exceeds 1 MiB",
            )));
        }

        serde_json::from_slice(&bytes)
            .map_err(|error| context(io::Error::new(io::ErrorKind::InvalidData, error)))
    }
}
