//! Temporary-file closure admission, with only EROFS indexing replaced by an inert peer.

use super::{binary, json, peer_link, run};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    process::Command,
};

/// One isolated pool and immutable source tree for an admission or transfer scenario.
pub struct Store {
    /// Temporary directory retaining all paths until child processes have exited.
    pub temporary: tempfile::TempDir,
    /// Explicitly built dom0 service under test.
    pub service: PathBuf,
    /// Explicitly built user-facing artifact command.
    pub cli: PathBuf,
    /// Private closure-store destination.
    pub pool: PathBuf,
    /// Physical source corresponding to logical /nix/store paths.
    pub physical: PathBuf,
    /// The declared system's closure manifest and path list.
    pub closure: PathBuf,
    /// Root disk with an ext filesystem signature and sparse payload.
    pub root: PathBuf,
    /// Logical Nix store system identity.
    pub system: String,
    // Inert index executable, never a mount or an EROFS build.
    index: PathBuf,
    // Artifact metadata describing the immutable system source.
    metadata: PathBuf,
}

impl Store {
    /// Prepare ordinary files only; no VM, mount, or host Nix store is modified.
    pub fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        let directory = temporary.path();
        let index = peer_link(directory, "mkfs.erofs")?;
        let physical = directory.join("source-store");
        let system = format!("/nix/store/{hash}-system", hash = "a".repeat(32));
        let package = physical.join(system.rsplit('/').next().unwrap());
        fs::create_dir_all(package.join("pam"))?;
        fs::create_dir(package.join("pam.d"))?;
        fs::write(package.join("init"), b"system fixture")?;
        fs::write(package.join("pam/environment"), b"ordering fixture")?;
        std::os::unix::fs::symlink("init", package.join("link"))?;
        let closure = directory.join("closure");
        fs::create_dir(&closure)?;
        fs::write(closure.join("store-paths"), format!("{system}\n"))?;
        fs::write(closure.join("manifest.json"), b"{}")?;

        let root = directory.join("root.img");
        let mut file = File::create(&root)?;
        file.set_len(16 * 1024 * 1024)?;
        file.seek(SeekFrom::Start(1080))?;
        file.write_all(b"\x53\xef")?;
        file.seek(SeekFrom::Start(2 * 65536))?;
        file.write_all(b"boot fixture")?;
        let metadata = directory.join("path-info.json");
        fs::write(
            &metadata,
            serde_json::to_vec(
                &json!({&system: {"narHash":"sha256:fixture","narSize":1024,"references":[]}}),
            )?,
        )?;
        let pool = directory.join("pool");

        Ok(Self {
            temporary,
            service: binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"),
            cli: binary("AIRLOCK_CLI_BIN", "airlock"),
            pool,
            physical,
            closure,
            root,
            system,
            index,
            metadata,
        })
    }

    /// Apply fixture-local command overrides without changing the test process environment.
    pub fn environment<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("AIRLOCK_MKFS_EROFS", &self.index)
            .env("AIRLOCK_CLOSURE_STORE_INDEX_SOURCE", "file")
    }

    /// Construct a real storage command, leaving failure assertions to the caller.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.service);
        self.environment(&mut command).arg("storage").args(args);
        command
    }

    /// Run and decode a real storage operation with a bounded lifetime.
    pub fn run(&self, args: &[&str]) -> Result<Value> {
        json(&mut self.command(args), None)
    }

    /// Build an admission command so crash failpoints can be scoped to this child only.
    pub fn admission(&self) -> Command {
        self.admission_mode("tree")
    }

    /// Select the real verification boundary independently of the fixture's inert EROFS index.
    pub fn admission_mode(&self, mode: &str) -> Command {
        let mut command = self.command(&["pool", "admit", "--pool-root"]);
        command
            .arg(&self.pool)
            .args([
                "--domain",
                "usr-home-runtime-store",
                "--system",
                &self.system,
                "--closure-source",
            ])
            .arg(&self.closure)
            .arg("--root-source")
            .arg(&self.root)
            .arg("--physical-store-root")
            .arg(&self.physical)
            .args(["--verify-mode", mode, "--retain", "false"]);
        command
    }

    /// Admit and optionally activate one generation from the current source tree.
    pub fn admit(&self, active: bool) -> Result<Value> {
        json(
            self.admission()
                .args(["--activate", if active { "true" } else { "false" }]),
            None,
        )
    }

    /// Construct an artifact command carrying the exact source-review provenance.
    pub fn artifact_command(&self, operation: &str) -> Command {
        let mut command = Command::new(&self.cli);
        self.environment(&mut command)
            .args([
                "artifact",
                operation,
                "--domain",
                "usr-home",
                "--system",
                &self.system,
                "--closure-source",
            ])
            .arg(&self.closure)
            .arg("--root-source")
            .arg(&self.root)
            .arg("--physical-store-root")
            .arg(&self.physical)
            .args([
                "--source-revision",
                &"a".repeat(40),
                "--lock-hash",
                &"b".repeat(64),
                "--path-info-json",
            ])
            .arg(&self.metadata);
        command
    }

    /// Produce the real artifact archive rather than reconstructing its protocol in the test.
    pub fn artifact(&self) -> Result<Vec<u8>> {
        run(&mut self.artifact_command("prepare"), None)?.checked()
    }

    /// Receive an archive through the service's actual stream parser.
    pub fn receiver(&self) -> Command {
        let mut command = self.command(&["artifact", "receive", "--pool-root"]);
        command.arg(&self.pool).args([
            "--expected-domain",
            "usr-home",
            "--approved-by",
            "sys-ctrl",
        ]);
        command
    }

    /// Replace qrexec with a direct exec of the actual incremental pool receiver.
    pub fn sender(&self) -> Result<Command> {
        let path = self.temporary.path().join("qrexec-client");
        if !path.exists() {
            peer_link(self.temporary.path(), "qrexec-client")?;
        }
        let mut command = self.artifact_command("send");
        command
            .arg("--qrexec-client")
            .arg(path)
            .env("AIRLOCK_TEST_RECEIVER", &self.service)
            .env("AIRLOCK_TEST_POOL", &self.pool);
        Ok(command)
    }

    /// Return UTF-8 pool paths for the service's string argument interface.
    pub fn pool_text(&self) -> &str {
        self.pool.to_str().unwrap()
    }
}
