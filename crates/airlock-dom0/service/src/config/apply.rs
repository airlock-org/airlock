//! Stage the existing configuration apply protocol without evaluating source or dispatching effects in shell.

use super::{
    build, host,
    io::{self, Operation},
};
use crate::process::Files;
use airlock_nix_core::identity::{GitRevision, GuestName};
use anyhow::{Context, Result, ensure};
use clap::Args;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// Existing configuration apply options; this remains an explicit administrative compatibility path.
#[derive(Args)]
pub(crate) struct Request {
    /// Explicitly request activation rather than an implicit default action.
    #[arg(long)]
    apply: bool,
    /// Staging role used by the fixed candidate archival helper.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_STAGING_VM",
        default_value = "sys-config-staging"
    )]
    staging: GuestName,
    /// Offline validation role used by the fixed candidate archival helper.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_VALIDATOR_VM",
        default_value = "sys-config-validator"
    )]
    validator: GuestName,
    /// Offline vault receiving both candidate and deployment evidence.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_VAULT_VM",
        default_value = "sys-config-vault"
    )]
    vault: GuestName,
    /// Optional staged source archive received through the existing controller.
    #[arg(long)]
    candidate_tar: Option<PathBuf>,
}

/// Fixed compatibility artifact mapping, shared by receipts and deployment archives.
const ARTIFACTS: &[(&str, &str, &str)] = &[
    ("topology", "config-apply-topology.json", "topology.json"),
    (
        "validated_topology",
        "config-apply-topology-validated.json",
        "validated-topology.json",
    ),
    ("system", "config-apply-system.json", "resolved-system.json"),
    (
        "validated_system",
        "config-apply-system-validated.json",
        "system.json",
    ),
    (
        "system_plan",
        "config-apply-system-plan.json",
        "system-plan.json",
    ),
    (
        "system_builds",
        "config-apply-system-builds.json",
        "system-builds.json",
    ),
    (
        "system_apply",
        "config-system-apply.json",
        "system-apply.json",
    ),
    (
        "systemd_actions",
        "config-system-systemd-actions.json",
        "systemd-actions.json",
    ),
    (
        "template_builds",
        "config-apply-template-builds.json",
        "template-builds.json",
    ),
    (
        "nix_domain_builds",
        "config-apply-nix-domain-builds.json",
        "nix-domain-builds.json",
    ),
    (
        "nix_domain_materializations",
        "config-apply-nix-domain-materializations.json",
        "nix-domain-materializations.json",
    ),
    (
        "unsupported_domain_backends",
        "config-apply-unsupported-domain-backends.json",
        "unsupported-domain-backends.json",
    ),
    (
        "domain_references",
        "config-apply-domain-references.json",
        "domain-references.json",
    ),
    (
        "validation",
        "config-apply-validation.json",
        "validation.json",
    ),
    ("manifest", "config-apply-manifest.json", "manifest.json"),
    ("archive", "config-archive.json", "archive.json"),
    ("setup", "setup-config.json", "setup.json"),
];

/// Fixed diagnostic filenames retained with every deployment archive.
const LOGS: &[(&str, &str, &str)] = &[
    (
        "command_log",
        "config-apply.commands.log",
        "apply.commands.log",
    ),
    (
        "archive_stdout",
        "config-apply-archive.stdout.log",
        "archive.stdout.log",
    ),
    (
        "archive_stderr",
        "config-apply-archive.stderr.log",
        "archive.stderr.log",
    ),
    (
        "setup_stdout",
        "config-apply-setup.stdout.log",
        "setup.stdout.log",
    ),
    (
        "setup_stderr",
        "config-apply-setup.stderr.log",
        "setup.stderr.log",
    ),
    (
        "template_build_log",
        "config-apply-template-builds.log",
        "template-builds.log",
    ),
    (
        "nix_domain_build_log",
        "config-apply-nix-domain-builds.log",
        "nix-domain-builds.log",
    ),
    (
        "system_build_log",
        "config-apply-system-builds.log",
        "system-builds.log",
    ),
    (
        "system_apply_log",
        "config-system-apply.log",
        "system-apply.log",
    ),
    (
        "profile_activation_log",
        "config-system-profile-activation.log",
        "profile-activation.log",
    ),
    (
        "host_system_activation_log",
        "config-system-host-activation.log",
        "host-system-activation.log",
    ),
    (
        "systemd_actions_log",
        "config-system-systemd-actions.log",
        "systemd-actions.log",
    ),
    ("clone_log", "config-apply-git-clone.log", "clone.log"),
    (
        "checkout_log",
        "config-apply-git-checkout.log",
        "checkout.log",
    ),
];

/// State of one existing apply invocation, separate from the reviewed deployment transaction protocol.
struct Apply {
    /// Command evidence and payload directory.
    operation: Operation,
    /// Explicit CLI role and candidate selections.
    request: Request,
    /// One of the existing full/archive/resolve/setup/system/deploy entry stages.
    stage: String,
    /// Candidate revision accepted from the archival result.
    commit: String,
    /// Timestamp, revision and random attempt identity for the deployment evidence bundle.
    deployment: String,
}

impl Apply {
    /// Publish the compatibility apply receipt with both paths and embedded observations.
    fn receipt(&self, status: &str, reason: &str) -> Result<Value> {
        let mut receipt = json!({"schema":"airlock.config-apply.v1", "status":status,"reason":reason,"stage":self.stage,"staging":self.request.staging,"validator":self.request.validator,"vault":self.request.vault,"candidate_commit":self.commit,"deployment_id":self.deployment,"deployment_archive_service":io::setting("AIRLOCK_CONFIG_DEPLOYMENT_ARCHIVE_SERVICE","airlock.Config.ArchiveDeployment")});
        for (key, file, _) in ARTIFACTS.iter().chain(LOGS) {
            receipt[*key] = json!(self.operation.file(file));
        }
        for (key, file) in [
            ("archive", "config-archive.json"),
            ("setup", "setup-config.json"),
            ("system_apply_result", "config-system-apply.json"),
            (
                "systemd_actions_result",
                "config-system-systemd-actions.json",
            ),
            (
                "nix_domain_builds_result",
                "config-apply-nix-domain-builds.json",
            ),
            (
                "nix_domain_materializations_result",
                "config-apply-nix-domain-materializations.json",
            ),
            (
                "unsupported_domain_backends_result",
                "config-apply-unsupported-domain-backends.json",
            ),
            (
                "domain_references_result",
                "config-apply-domain-references.json",
            ),
            ("deployment", "config-deployment.json"),
            ("deployment_archive", "config-vault-deployment-archive.json"),
        ] {
            receipt[key] = io::optional(&self.operation.file(file));
        }
        for (key, file) in [
            ("deployment_record", "config-deployment.json"),
            ("deployment_tar", "config-deployment.tar"),
            (
                "deployment_archive_stdout",
                "config-apply-deployment-archive.stdout.log",
            ),
            (
                "deployment_archive_stderr",
                "config-apply-deployment-archive.stderr.log",
            ),
            ("validated_candidate_tar", "config-candidate-validated.tar"),
        ] {
            receipt[key] = json!(self.operation.file(file));
        }
        io::save(&self.operation.file("config-apply.json"), &receipt)?;
        Ok(receipt)
    }

    /// Retain the archived candidate identity before any later stage is allowed to proceed.
    fn candidate(&mut self) -> Result<()> {
        if matches!(self.stage.as_str(), "full" | "archive") {
            let mut args = vec![
                "--staging".into(),
                self.request.staging.as_str().into(),
                "--validator".into(),
                self.request.validator.as_str().into(),
                "--vault".into(),
                self.request.vault.as_str().into(),
            ];
            if let Some(candidate) = &self.request.candidate_tar {
                args.extend(["--candidate-tar".into(), candidate.as_os_str().to_owned()]);
            }
            let result = self.operation.transfer(
                "airlock-config-archive-candidate",
                &args,
                Files {
                    input: None,
                    output: &self.operation.file("config-apply-archive.stdout.log"),
                    errors: &self.operation.file("config-apply-archive.stderr.log"),
                    limit: 16 * 1024 * 1024,
                },
                &[],
            );
            if let Err(error) = result {
                let receipt = io::optional(&self.operation.file("config-archive.json"));
                anyhow::bail!(
                    "{reason}",
                    reason = receipt["reason"].as_str().unwrap_or(&error.to_string())
                );
            }
        }
        let archived: Value = crate::state::read(&self.operation.file("config-archive.json"))?;
        ensure!(
            archived["status"] == "passed" && archived["validation"]["status"] == "passed",
            "archive or validation status is not passed"
        );
        self.commit = archived["candidate_commit"]
            .as_str()
            .context("archive result did not include candidate_commit")?
            .into();
        let _: GitRevision = self.commit.parse()?;
        Ok(())
    }

    /// Resolve only authenticated evaluated contracts, preserving the validator's generated-source checksum gate.
    fn resolve(&mut self) -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let members = [
            "config.bundle",
            "validation.json",
            "manifest.json",
            "topology.json",
            "system.json",
            "prepared-source.tar",
        ];
        io::unpack(
            &self.operation.file("config-candidate-validated.tar"),
            temporary.path(),
            &members,
            &members[..5],
        )?;
        let mut worktree = temporary.path().join("worktree");
        self.operation.logged(
            "git",
            &[
                "clone".into(),
                temporary.path().join("config.bundle").into_os_string(),
                worktree.as_os_str().to_owned(),
            ],
            &self.operation.file("config-apply-git-clone.log"),
            &[],
        )?;
        self.operation.logged(
            "git",
            &[
                "-C".into(),
                worktree.as_os_str().to_owned(),
                "checkout".into(),
                "--detach".into(),
                self.commit.as_str().into(),
            ],
            &self.operation.file("config-apply-git-checkout.log"),
            &[],
        )?;
        let topology: Value = crate::state::read(&temporary.path().join("topology.json"))?;
        let system: Value = crate::state::read(&temporary.path().join("system.json"))?;
        ensure!(
            topology["schema"] == "airlock.qubes-topology.v1"
                && ["domains", "templates", "kernels", "prefs"]
                    .iter()
                    .all(|key| topology[*key].is_object()),
            "validated candidate topology JSON is invalid"
        );
        ensure!(
            system["schema"] == "airlock.system.v1"
                && system["dom0"].is_object()
                && system["plan"].is_object()
                && system["plan"]["reboot_required"].is_boolean()
                && ["live_actions", "reboot_actions", "reboot_reasons"]
                    .iter()
                    .all(|key| system["plan"][*key].is_array()),
            "validated candidate system JSON is invalid"
        );
        for name in ["validation", "manifest"] {
            fs::copy(
                temporary.path().join(format!("{name}.json")),
                self.operation.file(&format!("config-apply-{name}.json")),
            )?;
        }
        io::save(
            &self
                .operation
                .file("config-apply-unsupported-domain-backends.json"),
            &json!({"schema":"airlock.unsupported-domain-backends.v1","domains":[]}),
        )?;
        if worktree.join("Cargo.toml").is_file() {
            let prepared = temporary.path().join("prepared-source");
            let validation: Value = crate::state::read(&temporary.path().join("validation.json"))?;
            airlock_nix_core::artifact::prepared::unpack(
                &temporary.path().join("prepared-source.tar"),
                &prepared,
                &self.commit.parse()?,
                validation["prepared_source"]["sha256"]
                    .as_str()
                    .context("validated Rust candidate is missing its prepared source checksum")?,
            )?;
            worktree = prepared;
        }
        build::execute(build::Request {
            topology: temporary.path().join("topology.json"),
            system: temporary.path().join("system.json"),
            worktree,
            commit: self.commit.parse()?,
        })
    }

    /// Apply the resolved VM topology through the fixed setup command and retain its exact receipt.
    fn setup(&mut self) -> Result<()> {
        let topology = self.operation.file("config-apply-topology.json");
        let program = io::setting("AIRLOCK_SETUP_CONFIG_COMMAND", "airlock-setup-config");
        host::pending(&self.operation.file("config-apply-system.json"), false)?;
        io::save(
            &self.operation.file("setup-config.json"),
            &json!({"schema":"airlock.setup-config.v1","status":"unknown","reason":"setup has not reported an outcome"}),
        )?;
        let result = self.operation.transfer(
            &program,
            &[
                "--apply".into(),
                "--topology".into(),
                topology.as_os_str().to_owned(),
            ],
            Files {
                input: None,
                output: &self.operation.file("config-apply-setup.stdout.log"),
                errors: &self.operation.file("config-apply-setup.stderr.log"),
                limit: 16 * 1024 * 1024,
            },
            &[],
        );
        let receipt: Value = crate::state::read(&self.operation.file("setup-config.json"))?;
        ensure!(
            result.is_ok() && receipt["status"] == "passed",
            "setup-config failed"
        );
        let state = io::path("AIRLOCK_SYSTEM_STATE_DIR", "/var/lib/airlock/system");
        fs::create_dir_all(&state)?;
        for (source, destination) in [
            ("config-apply-topology.json", "current-topology.json"),
            (
                "config-apply-topology-validated.json",
                "current-validated-topology.json",
            ),
            ("setup-config.json", "current-setup.json"),
        ] {
            fs::copy(self.operation.file(source), state.join(destination))?;
        }
        Ok(())
    }

    /// Retry only the existing transient qrexec connection failures, retaining the original payload.
    fn archive_rpc(&mut self, archive: &Path) -> Result<()> {
        let vm = self.request.vault.as_str();
        let service = io::setting(
            "AIRLOCK_CONFIG_DEPLOYMENT_ARCHIVE_SERVICE",
            "airlock.Config.ArchiveDeployment",
        );
        let runtime = io::path("AIRLOCK_QREXEC_RUN_DIR", "/var/run/qubes");
        let wait: u64 = io::setting("AIRLOCK_CONFIG_QREXEC_READY_WAIT_SECONDS", "60").parse()?;
        let output = self
            .operation
            .file("config-apply-deployment-archive.stdout.log");
        let errors = self
            .operation
            .file("config-apply-deployment-archive.stderr.log");
        for attempt in 0..3 {
            if runtime.is_dir() {
                let socket = runtime.join(format!("qrexec.{vm}"));
                let ready = || {
                    socket
                        .metadata()
                        .is_ok_and(|metadata| metadata.file_type().is_socket())
                };
                if !ready() {
                    let _ = self.operation.text("qvm-start", &[vm]);
                }
                let deadline = Instant::now()
                    .checked_add(Duration::from_secs(wait))
                    .context("qrexec wait is too large")?;
                while !ready() && Instant::now() < deadline {
                    thread::sleep(Duration::from_secs(1));
                }
                ensure!(ready(), "qrexec endpoint did not become ready for {vm}");
            }
            let result = self
                .operation
                .rpc(vm, &service, Some(archive), &output, &errors);
            match result {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let diagnostic = fs::read_to_string(&errors).unwrap_or_default();
                    let transient = diagnostic.contains("connect ")
                        && diagnostic.contains("/qrexec.")
                        && (diagnostic.contains(": No such file or directory")
                            || diagnostic.contains(": Connection refused"));
                    if !transient || attempt == 2 {
                        return Err(error);
                    }
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
        unreachable!()
    }

    /// Archive success or failure before updating the current/last deployment evidence pointers.
    fn archive(&mut self, status: &str, reason: &str) -> Result<()> {
        let at =
            time::OffsetDateTime::now_utc().format(&time::format_description::parse_borrowed::<
                2,
            >(
                "[year][month][day]T[hour][minute][second]Z",
            )?)?;
        let mut entropy = [0u8; 16];
        fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
        let attempt = entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.deployment = format!(
            "{at}-{short}-{attempt}",
            short = self.commit.chars().take(12).collect::<String>()
        );
        let mut record = json!({"schema":"airlock.config-deployment.v1","status":status,"reason":reason,"deployment_id":self.deployment,"deployed_at":at,"deployed_by":"dom0","staging":self.request.staging,"validator":self.request.validator,"vault":self.request.vault,"candidate_commit":self.commit,"artifacts":{}});
        for (key, file, member) in ARTIFACTS {
            let key = match *key {
                "system" => "resolved_system",
                "validated_system" => "system",
                other => other,
            };
            record["artifacts"][key] = json!(member);
            if [
                "archive",
                "setup",
                "system_apply",
                "systemd_actions",
                "nix_domain_builds",
                "nix_domain_materializations",
                "unsupported_domain_backends",
                "domain_references",
            ]
            .contains(&key)
            {
                record[key] = io::optional(&self.operation.file(file));
            }
        }
        record["artifacts"]["apply"] = json!("apply.json");
        io::save(&self.operation.file("config-deployment.json"), &record)?;
        self.receipt(status, reason)?;
        let temporary = tempfile::tempdir()?;
        let directory = temporary.path();
        fs::create_dir(directory.join("logs"))?;
        for (_, file, member) in ARTIFACTS {
            let content = match fs::read(self.operation.file(file)) {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => return Err(error.into()),
            };
            fs::write(directory.join(member), content)?;
        }
        for (_, file, member) in LOGS {
            let content = match fs::read(self.operation.file(file)) {
                Ok(value) => value,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(error) => return Err(error.into()),
            };
            fs::write(directory.join("logs").join(member), content)?;
        }
        fs::copy(
            self.operation.file("config-deployment.json"),
            directory.join("deployment.json"),
        )?;
        fs::copy(
            self.operation.file("config-apply.json"),
            directory.join("apply.json"),
        )?;
        let archive = self.operation.file("config-deployment.tar");
        let mut tar = tar::Builder::new(fs::File::create(&archive)?);
        for member in ["deployment.json", "apply.json"]
            .into_iter()
            .chain(ARTIFACTS.iter().map(|(_, _, member)| *member))
        {
            tar.append_path_with_name(directory.join(member), member)?;
        }
        for (_, _, member) in LOGS {
            let name = format!("logs/{member}");
            tar.append_path_with_name(directory.join(&name), &name)?;
        }
        tar.finish()?;
        drop(tar);
        self.archive_rpc(&archive)?;
        let receipt = io::result(
            &self
                .operation
                .file("config-apply-deployment-archive.stdout.log"),
            "airlock.config-vault-deployment.v1",
        )?;
        io::save(
            &self.operation.file("config-vault-deployment-archive.json"),
            &receipt,
        )?;
        ensure!(
            receipt["status"] == "passed" && receipt["candidate_commit"] == self.commit,
            "vault deployment candidate did not match or archival failed"
        );
        self.receipt(status, reason)?;
        let state = io::path("AIRLOCK_SYSTEM_STATE_DIR", "/var/lib/airlock/system");
        fs::create_dir_all(&state)?;
        for prefix in ["last", "current"] {
            if prefix == "current" && status != "passed" {
                continue;
            }
            fs::copy(
                self.operation.file("config-deployment.json"),
                state.join(format!("{prefix}-deployment.json")),
            )?;
            fs::copy(
                self.operation.file("config-apply.json"),
                state.join(format!("{prefix}-config-apply.json")),
            )?;
            fs::write(
                state.join(format!("{prefix}-deployment-id")),
                format!("{identity}\n", identity = self.deployment),
            )?;
            fs::write(
                state.join(format!("{prefix}-candidate-commit")),
                format!("{commit}\n", commit = self.commit),
            )?;
        }
        Ok(())
    }
}

/// Run the existing stage pipeline and preserve a failed result without inventing successful effects.
pub(super) fn execute(request: Request) -> Result<()> {
    ensure!(request.apply, "configuration apply requires --apply");
    let mut apply = Apply {
        operation: Operation::new("config-apply")?,
        request,
        stage: io::setting("AIRLOCK_CONFIG_APPLY_STAGE", "full"),
        commit: String::new(),
        deployment: String::new(),
    };
    let result: Result<()> = (|| {
        ensure!(
            ["full", "archive", "resolve", "setup", "system", "deploy"]
                .contains(&apply.stage.as_str()),
            "unsupported AIRLOCK_CONFIG_APPLY_STAGE: {stage}",
            stage = apply.stage
        );
        apply.operation.control_domain()?;
        apply.candidate()?;
        if apply.stage == "archive" {
            return Ok(());
        }
        if matches!(apply.stage.as_str(), "full" | "resolve") {
            apply.resolve()?;
        }
        if apply.stage == "resolve" {
            return Ok(());
        }
        if matches!(apply.stage.as_str(), "full" | "setup")
            && let Err(error) = apply.setup()
        {
            apply.archive("failed", "setup-config failed")?;
            return Err(error);
        }
        if apply.stage == "setup" {
            return Ok(());
        }
        if matches!(apply.stage.as_str(), "full" | "system")
            && host::execute(&apply.operation.file("config-apply-system.json")).is_err()
        {
            apply.archive("failed", "system apply failed")?;
            anyhow::bail!("system apply failed");
        }
        if apply.stage == "system" {
            return Ok(());
        }
        for file in ["setup-config.json", "config-system-apply.json"] {
            let record: Value = crate::state::read(&apply.operation.file(file))?;
            ensure!(record["status"] == "passed", "{file} status is not passed");
        }
        apply.archive("passed", "validated config candidate applied")
    })();
    let reason = match &result {
        Ok(()) => "validated config candidate applied".into(),
        Err(error) => error.to_string(),
    };
    let receipt = apply.receipt(if result.is_ok() { "passed" } else { "failed" }, &reason)?;
    if result.is_ok() {
        println!("{receipt}");
    }
    result
}
