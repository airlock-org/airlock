//! Shared inert process boundaries; test execution never builds or provisions prerequisites.

pub mod store;

pub mod peers;

pub mod desktop;

use airlock_vm_service::support::{self, Input, Output};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// Locate the checked-out sources independently of the invocation directory.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}

/// Resolve an explicitly installed host tool without invoking a shell or provisioning it.
pub fn tool(name: &str) -> Result<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").context("PATH is unset")?)
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
        .with_context(|| format!("explicitly prepare {name}"))
}

/// Run a privileged command in a disposable user namespace with only its Airlock state writable.
pub fn namespace(command: &Command, fixture: &Path) -> Result<Command> {
    namespace_tmpfs(command, fixture, &[])
}

/// Add explicitly requested scratch filesystems before running a command in the private namespace.
pub fn namespace_tmpfs(command: &Command, fixture: &Path, mounts: &[&str]) -> Result<Command> {
    let source = fixture
        .to_str()
        .context("non-UTF-8 namespace fixture")?
        .trim_end_matches('/');
    let translate = |text: &std::ffi::OsStr| -> Result<String> {
        Ok(text
            .to_str()
            .context("non-UTF-8 namespace argument")?
            .replace(source, "/var/lib/airlock"))
    };
    let mut isolated = Command::new(tool("bwrap")?);
    isolated
        .args([
            "--die-with-parent",
            "--unshare-user",
            "--uid",
            "0",
            "--gid",
            "0",
            "--ro-bind",
            "/",
            "/",
            "--tmpfs",
            "/tmp",
            "--tmpfs",
            "/var",
            "--dir",
            "/var/lib",
            "--bind",
        ])
        .arg(fixture)
        .args([
            "/var/lib/airlock",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--setenv",
            "TMPDIR",
            "/tmp",
        ]);
    for (key, value) in command.get_envs() {
        match value {
            Some(value) => {
                isolated.env(key, translate(value)?);
            }
            None => {
                isolated.env_remove(key);
            }
        }
    }
    for mount in mounts {
        isolated.args(["--tmpfs", mount]);
    }
    isolated.arg(translate(command.get_program())?);
    for argument in command.get_args() {
        isolated.arg(translate(argument)?);
    }
    if let Some(directory) = command.get_current_dir() {
        isolated.current_dir(directory);
    }
    Ok(isolated)
}

/// Locate an explicitly prepared executable beside this Cargo test's profile directory.
pub fn binary(variable: &str, name: &str) -> PathBuf {
    let path = std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(name)
        });
    assert!(
        path.is_file(),
        "explicitly prepare {name} or set {variable}: {path:?}"
    );
    fs::canonicalize(path).unwrap()
}

/// The test-only peer is built by Cargo as part of this package's integration targets.
pub fn peer() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_airlock-test-peer"))
}

/// Give a peer the command name expected by the implementation under test.
pub fn peer_link(directory: &Path, name: &str) -> Result<PathBuf> {
    let path = directory.join(name);
    std::os::unix::fs::symlink(peer(), &path)?;
    Ok(path)
}

/// Run a child with bounded pipes, process-group cleanup and a finite deadline.
pub fn run(command: &mut Command, input: Option<Vec<u8>>) -> Result<Output> {
    support::run(command, input.map(Input::bytes), Duration::from_secs(30))
}

/// Supply a complete archive as a regular file so an early rejection can retain its real exit status and diagnostics.
pub fn run_file(command: &mut Command, bytes: &[u8]) -> Result<Output> {
    use std::io::{Seek, Write};

    let temporary = tempfile::tempdir()?;
    let mut input = tempfile::tempfile()?;
    input.write_all(bytes)?;
    input.rewind()?;
    let mut job = airlock_tests::system::Job::spawn_input(command, temporary.path(), input.into())?;
    let status = job.wait(Duration::from_secs(30))?;
    Ok(Output {
        status,
        stdout: fs::read(temporary.path().join("stdout.log"))?,
        stderr: fs::read(temporary.path().join("stderr.log"))?,
    })
}

/// Decode a successful process reply and retain its diagnostics on failure.
pub fn json(command: &mut Command, input: Option<Vec<u8>>) -> Result<serde_json::Value> {
    serde_json::from_slice(&run(command, input)?.checked()?).context("decode native reply")
}

/// Poll observable state, with a bounded deadline instead of a fixed scheduling assumption.
pub fn until(mut ready: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if ready()? {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for fixture state"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Install a copied launcher and a fixture-owned contract without invoking package builders.
pub fn launcher(directory: &Path, contract: &serde_json::Value) -> Result<Command> {
    let executable = directory.join("fixed-command");
    if !executable.exists() {
        fs::copy(binary("AIRLOCK_EXEC_BIN", "airlock-exec"), &executable)?;
    }
    fs::write(
        directory.join("fixed-command.json"),
        serde_json::to_vec(contract)?,
    )?;
    Ok(Command::new(executable))
}

/// Replace one fixed option's value with an inert executable or fixture identity.
pub fn contract_argument(contract: &mut serde_json::Value, name: &str, value: &str) -> Result<()> {
    let arguments = contract["args"]
        .as_array_mut()
        .context("native command arguments")?;
    let index = arguments
        .iter()
        .position(|argument| argument.as_str() == Some(name))
        .with_context(|| format!("native option {name}"))?;
    *arguments
        .get_mut(index + 1)
        .context("native option value")? = value.into();
    Ok(())
}
