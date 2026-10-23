//! Native CLI control and source submission preserve authority, streaming and committed source identity.

use crate::common;
use airlock_tests::dialogue::Dialogue;
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command, time::Duration};

const RELEASE: &str = "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-machine-release";

/// One private user workspace and inert controller RPC endpoint.
struct Cli {
    // Owns source, response data and recorded RPC evidence for one scenario.
    temporary: tempfile::TempDir,
}

impl Cli {
    /// Prepare a qrexec process peer without contacting Qubes or building any prerequisites.
    fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        common::peer_link(temporary.path(), "qrexec-client-vm")?;
        Ok(Self { temporary })
    }

    /// Resolve fixture-owned paths without relying on the invocation directory.
    fn path(&self, name: &str) -> PathBuf {
        self.temporary.path().join(name)
    }

    /// Run the real CLI with a private home, Git configuration and process-local RPC capture.
    fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(common::binary("AIRLOCK_CLI_BIN", "airlock"));
        command
            .args(arguments)
            .current_dir(self.temporary.path())
            .env("HOME", self.path("home"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.path("gitconfig"))
            .env("AIRLOCK_TEST_SUBMISSION", self.path("rpc.json"))
            .env(
                "PATH",
                format!(
                    "{root}:{path}",
                    root = self.temporary.path().display(),
                    path = std::env::var("PATH").unwrap()
                ),
            )
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR");
        command
    }

    /// Read the request that actually crossed the process boundary.
    fn request(&self) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(self.path("rpc.json"))?)?)
    }

    /// Create and commit a minimal source checkout with an explicit test-only Git identity.
    fn source(&self) -> Result<PathBuf> {
        let source = self.path("home/airlockos-machine");
        fs::create_dir_all(&source)?;
        fs::write(source.join("flake.lock"), "{}\n")?;
        for arguments in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=CLI test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "Committed source",
            ],
        ] {
            common::run(
                Command::new("git")
                    .arg("-C")
                    .arg(&source)
                    .args(arguments)
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", self.path("gitconfig"))
                    .env_remove("GIT_DIR")
                    .env_remove("GIT_INDEX_FILE")
                    .env_remove("GIT_WORK_TREE")
                    .env_remove("GIT_COMMON_DIR"),
                None,
            )?
            .checked()?;
        }
        Ok(source)
    }
}

/// Help works in an empty environment; invalid authority, path and confirmation inputs never reach a VM.
#[test]
fn cli_help_and_invalid_requests_have_no_runtime_side_effects() -> Result<()> {
    let cli = Cli::new()?;
    for arguments in [vec![], vec!["--help"], vec!["machine", "--help"]] {
        let output = common::run(
            cli.command(&arguments).env("PATH", cli.temporary.path()),
            None,
        )?;
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("machine"));
        assert!(output.stderr.is_empty());
    }
    for arguments in [
        vec!["machine", "guest-plan", "--target", "dom0"],
        vec!["machine", "review-status", "--request-id", "invalid"],
        vec!["machine", "status", "--before-expiry"],
        vec!["machine", "release-retain", "--release", RELEASE],
        vec![
            "machine",
            "release-prune",
            "--release",
            RELEASE,
            "--confirm",
            "/var/lib/airlock/machine/baseline",
        ],
        vec![
            "machine",
            "trash-purge",
            "--identity",
            "123-vm",
            "--confirm",
            "456-vm",
        ],
        vec![
            "machine",
            "trash-purge",
            "--identity",
            "../vm",
            "--confirm",
            "../vm",
        ],
        vec![
            "machine",
            "release-prune-plan",
            "--release",
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-machine-release/../other",
        ],
    ] {
        assert_eq!(
            common::run(&mut cli.command(&arguments), None)?
                .status
                .code(),
            Some(2)
        );
        assert!(!cli.path("rpc.json").exists());
    }
    Ok(())
}

/// Machine observation and recovery use the fixed dom0 RPC; maintenance preserves exact target and field types.
#[test]
fn cli_control_uses_fixed_authority_and_typed_targets() -> Result<()> {
    let cli = Cli::new()?;
    for action in [
        "status",
        "verify",
        "resume",
        "rollback",
        "trash-list",
        "release-list",
    ] {
        common::run(&mut cli.command(&["machine", action]), None)?.checked()?;
        assert_eq!(
            cli.request()?,
            json!({"args": ["dom0", "airlock.Ctrl.MachineControl"],
            "request": {"schema": "airlock.machine-control.v1", "action": action}})
        );
    }
    for (action, options, fields) in [
        (
            "release-prune-plan",
            vec!["--release", RELEASE, "--allow-unexpired-trash", "123-vm"],
            json!({"release": RELEASE, "allow_unexpired_trash": ["123-vm"]}),
        ),
        (
            "release-prune",
            vec!["--release", RELEASE, "--confirm", RELEASE],
            json!({"release": RELEASE, "confirm": RELEASE}),
        ),
        (
            "release-retain",
            vec!["--release", RELEASE, "--retain", "false"],
            json!({"release": RELEASE, "retain": "false"}),
        ),
        (
            "trash-purge",
            vec![
                "--identity",
                "123-vm",
                "--confirm",
                "123-vm",
                "--before-expiry",
            ],
            json!({"identity": "123-vm", "confirm": "123-vm", "before_expiry": true}),
        ),
    ] {
        let mut arguments = vec!["machine", action];
        arguments.extend(options);
        common::run(&mut cli.command(&arguments), None)?.checked()?;
        let record = cli.request()?;
        assert_eq!(
            record["args"],
            json!(["dom0", "airlock.Ctrl.MachineControl"])
        );
        let mut expected = json!({"schema": "airlock.machine-control.v1", "action": action});
        expected
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        assert_eq!(record["request"], expected);
    }
    let identity = "a".repeat(32);
    common::run(
        &mut cli.command(&["machine", "review-status", "--request-id", &identity]),
        None,
    )?
    .checked()?;
    assert_eq!(
        cli.request()?,
        json!({"args": ["sys-ctrl", "airlock.Ctrl.ReviewStatus"], "request": {"request_id": identity}})
    );
    Ok(())
}

/// Control output streams before RPC exit and preserves the controller's diagnostics and failure status exactly.
#[test]
fn cli_streams_control_output_and_preserves_controller_exit() -> Result<()> {
    let cli = Cli::new()?;
    let reply = "{\"status\":\"failed\",\"error\":\"activation failed\"}\n";
    let output = common::run(
        cli.command(&["machine", "status"])
            .env("AIRLOCK_TEST_SUBMISSION_REPLY", reply)
            .env("AIRLOCK_TEST_SUBMISSION_ERROR", "diagnostic\n")
            .env("AIRLOCK_TEST_SUBMISSION_EXIT", "7"),
        None,
    )?;
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, reply.as_bytes());
    assert_eq!(output.stderr, b"diagnostic\n");
    let started = std::time::Instant::now();
    let mut process = Dialogue::spawn(
        cli.command(&["machine", "verify"])
            .env("AIRLOCK_TEST_SUBMISSION_DELAY", "2000"),
    )?;
    assert_eq!(
        process.reply(Duration::from_secs(1))?,
        json!({"status": "passed"})
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(process.finish()?.success());
    Ok(())
}

/// Source commands send only committed source to the controller and bind archive provenance to request identity.
#[test]
fn cli_source_submission_uploads_committed_provenance_and_preserves_decisions() -> Result<()> {
    let cli = Cli::new()?;
    let source = cli.source()?;
    for (action, options, wire) in [
        ("build", vec![], "machine-build"),
        ("plan", vec![], "machine-plan"),
        ("apply", vec![], "machine-apply"),
        ("guest-plan", vec!["--target", "usr-home"], "plan"),
        ("guest-apply", vec!["--target", "usr-home"], "apply"),
    ] {
        let mut arguments = vec!["machine", action, "--repo", source.to_str().unwrap()];
        arguments.extend(options.iter());
        common::run(&mut cli.command(&arguments), None)?.checked()?;
        let record = cli.request()?;
        assert_eq!(
            record["args"],
            json!(["sys-ctrl", "airlock.Ctrl.GuestDeploy"])
        );
        assert_eq!(record["request"]["action"], wire);
        assert_eq!(
            record["request"]["target"],
            if options.is_empty() {
                Value::Null
            } else {
                json!("usr-home")
            }
        );
        assert_eq!(
            record["request"]["schema"],
            "airlock.sys-ctrl-guest-request.v1"
        );
        assert_eq!(record["members"], json!(["manifest.json", "repo.bundle"]));
        for field in ["revision", "lock_hash"] {
            assert_eq!(record["manifest"][field], record["request"][field]);
        }
    }
    common::run(&mut cli.command(&["machine", "plan"]), None)?.checked()?;
    assert_eq!(cli.request()?["request"]["action"], "machine-plan");
    for exit in [20, 21, 22, 23, 24] {
        assert_eq!(
            common::run(
                cli.command(&["machine", "apply", "--repo", source.to_str().unwrap()])
                    .env("AIRLOCK_TEST_SUBMISSION_EXIT", exit.to_string()),
                None
            )?
            .status
            .code(),
            Some(exit)
        );
    }
    fs::remove_file(cli.path("rpc.json"))?;
    fs::write(source.join("flake.lock"), "changed\n")?;
    let result = common::run(
        &mut cli.command(&["machine", "plan", "--repo", source.to_str().unwrap()]),
        None,
    )?;
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stdout).contains("commit or stash"));
    assert!(!cli.path("rpc.json").exists());
    Ok(())
}
