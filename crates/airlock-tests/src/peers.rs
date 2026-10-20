//! Temporary command peers provide literal replies and observable effects for Rust integration scenarios.

use crate::process::{binary, json, peer_link, run};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// One private command namespace, its reply data and recorded process invocations.
pub struct Peers {
    // Owns all mutable files until the scenario exits, including on assertion failure.
    temporary: tempfile::TempDir,
    // Commands and literal rules serialized for each child; process environment stays untouched.
    specification: Value,
}

impl Peers {
    /// Create an empty command namespace without installing or building prerequisites.
    pub fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        fs::create_dir(temporary.path().join("bin"))?;
        Ok(Self {
            temporary,
            specification: json!({"commands":{}}),
        })
    }

    /// Resolve data files in the private fixture directory.
    pub fn path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.temporary.path().join(name)
    }

    /// Register a process name with ordered reply rules; unmatched calls fail closed.
    pub fn rules(&mut self, name: &str, rules: Value) -> Result<()> {
        let link = self.path("bin").join(name);
        if !link.exists() {
            peer_link(&self.path("bin"), name)?;
        }
        self.specification["commands"][name] = rules;
        Ok(())
    }

    /// Use the actual native implementation for XML queries even when storage is an inert peer.
    pub fn native_dom0(&mut self) {
        self.specification["native_dom0"] =
            json!(binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
    }

    /// Persist scenario data and configure only this child's command namespace.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Result<Command> {
        let mut command = Command::new(program);
        self.environment(&mut command)?;
        Ok(command)
    }

    /// Persist finite peer data and apply its environment to an already constructed command.
    pub fn environment(&self, command: &mut Command) -> Result<()> {
        let specification = self.path("replies.json");
        let bytes = serde_json::to_vec_pretty(&self.specification)?;
        if fs::read(&specification).ok().as_deref() != Some(bytes.as_slice()) {
            fs::write(&specification, bytes)?;
        }
        command.env("AIRLOCK_TEST_REPLIES", specification).env(
            "PATH",
            format!(
                "{bin}:{path}",
                bin = self.path("bin").display(),
                path = std::env::var("PATH")?
            ),
        );
        Ok(())
    }

    /// Read all process invocations as structured arguments rather than shell-rendered strings.
    pub fn calls(&self) -> Result<Vec<Value>> {
        let path = self.path("calls.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        fs::read_to_string(path)?
            .lines()
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }

    /// Decode a scenario-owned JSON file, preserving its path in diagnostics.
    pub fn read(&self, name: impl AsRef<Path>) -> Result<Value> {
        let path = self.path(name);
        serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("decode {path:?}"))
    }

    /// Execute a prepared command with the common bounded process owner.
    pub fn run(&self, command: &mut Command) -> Result<Vec<u8>> {
        run(command, None)?.checked()
    }

    /// Execute a prepared JSON command with the common bounded process owner.
    pub fn json(&self, command: &mut Command) -> Result<Value> {
        json(command, None)
    }
}
