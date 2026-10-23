//! User workspace initialization preserves editable repositories, credentials and personally chosen guidance.

use crate::common;
use anyhow::Result;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Command,
};

/// One private home and Git configuration, insulated from the developer's own settings and hooks.
struct Workspace {
    // Owns the home, current guidance and editable repositories until each scenario finishes.
    temporary: tempfile::TempDir,
}

impl Workspace {
    /// Start with a home directory and one editable machine repository.
    fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        let value = Self { temporary };
        fs::create_dir(value.home())?;
        value.git(&["init", "-q", value.repo().to_str().unwrap()], true)?;
        Ok(value)
    }

    /// Resolve only fixture-owned paths.
    fn path(&self, name: &str) -> PathBuf {
        self.temporary.path().join(name)
    }

    /// User home containing private application state and the editable machine checkout.
    fn home(&self) -> PathBuf {
        self.path("home")
    }

    /// The supported location of the user-owned machine repository.
    fn repo(&self) -> PathBuf {
        self.path("home/airlockos-machine")
    }

    /// Build a child-local Git/home environment without changing process-global variables.
    fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env(
                "AIRLOCK_VM_SERVICE",
                common::binary("AIRLOCK_VM_SERVICE_BIN", "airlock-vm-service"),
            )
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.path("home/.config"))
            .env("GIT_CONFIG_GLOBAL", self.path("home/.gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1");
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        ] {
            command.env_remove(key);
        }
        command
    }

    /// Invoke real Git with no hook or global identity leakage from the developer's checkout.
    fn git(&self, arguments: &[&str], success: bool) -> Result<String> {
        let output = common::run(self.command("git").args(arguments), None)?;
        assert_eq!(
            output.status.success(),
            success,
            "{error}",
            error = String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }

    /// Read or write one identity field using the same Git resolution rules as an ordinary commit.
    fn config(&self, scope: &str, name: &str, value: Option<&str>) -> Result<String> {
        let repo = self.repo();
        let mut arguments = vec!["-C", repo.to_str().unwrap(), "config", scope, name];
        if let Some(value) = value {
            arguments.push(value);
        }
        self.git(&arguments, true)
    }

    /// Run the installed identity initializer against an explicit checkout path.
    fn identity(&self, repository: &Path, success: bool) -> Result<()> {
        let output = common::run(
            self.command(common::binary(
                "AIRLOCK_VM_SERVICE_BIN",
                "airlock-vm-service",
            ))
            .args(["runtime", "git-identity"])
            .arg(repository),
            None,
        )?;
        assert_eq!(
            output.status.success(),
            success,
            "{error}",
            error = String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    /// Initialize workspace state while keeping guidance inputs outside the user-owned directories.
    fn initialize(
        &self,
        state: &Path,
        instructions: &Path,
        home_instructions: &Path,
    ) -> Result<()> {
        common::run(
            self.command(common::binary(
                "AIRLOCK_VM_SERVICE_BIN",
                "airlock-vm-service",
            ))
            .args(["runtime", "workspace"])
            .arg(state)
            .arg(instructions)
            .arg(self.home())
            .arg(home_instructions)
            .arg(self.repo()),
            None,
        )?
        .checked()?;
        Ok(())
    }
}

/// Defaults permit commits without changing global identity, and repeated setup fills only absent fields.
#[test]
fn workspace_git_identity_is_repository_local_and_respects_personal_settings() -> Result<()> {
    let workspace = Workspace::new()?;
    workspace.identity(&workspace.repo(), true)?;
    assert_eq!(
        workspace.config("--local", "user.name", None)?,
        "Airlock User"
    );
    assert_eq!(
        workspace.config("--local", "user.email", None)?,
        "user@airlock.invalid"
    );
    assert!(!workspace.path("home/.gitconfig").exists());
    workspace.git(
        &[
            "-C",
            workspace.repo().to_str().unwrap(),
            "commit",
            "--allow-empty",
            "-qm",
            "First change",
        ],
        true,
    )?;
    assert_eq!(
        workspace.git(
            &[
                "-C",
                workspace.repo().to_str().unwrap(),
                "log",
                "-1",
                "--format=%an <%ae>"
            ],
            true
        )?,
        "Airlock User <user@airlock.invalid>"
    );

    for scope in ["--local", "--global"] {
        let workspace = Workspace::new()?;
        workspace.config(scope, "user.name", Some("Personal Name"))?;
        workspace.config(scope, "user.email", Some("personal@example.com"))?;
        let before = fs::read(workspace.repo().join(".git/config"))?;
        for _ in 0..2 {
            workspace.identity(&workspace.repo(), true)?;
        }
        assert_eq!(fs::read(workspace.repo().join(".git/config"))?, before);
        assert_eq!(workspace.config(scope, "user.name", None)?, "Personal Name");
        assert_eq!(
            workspace.config(scope, "user.email", None)?,
            "personal@example.com"
        );

        let workspace = Workspace::new()?;
        workspace.config(scope, "user.name", Some("Personal Name"))?;
        workspace.identity(&workspace.repo(), true)?;
        assert_eq!(workspace.config(scope, "user.name", None)?, "Personal Name");
        assert_eq!(
            workspace.config("--local", "user.email", None)?,
            "user@airlock.invalid"
        );
    }
    Ok(())
}

/// A late-created checkout can be initialized on the next boot; malformed user configuration is never overwritten.
#[test]
fn workspace_git_identity_tolerates_absence_and_preserves_invalid_configuration() -> Result<()> {
    let workspace = Workspace::new()?;
    let later = workspace.path("home/later-checkout");
    workspace.identity(&later, true)?;
    assert!(!later.exists());
    workspace.git(&["init", "-q", later.to_str().unwrap()], true)?;
    workspace.identity(&later, true)?;
    assert_eq!(
        workspace.git(
            &[
                "-C",
                later.to_str().unwrap(),
                "config",
                "--local",
                "user.name"
            ],
            true
        )?,
        "Airlock User"
    );

    fs::write(workspace.repo().join(".git/config"), "[broken\n")?;
    workspace.identity(&workspace.repo(), false)?;
    assert_eq!(
        fs::read_to_string(workspace.repo().join(".git/config"))?,
        "[broken\n"
    );
    Ok(())
}

/// Relocation keeps uncommitted files and local settings, while generation guidance follows the source link.
#[test]
fn workspace_relocation_preserves_local_work_and_updates_linked_guidance() -> Result<()> {
    let workspace = Workspace::new()?;
    let old = workspace.path("home/dev/airlock-machine");
    fs::create_dir_all(old.parent().unwrap())?;
    fs::rename(workspace.repo(), &old)?;
    fs::write(old.join("local-work"), "uncommitted work")?;
    workspace.git(
        &[
            "-C",
            old.to_str().unwrap(),
            "config",
            "airlock.test",
            "retained",
        ],
        true,
    )?;
    let intro = workspace.path("intro.md");
    fs::write(&intro, "generation one")?;
    let state = workspace.path("home/.codex");
    let instructions = workspace.path("instructions.md");
    workspace.initialize(&state, &instructions, &intro)?;
    assert!(!old.exists());
    assert_eq!(
        fs::read_to_string(workspace.repo().join("local-work"))?,
        "uncommitted work"
    );
    assert_eq!(
        workspace.config("--local", "airlock.test", None)?,
        "retained"
    );
    assert!(workspace.path("home/AGENTS.md").is_symlink());
    fs::write(&intro, "generation two")?;
    workspace.initialize(&state, &instructions, &intro)?;
    assert_eq!(
        fs::read_to_string(workspace.path("home/AGENTS.md"))?,
        "generation two"
    );
    Ok(())
}

/// Private guidance tracks generations while credentials and deliberately customized files or links survive setup.
#[test]
fn workspace_guidance_preserves_private_state_and_user_choices() -> Result<()> {
    let workspace = Workspace::new()?;
    let state = workspace.path("private/codex");
    let instructions = workspace.path("system/AGENTS.md");
    fs::create_dir_all(instructions.parent().unwrap())?;
    fs::write(&instructions, "generation one")?;
    workspace.initialize(&state, &instructions, &instructions)?;
    assert_eq!(state.metadata()?.permissions().mode() & 0o777, 0o700);
    assert!(state.join("AGENTS.md").is_symlink());
    fs::write(state.join("auth.json"), "private credentials")?;
    fs::write(&instructions, "generation two")?;
    workspace.initialize(&state, &instructions, &instructions)?;
    assert_eq!(
        fs::read_to_string(state.join("AGENTS.md"))?,
        "generation two"
    );
    assert_eq!(
        fs::read_to_string(state.join("auth.json"))?,
        "private credentials"
    );

    for existing in ["file", "symlink", "dangling-symlink"] {
        let workspace = Workspace::new()?;
        let state = workspace.path("home/.codex");
        fs::create_dir(&state)?;
        let custom = workspace.path("custom.md");
        fs::write(&custom, "user instructions")?;
        for target in [state.join("AGENTS.md"), workspace.path("home/AGENTS.md")] {
            match existing {
                "file" => fs::write(&target, "user instructions")?,
                "symlink" => symlink(&custom, &target)?,
                _ => symlink(workspace.path("missing"), &target)?,
            }
        }
        let old = workspace.path("home/dev/airlock-machine");
        fs::create_dir_all(old.join(".git"))?;
        fs::write(old.join("local-work"), "old work")?;
        fs::write(workspace.repo().join("local-work"), "new work")?;
        workspace.initialize(
            &state,
            &workspace.path("new-system-instructions"),
            &workspace.path("new-home-instructions"),
        )?;
        for target in [state.join("AGENTS.md"), workspace.path("home/AGENTS.md")] {
            if existing == "dangling-symlink" {
                assert_eq!(fs::read_link(target)?, workspace.path("missing"));
            } else {
                assert_eq!(fs::read_to_string(target)?, "user instructions");
            }
        }
        assert_eq!(fs::read_to_string(old.join("local-work"))?, "old work");
        assert_eq!(
            fs::read_to_string(workspace.repo().join("local-work"))?,
            "new work"
        );
    }
    Ok(())
}
