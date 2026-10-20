//! Validate candidate artifacts in the existing offline validator and archive them in the vault.

use super::{
    io::{self, Operation},
    references,
};
use anyhow::{Context, Result, ensure};
use clap::Args;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

/// Existing archive command options, retaining role-specific environment defaults.
#[derive(Args)]
pub(crate) struct Request {
    /// Offline staging VM used only when no candidate archive was supplied.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_STAGING_VM",
        default_value = "sys-config-staging"
    )]
    staging: airlock_nix_core::identity::GuestName,
    /// Offline validator VM that returns evaluated and validated artifacts.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_VALIDATOR_VM",
        default_value = "sys-config-validator"
    )]
    validator: airlock_nix_core::identity::GuestName,
    /// Offline vault receiving the validated candidate.
    #[arg(
        long,
        env = "AIRLOCK_CONFIG_VAULT_VM",
        default_value = "sys-config-vault"
    )]
    vault: airlock_nix_core::identity::GuestName,
    /// Caller-supplied staged bundle and manifest; otherwise export them from staging.
    #[arg(long)]
    candidate_tar: Option<PathBuf>,
    /// Validate without requesting archival in the vault.
    #[arg(long)]
    validate_only: bool,
}

/// Check the returned schemas, references and Git bundle commit against the validation identity.
fn verify(operation: &mut Operation) -> Result<String> {
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
        &operation.file("config-candidate-validated.tar"),
        temporary.path(),
        &members,
        &members[..5],
    )?;
    for name in ["validation", "manifest", "topology", "system"] {
        fs::copy(
            temporary.path().join(format!("{name}.json")),
            operation.file(&format!("config-candidate-{name}.json")),
        )?;
    }
    let read = |name: &str| -> Result<Value> {
        crate::state::read(&temporary.path().join(format!("{name}.json")))
    };
    let manifest = read("manifest")?;
    let validation = read("validation")?;
    let topology = read("topology")?;
    let system = read("system")?;
    ensure!(
        topology["schema"] == "airlock.qubes-topology.v1"
            && ["templates", "kernels", "prefs", "domains"]
                .iter()
                .all(|key| topology[*key].is_object()),
        "evaluated candidate topology JSON is invalid"
    );
    ensure!(
        system["schema"] == "airlock.system.v1"
            && system["dom0"].is_object()
            && system["plan"].is_object()
            && ["live_actions", "reboot_actions", "reboot_reasons"]
                .iter()
                .all(|key| system["plan"][*key].is_array())
            && system["plan"]["reboot_required"].is_boolean(),
        "evaluated candidate system JSON is invalid"
    );
    ensure!(
        manifest["schema"] == "airlock.config-candidate.v1"
            && validation["schema"] == "airlock.config-validation.v1"
            && validation["status"] == "passed",
        "candidate manifest or validation is invalid"
    );
    let commit = validation["commit"]
        .as_str()
        .context("missing validation commit")?;
    let _: airlock_nix_core::identity::GitRevision = commit.parse()?;
    ensure!(
        manifest["commit"] == commit,
        "candidate manifest commit does not match validation commit"
    );
    references::validate(
        &operation.file("config-candidate-topology.json"),
        &operation.file("config-candidate-domain-references.json"),
    )?;
    let repo = temporary.path().join("verify.git");
    let repo = repo.to_str().context("Git verification path")?;
    operation.text("git", &["init", "--bare", "-b", "main", repo])?;
    let bundle = temporary.path().join("config.bundle");
    let bundle = bundle.to_str().context("bundle path")?;
    let verification = operation.text("git", &["-C", repo, "bundle", "verify", bundle])?;
    fs::write(
        operation.file("config-candidate-bundle-verify.log"),
        verification,
    )?;
    let heads = operation.text("git", &["-C", repo, "bundle", "list-heads", bundle])?;
    let candidate = heads
        .split_whitespace()
        .next()
        .context("candidate bundle did not expose a commit")?;
    ensure!(
        candidate == commit,
        "candidate commit {candidate} does not match validation commit {commit}"
    );
    Ok(commit.into())
}

/// Preserve archive results on every handled failure without confusing them with deployment authority.
pub(super) fn execute(request: Request) -> Result<()> {
    let mut operation = Operation::new("config-archive")?;
    let staging = request.staging.as_str();
    let validator = request.validator.as_str();
    let vault = request.vault.as_str();
    let export = io::setting(
        "AIRLOCK_CONFIG_CANDIDATE_EXPORT_SERVICE",
        "airlock.Config.CandidateExport",
    );
    let validate = io::setting(
        "AIRLOCK_CONFIG_CANDIDATE_VALIDATE_SERVICE",
        "airlock.Config.ValidateCandidate",
    );
    let archive = io::setting(
        "AIRLOCK_CONFIG_ARCHIVE_SERVICE",
        "airlock.Config.ArchiveCandidate",
    );
    let mut commit = String::new();
    let mut vault_commit = String::new();
    let result = (|| {
        operation.control_domain()?;
        if request.candidate_tar.is_none() {
            operation.offline(staging)?;
            operation.start(staging)?;
        }
        operation.offline(validator)?;
        operation.start(validator)?;
        if !request.validate_only {
            operation.offline(vault)?;
            operation.start(vault)?;
        }
        let staged = operation.file("config-candidate-staged.tar");
        let validated = operation.file("config-candidate-validated.tar");
        if let Some(candidate) = &request.candidate_tar {
            fs::copy(candidate, &staged)?;
        } else {
            operation.rpc(
                staging,
                &export,
                None,
                &staged,
                &operation.file("config-candidate-export.stderr.log"),
            )?;
        }
        operation.rpc(
            validator,
            &validate,
            Some(&staged),
            &validated,
            &operation.file("config-candidate-validate.stderr.log"),
        )?;
        commit = verify(&mut operation)?;
        if !request.validate_only {
            let output = operation.file("config-vault-archive.log");
            operation.rpc(
                vault,
                &archive,
                Some(&validated),
                &output,
                &operation.file("config-vault-archive.stderr.log"),
            )?;
            let archived = io::result(&output, "airlock.config-vault-archive.v1")?;
            io::save(&operation.file("config-vault-archive.json"), &archived)?;
            ensure!(
                archived["status"] == "passed" && archived["candidate_commit"] == commit,
                "vault archived a different candidate or reported failure"
            );
            vault_commit = archived["vault_commit"]
                .as_str()
                .context("missing vault commit")?
                .into();
        }
        Ok(())
    })();
    let mut receipt = json!({"schema":"airlock.config-archive.v1", "status":if result.is_ok() {"passed"} else {"failed"}, "reason":match &result { Ok(()) => "config candidate validated in sys-config-validator".into(), Err(error) => format!("{error:#}") }, "staging":staging,"validator":validator,"vault":vault,"candidate_export_service":export,"candidate_validate_service":validate,"archive_service":archive,"candidate_commit":commit,"validation_commit":commit,"vault_commit":vault_commit});
    for (key, file) in [
        ("staged_candidate_tar", "config-candidate-staged.tar"),
        ("validated_candidate_tar", "config-candidate-validated.tar"),
        ("topology", "config-candidate-topology.json"),
        ("system", "config-candidate-system.json"),
        (
            "domain_references",
            "config-candidate-domain-references.json",
        ),
        ("command_log", "config-archive.commands.log"),
        ("export_stderr", "config-candidate-export.stderr.log"),
        ("validate_stderr", "config-candidate-validate.stderr.log"),
        ("bundle_verify_log", "config-candidate-bundle-verify.log"),
        ("vault_archive_log", "config-vault-archive.log"),
        ("vault_archive_stderr", "config-vault-archive.stderr.log"),
    ] {
        receipt[key] = json!(operation.file(file));
    }
    for (key, file) in [
        ("validation", "config-candidate-validation.json"),
        (
            "domain_references_result",
            "config-candidate-domain-references.json",
        ),
        ("vault_archive", "config-vault-archive.json"),
    ] {
        receipt[key] = io::optional(&operation.file(file));
    }
    io::save(&operation.file("config-archive.json"), &receipt)?;
    if result.is_ok() {
        println!("{receipt}");
    }
    result
}
