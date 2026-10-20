//! Finite upstream replies and real Git archives for native configuration transport boundaries.

use super::*;

/// A private candidate source, normalized contracts and inert command namespace.
pub struct Config {
    // Owns every source file, response and system-link destination for one scenario.
    temporary: tempfile::TempDir,
    // Supplied upstream outcomes; helpers record calls but never implement configuration policy.
    pub data: Value,
    // Real normalized topology consumed by the configuration runtime.
    pub topology: Value,
    // Real normalized host contract consumed by the configuration runtime.
    pub system: Value,
}

impl Config {
    /// Prepare an inert source tree whose executable entry points still pass through the native fixture peer.
    pub fn new() -> Result<Self> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        for name in ["candidate", "bin", "logs", "etc", "state", "profiles"] {
            fs::create_dir(root.join(name))?;
        }
        for name in [
            "airlock-config-archive-candidate",
            "airlock-config-apply",
            "airlock-config-status",
            "airlock-setup-config",
            "qrexec-client",
            "qvm-ls",
            "qvm-prefs",
            "qvm-check",
            "qvm-start",
            "systemctl",
            "nix-env",
            "nix",
        ] {
            common::peer_link(&root.join("bin"), name)?;
        }
        std::os::unix::fs::symlink(
            common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"),
            root.join("bin/airlock-dom0-service"),
        )?;
        for (directory, executable) in [
            ("dom0-system", "switch-to-configuration"),
            ("profile", "airlock-dom0-profile-activate"),
        ] {
            let bin = root.join("candidate").join(directory).join("bin");
            fs::create_dir_all(&bin)?;
            common::peer_link(&bin, executable)?;
        }
        fs::write(root.join("xen-capabilities"), "control_d\n")?;
        let system = json!({
            "schema": "airlock.system.v1", "dom0": {
                "profile": {"source": null, "source_ref": null, "activation_command": "bin/airlock-dom0-profile-activate", "activation_action": "run", "message": "hello"},
                "packages": [], "systemd": {"services": {}},
                "nixos": {
                    "system": {"enable": false, "source": null, "source_ref": null, "switch_action": "switch"},
                    "kernel": {"enable": false, "source": null, "source_ref": null},
                    "xen": {"enable": false, "source": null, "source_ref": null},
                    "bootloader": {"enable": false, "source": null, "source_ref": null},
                    "qubes": {"enable": false, "source": null, "source_ref": null}
                }
            }, "plan": {"live_actions": [], "reboot_actions": [], "reboot_required": false, "reboot_reasons": []}
        });
        let value = Self {
            temporary,
            data: json!({"domains": ["dom0", "dev", "sys-net", "sys-firewall"], "outputs": {}}),
            topology: json!({"schema": "airlock.qubes-topology.v1", "templates": {}, "kernels": {}, "prefs": {}, "domains": {}, "qrexec_policy": {"rules": []}}),
            system,
        };
        value.git(&["init", "-q", "--initial-branch=main"])?;
        value.git(&["add", "."])?;
        value.git(&[
            "-c",
            "user.name=Configuration test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Inert candidate source",
        ])?;
        Ok(value)
    }

    /// Locate a fixture-owned path; all mutable production locations are redirected into this root.
    pub fn path(&self, name: &str) -> PathBuf {
        self.temporary.path().join(name)
    }

    /// Run real Git independently of the developer's own configuration and repository environment.
    fn git(&self, arguments: &[&str]) -> Result<Vec<u8>> {
        common::run(
            Command::new("git")
                .arg("-C")
                .arg(self.path("candidate"))
                .args(arguments)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", self.path("gitconfig"))
                .env_remove("GIT_DIR")
                .env_remove("GIT_INDEX_FILE")
                .env_remove("GIT_COMMON_DIR")
                .env_remove("GIT_WORK_TREE"),
            None,
        )?
        .checked()
    }

    /// Write literal response data or normalized contracts without introducing executable configuration text.
    pub fn write(&self, name: &str, value: &Value) -> Result<()> {
        let path = self.path(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, serde_json::to_vec(value)?)?;
        Ok(())
    }

    /// Decode an artifact only after the actual script has published it.
    pub fn read(&self, name: &str) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(self.path(name))?)?)
    }

    /// Save upstream response configuration separately from the actual transport archive.
    pub fn persist(&self) -> Result<()> {
        self.write("config.json", &self.data)
    }

    /// Create a real source bundle and validated transport, binding each contract to its committed source identity.
    pub fn pack(&mut self) -> Result<()> {
        let commit = String::from_utf8(self.git(&["rev-parse", "HEAD"])?)?
            .trim()
            .to_owned();
        self.data["commit"] = json!(commit);
        self.git(&[
            "bundle",
            "create",
            self.path("config.bundle").to_str().unwrap(),
            "HEAD",
            "refs/heads/main",
        ])?;
        self.write("manifest.json", &json!({"schema": "airlock.config-candidate.v1", "commit": commit, "branch": "main", "source_role": "sys-config-staging"}))?;
        self.write("validation.json", &json!({"schema": "airlock.config-validation.v1", "status": "passed", "commit": commit, "checks": []}))?;
        self.write("topology.json", &self.topology)?;
        self.write("system.json", &self.system)?;
        for (name, members) in [
            (
                "candidate.tar",
                &[
                    "config.bundle",
                    "validation.json",
                    "manifest.json",
                    "topology.json",
                    "system.json",
                ][..],
            ),
            ("staged.tar", &["config.bundle", "manifest.json"]),
        ] {
            let mut archive = tar::Builder::new(fs::File::create(self.path(name))?);
            for member in members {
                archive.append_path_with_name(self.path(member), member)?;
            }
            archive.finish()?;
        }
        self.persist()
    }

    /// Build a child-only environment with every mutable system destination inside the fixture.
    pub fn command(&self, operation: &str, arguments: &[&str]) -> Result<Command> {
        self.persist()?;
        let mut command = Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        command
            .args(["config", operation])
            .args(arguments)
            .env("AIRLOCK_TEST_CONFIGURATION", self.temporary.path())
            .env(
                "PATH",
                format!(
                    "{bin}:{path}",
                    bin = self.path("bin").display(),
                    path = std::env::var("PATH")?
                ),
            )
            .env("AIRLOCK_XEN_CAPABILITIES", self.path("xen-capabilities"))
            .env("AIRLOCK_LOG_DIR", self.path("logs"))
            .env("AIRLOCK_USE_HOST_NIX_STORE", "1")
            .env("AIRLOCK_SYSTEM_STATE_DIR", self.path("state"))
            .env("AIRLOCK_SYSTEM_ETC_DIR", self.path("etc"))
            .env(
                "AIRLOCK_SYSTEM_PROFILE",
                self.path("profiles/airlock-system"),
            )
            .env(
                "AIRLOCK_HOST_SYSTEM_PROFILE",
                self.path("profiles/host-system"),
            )
            .env(
                "AIRLOCK_HOST_CURRENT_SYSTEM_LINK",
                self.path("current-system"),
            )
            .env("AIRLOCK_QREXEC_RUN_DIR", self.path("absent-qrexec"))
            .env("AIRLOCK_CONFIG_CONTROL_LOG_DIR", self.path("logs/control"))
            .env("AIRLOCK_QUBES_POLICY_DIR", self.path("policy.d"))
            .env(
                "AIRLOCK_CONFIG_STATUS_BOOT_TOPOLOGY_JSON",
                self.path("etc/qubes-topology.json"),
            )
            .env(
                "AIRLOCK_CONFIG_STATUS_SYSTEM_PROFILE",
                self.path("profiles/airlock-system"),
            )
            .env(
                "AIRLOCK_CONFIG_STATUS_HOST_SYSTEM_PROFILE",
                self.path("profiles/host-system"),
            )
            .env(
                "AIRLOCK_CONFIG_STATUS_RUN_CURRENT_SYSTEM",
                self.path("current-system"),
            )
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.path("gitconfig"));
        Ok(command)
    }

    /// Read actual process invocations to distinguish validation from activation and to inspect offline build flags.
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

    /// Invoke an administrative native entry point with the same isolated filesystem and tool peers.
    pub fn native(&self, arguments: &[&str]) -> Result<Command> {
        let environment = self.command("apply", &[])?;
        let mut command = Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        for (key, value) in environment.get_envs() {
            match value {
                Some(value) => {
                    command.env(key, value);
                }
                None => {
                    command.env_remove(key);
                }
            }
        }
        command.args(arguments);
        Ok(command)
    }

    /// Run deployment against the prepared validated archive and expose both durable success and intentional failures.
    pub fn apply(&self, accepted: bool) -> Result<Value> {
        let output = common::run(&mut self.command("apply", &["--apply"])?, None)?;
        assert_eq!(
            output.status.success(),
            accepted,
            "{stdout}; {stderr}; {report}",
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr),
            report = fs::read_to_string(self.path("logs/config-apply.json")).unwrap_or_default()
        );
        self.read("logs/config-apply.json")
    }

    /// Send the actual deployment bundle to the native vault receiver with an isolated archived candidate.
    pub fn vault(&self) -> Result<Value> {
        let archive = self.path("vault-archive");
        fs::create_dir_all(
            archive
                .join("candidates")
                .join(self.data["commit"].as_str().unwrap())
                .join("snapshot"),
        )?;
        common::run(
            Command::new("git")
                .args(["init", "-q", "-b", "main"])
                .arg(&archive),
            None,
        )?
        .checked()?;
        let settings = self.path("vault-runtime.json");
        self.write("vault-runtime.json", &json!({"user":"user","repo":self.path("candidate"),"archive":archive,"validation":self.path("validation"),"inputs":self.path("inputs"),"store":self.path("store"),"public_inputs":self.path("public-inputs"),"seed":self.path("candidate"),"nix":"nix","role":"sys-config-vault","staging_path":std::env::var("PATH")?}))?;
        common::json(
            Command::new(common::binary("AIRLOCK_VM_BIN", "airlock-vm-service"))
                .args(["configuration", "--config"])
                .arg(settings)
                .arg("vault-deployment"),
            Some(fs::read(self.path("deployment.tar"))?),
        )
    }
}
