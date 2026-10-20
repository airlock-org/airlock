//! Writable builder views preserve private outputs while binding their lower store to approved immutable identity.

use crate::common;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
};

/// Child-side builder state lives entirely under a root-owned scratch hierarchy in a private mount namespace.
struct Builder {
    // Private root supplied by the namespace parent, never a real VM's state directory.
    root: PathBuf,
}

impl Builder {
    /// Write a selected base identity into the immutable source view consumed by the real builder command.
    fn identity(&self, generation: u64, fields: Value) -> Result<Value> {
        let mut identity = json!({"schema": "airlock.usr-assistant-builder.v1", "domain": "usr-assistant-builder",
            "generation": generation, "closure_hash": format!("{generation:064x}"),
            "registration_sha256": format!("{hash:x}", hash = Sha256::digest(b"registration"))});
        identity
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        fs::write(
            self.root.join("source/.airlock-builder/identity.json"),
            serde_json::to_vec(&identity)?,
        )?;
        Ok(identity)
    }

    /// Invoke only the installed builder view command, with fixture-local state and runtime paths.
    fn command(&self, operation: &str, runtime: &str, state: &str) -> Command {
        let mut command = Command::new(common::binary("AIRLOCK_CLI_BIN", "airlock"));
        command
            .args(["builder-store", operation, "--state"])
            .arg(self.root.join(state))
            .arg("--runtime")
            .arg(self.root.join(runtime))
            .args(["--user", "root"]);
        command
    }

    /// Run one successful lifecycle operation and retain its JSON or diagnostic bytes as needed.
    fn call(&self, operation: &str) -> Result<Vec<u8>> {
        common::run(&mut self.command(operation, "run space", "state"), None)?.checked()
    }

    /// Read a state record written by production code after a completed operation.
    fn read(&self, name: &str) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(self.root.join(name))?)?)
    }

    /// Failure must not leave a writable store or stale lower mounts exposed.
    fn unmounted(&self) -> Result<()> {
        let table = fs::read_to_string("/proc/self/mountinfo")?;
        assert!(!table.contains("/var/lib/airlock/run\\040space/"));
        assert!(!table.contains("/var/lib/airlock/state/merged/nix/store"));
        Ok(())
    }
}

/// Root-owned namespace ancestors preserve production path checks without weakening them for unmapped host root ownership.
fn child(case: &str) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    fs::create_dir_all(root.join("source/.airlock-builder"))?;
    fs::create_dir(root.join("source/.links"))?;
    fs::write(
        root.join("source/.airlock-builder/registration"),
        b"registration",
    )?;
    fs::create_dir(root.join("bin"))?;
    for name in ["mount", "umount", "nix-store"] {
        common::peer_link(&root.join("bin"), name)?;
    }
    fs::write(
        root.join("tools.json"),
        serde_json::to_vec(
            &json!({"mount": common::tool("mount")?, "umount": common::tool("umount")?}),
        )?,
    )?;
    let mut command = Command::new(common::tool("bwrap")?);
    command.args([
        "--die-with-parent",
        "--unshare-user",
        "--uid",
        "0",
        "--gid",
        "0",
        "--unshare-pid",
        "--cap-add",
        "CAP_SYS_ADMIN",
        "--tmpfs",
        "/",
    ]);
    for path in [
        "/nix",
        "/home",
        "/etc",
        "/lib",
        "/lib64",
        "/usr",
        "/bin",
        "/run/current-system",
        "/run/wrappers",
    ] {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
    command
        .args([
            "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp", "--dir", "/var/lib", "--bind",
        ])
        .arg(root)
        .arg("/var/lib/airlock");
    command
        .arg(std::env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "builder::namespace_builder_lifecycle",
            "--nocapture",
        ])
        .env("TMPDIR", "/tmp")
        .env("AIRLOCK_TEST_BUILDER_CASE", case)
        .env("AIRLOCK_TEST_BUILDER_ROOT", "/var/lib/airlock")
        .env(
            "PATH",
            format!("/var/lib/airlock/bin:{path}", path = std::env::var("PATH")?),
        );
    common::run(&mut command, None)?.checked()?;
    Ok(())
}

/// Approved successors preserve private output; corrupt identities, partial setup, lost journals and untrusted paths fail closed.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged mount namespaces"]
fn namespace_builder_lifecycle() -> Result<()> {
    let case = match std::env::var("AIRLOCK_TEST_BUILDER_CASE") {
        Ok(case) => case,
        Err(_) => {
            for case in [
                "restart",
                "successor",
                "registration",
                "flake-inputs",
                "last-mount",
                "foreign",
                "journal",
                "stop-retry",
                "database",
                "zero-generation",
                "hash",
                "history",
                "symlink",
                "fifo",
                "other-runtime",
                "missing-journal",
                "ancestor",
            ] {
                child(case).with_context(|| format!("builder lifecycle case {case}"))?;
            }
            return Ok(());
        }
    };
    let builder = Builder {
        root: PathBuf::from(
            std::env::var_os("AIRLOCK_TEST_BUILDER_ROOT").context("builder fixture root")?,
        ),
    };
    let original = builder.identity(1, json!({}))?;
    let root = &builder.root;
    match case.as_str() {
        "restart" | "successor" => {
            builder.call("start")?;
            let ready = builder.read("run space/ready.json")?;
            assert!(
                ready["uri"]
                    .as_str()
                    .unwrap()
                    .starts_with("local-overlay://")
            );
            assert!(ready["uri"].as_str().unwrap().contains("read-only%3Dtrue"));
            fs::write(root.join("state/upper/saved-output"), "keep")?;
            builder.call("stop")?;
            builder.unmounted()?;
            builder.call("start")?;
            builder.call("stop")?;
            if case == "successor" {
                let predecessor = json!({"generation": original["generation"], "closure_hash": original["closure_hash"], "registration_sha256": original["registration_sha256"]});
                let successor = builder.identity(2, json!({"predecessors": [predecessor]}))?;
                builder.call("start")?;
                assert_eq!(builder.read("state/base.json")?, successor);
                builder.call("stop")?;
                fs::remove_file(root.join("state/base.json"))?;
            } else {
                builder.identity(2, json!({}))?;
            }
            let failure = common::run(&mut builder.command("start", "run space", "state"), None)?;
            assert!(!failure.status.success());
            let error = String::from_utf8_lossy(&failure.stderr);
            assert!(
                error.contains(if case == "successor" {
                    "without its base identity"
                } else {
                    "base changed"
                }),
                "{error}"
            );
            assert_eq!(
                fs::read_to_string(root.join("state/upper/saved-output"))?,
                "keep"
            );
            if case == "restart" {
                assert_eq!(builder.read("state/base.json")?, original);
            }
        }
        "foreign" => {
            fs::create_dir(root.join("state"))?;
            let previous = builder.identity(
                1,
                json!({"domain": "sys-ctrl-builder", "schema": "airlock.sys-ctrl-builder.v1"}),
            )?;
            builder.identity(1, json!({}))?;
            fs::write(root.join("state/base.json"), serde_json::to_vec(&previous)?)?;
            fs::create_dir(root.join("state/upper"))?;
            fs::write(root.join("state/upper/saved-output"), "keep")?;
            builder.call("start")?;
            assert!(!root.join("state/upper/saved-output").exists());
            builder.call("stop")?;
            fs::write(root.join("state/base.json"), serde_json::to_vec(&previous)?)?;
            let failure = common::run(&mut builder.command("start", "run space", "state"), None)?;
            assert!(!failure.status.success());
            assert!(String::from_utf8_lossy(&failure.stderr).contains("archive already exists"));
            assert_eq!(
                fs::read_to_string(root.join("state-sys-ctrl-builder/upper/saved-output"))?,
                "keep"
            );
        }
        "journal" => {
            fs::create_dir(root.join("run space"))?;
            fs::write(root.join("run space/mounts.json"), "[\"/unrelated\"]")?;
            let failure = common::run(&mut builder.command("stop", "run space", "state"), None)?;
            assert!(!failure.status.success());
            assert!(
                String::from_utf8_lossy(&failure.stderr).contains("invalid builder mount journal")
            );
            assert!(root.join("run space/mounts.json").exists());
        }
        "stop-retry" | "database" | "other-runtime" | "missing-journal" => {
            builder.call("start")?;
            match case.as_str() {
                "stop-retry" => {
                    fs::write(root.join("umount-fail"), "")?;
                    assert!(
                        !common::run(&mut builder.command("stop", "run space", "state"), None)?
                            .status
                            .success()
                    );
                    assert!(!root.join("run space/ready.json").exists());
                    assert!(root.join("run space/mounts.json").exists());
                    fs::remove_file(root.join("umount-fail"))?;
                }
                "database" => {
                    builder.call("stop")?;
                    fs::write(
                        root.join("run space/lower/nix/var/nix/db/stale-row"),
                        "unapproved",
                    )?;
                    builder.call("start")?;
                    assert!(
                        !root
                            .join("run space/lower/nix/var/nix/db/stale-row")
                            .exists()
                    );
                }
                "other-runtime" => {
                    fs::write(root.join("state/upper/saved-output"), "keep")?;
                    let failure = common::run(
                        &mut builder.command("start", "another-runtime", "state"),
                        None,
                    )?;
                    assert!(!failure.status.success());
                    assert!(String::from_utf8_lossy(&failure.stderr).contains("existing mounts"));
                    assert_eq!(
                        fs::read_to_string(root.join("state/upper/saved-output"))?,
                        "keep"
                    );
                    assert!(root.join("run space/ready.json").exists());
                }
                _ => {
                    let path = root.join("run space/mounts.json");
                    let journal = fs::read(&path)?;
                    fs::remove_file(&path)?;
                    let failure =
                        common::run(&mut builder.command("stop", "run space", "state"), None)?;
                    assert!(!failure.status.success());
                    assert!(
                        String::from_utf8_lossy(&failure.stderr).contains("without their journal")
                    );
                    assert!(!root.join("run space/ready.json").exists());
                    fs::write(path, journal)?;
                }
            }
            builder.call("stop")?;
        }
        _ => {
            let mut state = "state";
            match case.as_str() {
                "registration" => {
                    fs::write(root.join("source/.airlock-builder/registration"), "corrupt")?
                }
                "flake-inputs" => {
                    builder.identity(1, json!({"flake_inputs_sha256": "wrong"}))?;
                    fs::write(root.join("source/.airlock-builder/flake-inputs.json"), "{}")?;
                }
                "last-mount" => fs::write(root.join("last-mount"), "")?,
                "zero-generation" => {
                    builder.identity(0, json!({}))?;
                }
                "hash" => {
                    builder.identity(1, json!({"closure_hash": "bad"}))?;
                }
                "history" => {
                    builder.identity(1, json!({"predecessors": [{"generation": original["generation"], "closure_hash": original["closure_hash"], "registration_sha256": original["registration_sha256"]}]}))?;
                }
                "symlink" | "fifo" => {
                    let path = root.join("source/.airlock-builder/identity.json");
                    let saved = root.join("source/saved-identity.json");
                    fs::rename(&path, &saved)?;
                    if case == "symlink" {
                        symlink(saved, path)?;
                    } else {
                        rustix::fs::mknodat(
                            rustix::fs::CWD,
                            &path,
                            rustix::fs::FileType::Fifo,
                            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                            0,
                        )?;
                    }
                }
                "ancestor" => {
                    fs::create_dir(root.join("unsafe"))?;
                    fs::set_permissions(root.join("unsafe"), fs::Permissions::from_mode(0o777))?;
                    state = "unsafe/state";
                }
                _ => anyhow::bail!("unknown builder case: {case}"),
            }
            assert!(
                !common::run(&mut builder.command("start", "run space", state), None)?
                    .status
                    .success(),
                "{case}"
            );
            assert!(!root.join("run space/ready.json").exists());
            assert!(!root.join("run space/mounts.json").exists());
            if ["zero-generation", "hash", "history", "symlink", "fifo"].contains(&case.as_str()) {
                assert!(!root.join("state/upper").exists());
            }
        }
    }
    builder.unmounted()?;
    Ok(())
}
