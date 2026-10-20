//! Shared native test prerequisites and bounded process execution; execution never provisions or builds them.

use airlock_vm_service::support::{self, Input, Output};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

/// Quote one literal argument for an explicitly selected fixture shell.
pub fn quote(value: &str) -> String {
    format!("'{value}'", value = value.replace('\'', "'\"'\"'"))
}

/// Resolve an explicitly selected executable or a sibling of the current Cargo test profile.
pub fn binary(variable: &str, name: &str) -> PathBuf {
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let executable = std::env::current_exe().expect("current test executable");
            let directory = executable.parent().unwrap();
            let profile = if directory.file_name().is_some_and(|name| name == "deps") {
                directory.parent().unwrap()
            } else {
                directory
            };
            profile.join(name)
        });
    assert!(
        path.is_file(),
        "explicitly prepare {name} or set {variable}: {path:?}"
    );
    fs::canonicalize(path).unwrap()
}

/// Give an explicitly prepared inert peer the command name expected by the implementation under test.
pub fn peer_link(directory: &Path, name: &str) -> Result<PathBuf> {
    let path = directory.join(name);
    std::os::unix::fs::symlink(binary("AIRLOCK_TEST_PEER_BIN", "airlock-test-peer"), &path)?;
    Ok(path)
}

/// Run a child with bounded pipes, process-group cleanup and a finite deadline.
pub fn run(command: &mut Command, input: Option<Vec<u8>>) -> Result<Output> {
    support::run(command, input.map(Input::bytes), Duration::from_secs(30))
}

/// Decode a successful process reply, retaining its diagnostics if it fails.
pub fn json(command: &mut Command, input: Option<Vec<u8>>) -> Result<serde_json::Value> {
    serde_json::from_slice(&run(command, input)?.checked()?).context("decode native reply")
}
