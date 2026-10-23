//! Content-bound configuration validation shared by the offline validator, dom0 and vault.
//! Checked values describe source and content; existing Qubes policy and explicit administration supply permission.

use crate::{
    Error, Result,
    admission::CheckStatus,
    identity::{ConfigurationHash, GitRevision},
};
use airlock_machine_planning::validation as verified;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::Path};

/// Mandatory successful checks, in the order used by the independent completeness specification.
pub const REQUIRED_CHECKS: [CheckKind; 11] = [
    CheckKind::Manifest,
    CheckKind::Bundle,
    CheckKind::Clone,
    CheckKind::Clean,
    CheckKind::Flake,
    CheckKind::TopologyEval,
    CheckKind::TopologySchema,
    CheckKind::TopologyReferences,
    CheckKind::SystemEval,
    CheckKind::SystemSchema,
    CheckKind::SystemReferences,
];

/// Closed validation protocol; older unbound receipts require fresh validation before apply.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSchema {
    /// Complete content-bound validation evidence.
    #[serde(rename = "airlock.config-validation.v2")]
    V2,
}

/// Role expected on the authenticated offline validation channel.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorRole {
    /// The existing offline configuration validator; this marker alone authenticates no sender.
    #[serde(rename = "sys-config-validator")]
    Validator,
}

/// Named validation checks; unknown names cannot silently become optional checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckKind {
    /// Staged manifest source identity.
    #[serde(rename = "candidate-manifest")]
    Manifest,
    /// Source bundle integrity and declared head.
    #[serde(rename = "git-bundle-verify")]
    Bundle,
    /// Exact source checkout.
    #[serde(rename = "git-clone")]
    Clone,
    /// Absence of uncommitted source changes.
    #[serde(rename = "git-clean")]
    Clean,
    /// Offline flake evaluation checks.
    #[serde(rename = "nix-flake-check-no-build")]
    Flake,
    /// Evaluated topology output.
    #[serde(rename = "nix-eval-airlock-topology")]
    TopologyEval,
    /// Supported topology shape.
    #[serde(rename = "airlock-topology-json")]
    TopologySchema,
    /// Declared VM source references.
    #[serde(rename = "nix-template-source-refs-dry-run")]
    TopologyReferences,
    /// Evaluated host output.
    #[serde(rename = "nix-eval-airlock-system")]
    SystemEval,
    /// Supported host configuration shape.
    #[serde(rename = "airlock-system-json")]
    SystemSchema,
    /// Declared host source references.
    #[serde(rename = "nix-system-source-refs-dry-run")]
    SystemReferences,
    /// Optional full build, whose explicit skip does not stand in for a mandatory check.
    #[serde(rename = "nix-flake-check")]
    FullBuild,
}

impl CheckKind {
    /// Exhaustively project known wire names into the production completeness checker.
    fn code(self) -> u8 {
        match self {
            Self::Manifest => 0,
            Self::Bundle => 1,
            Self::Clone => 2,
            Self::Clean => 3,
            Self::Flake => 4,
            Self::TopologyEval => 5,
            Self::TopologySchema => 6,
            Self::TopologyReferences => 7,
            Self::SystemEval => 8,
            Self::SystemSchema => 9,
            Self::SystemReferences => 10,
            Self::FullBuild => 11,
        }
    }
}

/// Recorded outcome; skipped mandatory work never counts as success.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    /// The check completed successfully.
    Passed,
    /// The check failed.
    Failed,
    /// An optional check was deliberately not run.
    Skipped,
}

impl CheckOutcome {
    /// Preserve all three outcome meanings at the verified boundary.
    fn evidence(self) -> verified::Status {
        match self {
            Self::Passed => verified::Status::Passed,
            Self::Failed => verified::Status::Failed,
            Self::Skipped => verified::Status::Skipped,
        }
    }
}

/// One check and its non-authoritative diagnostic details.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationCheck {
    /// Supported check identity.
    pub name: CheckKind,
    /// Actual check outcome.
    pub status: CheckOutcome,
    /// Logs and diagnostic fields, excluded from permission decisions.
    #[serde(flatten)]
    pub details: BTreeMap<String, Value>,
}

/// Digests of the exact files exported by the validator, without reserializing their JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactHashes {
    /// Original submitted Git bundle bytes.
    pub bundle: ConfigurationHash,
    /// Original submitted manifest bytes.
    pub manifest: ConfigurationHash,
    /// Evaluated topology bytes.
    pub topology: ConfigurationHash,
    /// Evaluated host contract bytes.
    pub system: ConfigurationHash,
}

impl ArtifactHashes {
    /// Hash the exact candidate files without executing or reserializing their contents.
    pub fn read(directory: &Path) -> Result<Self> {
        Ok(Self {
            bundle: file_hash(&directory.join("config.bundle"))?,
            manifest: file_hash(&directory.join("manifest.json"))?,
            topology: file_hash(&directory.join("topology.json"))?,
            system: file_hash(&directory.join("system.json"))?,
        })
    }
}

/// Only this fixed optional archive may carry generated Rust/Rusix build inputs.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreparedArchive {
    /// Generated source archive, interpreted as data in dom0.
    #[serde(rename = "prepared-source.tar")]
    Source,
}

/// Generated-source identity recorded by the validator.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedSource {
    /// Fixed generated-source archive member.
    pub archive: PreparedArchive,
    /// Digest of its complete archive bytes.
    pub sha256: ConfigurationHash,
}

/// Content-bound validation response. Decoding does not establish that checks succeeded.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRecord {
    /// Closed content-bound protocol version.
    pub schema: ValidationSchema,
    /// Summary outcome, checked together with every required individual result.
    pub status: CheckStatus,
    /// Human diagnostic retained on both success and failure.
    pub reason: String,
    /// Expected offline role; authenticated transport remains a separate requirement.
    pub validator_role: ValidatorRole,
    /// Source commit claimed by validation.
    pub commit: GitRevision,
    /// Complete exported artifact identity; failed validation may lack outputs.
    pub artifacts: Option<ArtifactHashes>,
    /// Generated-source identity, absent only for candidates not requiring it.
    pub prepared_source: Option<PreparedSource>,
    /// Named evidence, including deliberate skips.
    pub checks: Vec<ValidationCheck>,
}

/// Original submitted source identity, captured before contacting the validator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubmittedConfiguration {
    /// Exact requested Git source.
    pub commit: GitRevision,
    /// Digest of its submitted bundle.
    pub bundle: ConfigurationHash,
    /// Digest of its submitted manifest.
    pub manifest: ConfigurationHash,
}

impl SubmittedConfiguration {
    /// Preserve every source coordinate at the verified boundary.
    fn evidence(&self) -> verified::Source {
        verified::Source {
            commit: self.commit.to_string(),
            bundle: self.bundle.to_string(),
            manifest: self.manifest.to_string(),
        }
    }
}

/// Complete validated-candidate identity retained by resolution records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationIdentity {
    /// Original submitted source.
    pub source: SubmittedConfiguration,
    /// Evaluated topology content.
    pub topology: ConfigurationHash,
    /// Evaluated host content.
    pub system: ConfigurationHash,
    /// Optional generated-source content, including its presence.
    pub prepared: Option<ConfigurationHash>,
}

impl ConfigurationIdentity {
    /// Project the complete identity without dropping optional-source presence.
    fn evidence(&self) -> verified::Binding {
        verified::Binding {
            source: self.source.evidence(),
            topology: self.topology.to_string(),
            system: self.system.to_string(),
            prepared: self.prepared.as_ref().map(ToString::to_string),
        }
    }
}

impl ValidationRecord {
    /// Read the claimed content identity; only [`ValidatedConfiguration::check`] authenticates it.
    pub fn identity(&self) -> Result<ConfigurationIdentity> {
        let artifacts = self
            .artifacts
            .as_ref()
            .ok_or_else(|| invalid("missing evaluated artifact hashes"))?;
        Ok(ConfigurationIdentity {
            source: SubmittedConfiguration {
                commit: self.commit.clone(),
                bundle: artifacts.bundle.clone(),
                manifest: artifacts.manifest.clone(),
            },
            topology: artifacts.topology.clone(),
            system: artifacts.system.clone(),
            prepared: self.prepared_source.as_ref().map(|p| p.sha256.clone()),
        })
    }
}

/// Supported candidate manifest, shared by staging observation and validation consumers.
#[derive(Debug, Deserialize)]
pub struct CandidateManifest {
    /// Closed staged-candidate schema.
    pub schema: CandidateSchema,
    /// Selected source commit.
    pub commit: GitRevision,
}

/// Closed staged source protocol.
#[derive(Debug, Deserialize)]
pub enum CandidateSchema {
    /// Existing inert source bundle and manifest.
    #[serde(rename = "airlock.config-candidate.v1")]
    V1,
}

/// Existing archival response protocol, extended with mandatory content binding for apply.
#[derive(Debug, Serialize, Deserialize)]
pub enum ArchiveSchema {
    /// Archival and validation evidence, separate from deployment authority.
    #[serde(rename = "airlock.config-archive.v1")]
    V1,
}

/// Protected archive result consumed by staged apply; older records lacking bindings fail closed.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArchivedConfiguration {
    /// Supported archive response format.
    pub schema: ArchiveSchema,
    /// Archival outcome; failed archival cannot enter apply.
    pub status: CheckStatus,
    /// Candidate commit also checked against the original submission.
    pub candidate_commit: GitRevision,
    /// Original source captured before validation.
    pub submitted: SubmittedConfiguration,
    /// Exact validated archive snapshot retained by dom0.
    pub validated_candidate_sha256: ConfigurationHash,
    /// Full original validation evidence, not a reduced success flag.
    pub validation: ValidationRecord,
    /// Informational filenames, timings and vault results.
    #[serde(flatten)]
    pub details: BTreeMap<String, Value>,
}

impl ArchivedConfiguration {
    /// Refuse failed or contradictory archive summaries; actual artifacts still require independent validation.
    pub fn check(&self) -> Result<()> {
        if self.status != CheckStatus::Passed
            || self.validation.status != CheckStatus::Passed
            || self.candidate_commit != self.submitted.commit
            || self.validation.commit != self.submitted.commit
        {
            return Err(invalid("consistent successful archive identity"));
        }
        Ok(())
    }
}

/// Raw received documents; none is accepted before its digest and shape are checked.
pub struct CandidateDocuments {
    /// Exact submitted manifest bytes.
    pub manifest: Vec<u8>,
    /// Exact evaluated topology bytes.
    pub topology: Vec<u8>,
    /// Exact evaluated host bytes.
    pub system: Vec<u8>,
}

impl CandidateDocuments {
    /// Read bounded documents from a private extracted snapshot, preserving the exact bytes being hashed.
    pub fn read(directory: &Path) -> Result<Self> {
        fn read(path: &Path) -> Result<Vec<u8>> {
            let input = std::fs::File::open(path)
                .map_err(|e| crate::error::io_error("open configuration document", path, e))?;
            let mut bytes = Vec::new();
            input
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| crate::error::io_error("read configuration document", path, e))?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err(Error::RecordTooLarge {
                    path: path.to_owned(),
                    limit: 16 * 1024 * 1024,
                });
            }
            Ok(bytes)
        }
        Ok(Self {
            manifest: read(&directory.join("manifest.json"))?,
            topology: read(&directory.join("topology.json"))?,
            system: read(&directory.join("system.json"))?,
        })
    }
}

/// Independently inspected non-JSON artifacts from the same private received snapshot.
pub struct CandidateFiles {
    /// Head independently read by Git from the received bundle.
    pub bundle_commit: GitRevision,
    /// Actual bundle content digest.
    pub bundle: ConfigurationHash,
    /// Actual optional generated archive digest.
    pub prepared: Option<ConfigurationHash>,
}

/// Hash exactly the supplied bytes, without changing whitespace or JSON ordering.
pub fn content_hash(bytes: &[u8]) -> ConfigurationHash {
    format!("{hash:x}", hash = Sha256::digest(bytes))
        .parse()
        .expect("SHA-256 produces lowercase hexadecimal")
}

/// Stream an artifact digest; the caller must supply a stable, authenticated file snapshot.
pub fn file_hash(path: &Path) -> Result<ConfigurationHash> {
    let mut input = std::fs::File::open(path)
        .map_err(|e| crate::error::io_error("open configuration artifact", path, e))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|e| crate::error::io_error("hash configuration artifact", path, e))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    format!("{hash:x}", hash = hash.finalize()).parse()
}

/// Keep correspondence failures distinguishable from transport errors.
fn invalid(expected: &'static str) -> Error {
    Error::InvalidField {
        field: "validated configuration",
        expected,
    }
}

/// Decode one already bounded protocol document, rejecting ambiguous duplicate fields.
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], origin: &str) -> Result<T> {
    crate::state::parse_json(bytes, Path::new(origin))
}

/// Check supported document shapes; the content binding covers all their remaining fields.
fn documents(topology: &[u8], system: &[u8]) -> Result<(Value, Value)> {
    let topology: Value = decode(topology, "topology.json")?;
    let system: Value = decode(system, "system.json")?;
    if topology["schema"] != "airlock.qubes-topology.v1"
        || !["templates", "kernels", "prefs", "domains"]
            .iter()
            .all(|k| topology[*k].is_object())
        || !topology["qrexec_policy"]["rules"].is_array()
        || system["schema"] != "airlock.system.v1"
        || !system["dom0"].is_object()
        || !["live_actions", "reboot_actions", "reboot_reasons"]
            .iter()
            .all(|k| system["plan"][*k].is_array())
        || !system["plan"]["reboot_required"].is_boolean()
    {
        return Err(invalid("supported evaluated topology and host contract"));
    }
    Ok((topology, system))
}

/// Permit only declared source materialization, preserving every policy, VM, host and action field.
/// This JSON projection is a tested adapter; the executable Verus gate binds its sealed outputs to the input.
fn resolution_shape(
    original_topology: &Value,
    original_system: &Value,
    topology: &Value,
    system: &Value,
) -> Result<()> {
    /// Restore only materialized source coordinates, retaining all other descriptor fields for comparison.
    fn descriptor(original: &Value, resolved: Option<&mut Value>) -> Result<()> {
        if original["source"].is_null() && original["source_ref"].is_null() {
            return Ok(());
        }
        let resolved = resolved
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("resolved declared source descriptor"))?;
        if !resolved
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
            || resolved.get("source_ref") != Some(&Value::Null)
        {
            return Err(invalid("concrete source and cleared source reference"));
        }
        for key in ["source", "source_ref"] {
            if let Some(value) = original.get(key) {
                resolved.insert(key.into(), value.clone());
            } else {
                resolved.remove(key);
            }
        }
        Ok(())
    }
    let mut normalized_topology = topology.clone();
    let mut normalized_system = system.clone();
    for (name, original) in original_topology["templates"]
        .as_object()
        .ok_or_else(|| invalid("template declarations"))?
    {
        if !original["source_ref"].is_null() {
            descriptor(
                original,
                normalized_topology
                    .get_mut("templates")
                    .and_then(|v| v.get_mut(name)),
            )?;
        }
    }
    for (name, original) in original_topology["domains"]
        .as_object()
        .ok_or_else(|| invalid("domain declarations"))?
    {
        if original["backend"]["type"] == "nix-closure" {
            for key in ["system", "kernel", "initrd", "modules", "root", "store"] {
                let before = &original["backend"][key];
                if !before.is_null() {
                    descriptor(
                        before,
                        normalized_topology
                            .get_mut("domains")
                            .and_then(|v| v.get_mut(name))
                            .and_then(|v| v.get_mut("backend"))
                            .and_then(|v| v.get_mut(key)),
                    )?;
                }
            }
        }
    }
    for key in ["profile", "system", "kernel", "xen", "bootloader", "qubes"] {
        let before = if key == "profile" {
            &original_system["dom0"][key]
        } else {
            &original_system["dom0"]["nixos"][key]
        };
        if !before.is_null() {
            let dom0 = normalized_system.get_mut("dom0");
            let after = if key == "profile" {
                dom0.and_then(|v| v.get_mut(key))
            } else {
                dom0.and_then(|v| v.get_mut("nixos"))
                    .and_then(|v| v.get_mut(key))
            };
            descriptor(before, after)?;
        }
    }
    if normalized_topology != *original_topology || normalized_system != *original_system {
        return Err(invalid(
            "source resolution must preserve all non-source configuration fields",
        ));
    }
    Ok(())
}

/// Validated source and immutable evaluated documents; callers cannot replace either document.
///
/// ```compile_fail,E0616
/// use airlock_nix_core::configuration::ValidatedConfiguration;
/// /// A caller cannot replace the checked topology with independently supplied JSON.
/// fn substitute(checked: &mut ValidatedConfiguration, topology: serde_json::Value) {
///     checked.topology = topology;
/// }
/// ```
pub struct ValidatedConfiguration {
    // Complete accepted content identity.
    identity: ConfigurationIdentity,
    // Original validator receipt, retained for archival without rebuilding its fields.
    record: ValidationRecord,
    // Checked topology values supplied to the actual resolver.
    topology: Value,
    // Checked host values supplied to that same resolver.
    system: Value,
    // Exact original document bytes retained for content-verifiable archival evidence.
    raw: CandidateDocuments,
    // Verified correspondence and complete mandatory-check evidence.
    evidence: verified::Validated,
}

impl ValidatedConfiguration {
    /// Authenticate received artifacts against both the original submission and complete validator evidence.
    pub fn check(
        submitted: &SubmittedConfiguration,
        record: ValidationRecord,
        raw: CandidateDocuments,
        files: CandidateFiles,
    ) -> Result<Self> {
        let manifest: CandidateManifest = decode(&raw.manifest, "manifest.json")?;
        let actual = ConfigurationIdentity {
            source: SubmittedConfiguration {
                commit: files.bundle_commit,
                bundle: files.bundle,
                manifest: content_hash(&raw.manifest),
            },
            topology: content_hash(&raw.topology),
            system: content_hash(&raw.system),
            prepared: files.prepared,
        };
        let identity = record.identity()?;
        let checks = record
            .checks
            .iter()
            .map(|c| verified::Check {
                kind: c.name.code(),
                status: c.status.evidence(),
            })
            .collect();
        let status = match record.status {
            CheckStatus::Passed => verified::Status::Passed,
            CheckStatus::Failed => verified::Status::Failed,
        };
        let evidence = verified::Validated::check(
            submitted.evidence(),
            identity.evidence(),
            &actual.evidence(),
            manifest.commit.as_str(),
            status,
            checks,
        )
        .ok_or_else(|| {
            invalid("complete successful validation of the exact submitted artifacts")
        })?;
        let (topology, system) = documents(&raw.topology, &raw.system)?;
        Ok(Self {
            identity,
            record,
            topology,
            system,
            raw,
            evidence,
        })
    }

    /// Original content identity, including generated-source presence.
    pub fn identity(&self) -> &ConfigurationIdentity {
        &self.identity
    }

    /// Original complete validator receipt for persistence and diagnostics.
    pub fn record(&self) -> &ValidationRecord {
        &self.record
    }

    /// Immutable evaluated topology supplied to source resolution.
    pub fn topology(&self) -> &Value {
        &self.topology
    }

    /// Immutable evaluated host contract supplied to source resolution.
    pub fn system(&self) -> &Value {
        &self.system
    }

    /// Exact evaluated topology bytes retained for audit, before any source resolution.
    pub fn topology_bytes(&self) -> &[u8] {
        &self.raw.topology
    }

    /// Exact evaluated host bytes retained for audit, before any source resolution.
    pub fn system_bytes(&self) -> &[u8] {
        &self.raw.system
    }

    /// Seal actual resolver outputs while retaining this exact validated input identity.
    /// Source-reference resolution itself remains ordinary Rust and external Nix execution.
    pub fn resolved(self, topology: &Value, system: &Value) -> Result<ResolvedConfiguration> {
        let topology = serde_json::to_vec(topology).map_err(|source| Error::Json {
            path: "resolved topology".into(),
            source,
        })?;
        let system = serde_json::to_vec(system).map_err(|source| Error::Json {
            path: "resolved system".into(),
            source,
        })?;
        let record = ResolutionRecord {
            schema: ResolutionSchema::V1,
            input: self.identity.clone(),
            topology: content_hash(&topology),
            system: content_hash(&system),
        };
        ResolvedConfiguration::restore(self, record, topology, system)
    }
}

/// Closed persisted resolution format.
#[derive(Debug, Serialize, Deserialize)]
pub enum ResolutionSchema {
    /// Exact original input and resolved-output binding.
    #[serde(rename = "airlock.config-resolution.v1")]
    V1,
}

/// Persisted correspondence for explicit setup/system stage retries; this grants no permission.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionRecord {
    /// Supported binding format.
    pub schema: ResolutionSchema,
    /// Complete original validation identity.
    pub input: ConfigurationIdentity,
    /// Actual resolver-produced topology bytes.
    pub topology: ConfigurationHash,
    /// Actual resolver-produced host bytes.
    pub system: ConfigurationHash,
}

/// Resolved documents sealed against the original validated candidate and their own content digests.
pub struct ResolvedConfiguration {
    // Persisted input/output binding retained for explicit later stages.
    record: ResolutionRecord,
    // Exact resolved topology bytes; no mutable access is exposed.
    topology: Vec<u8>,
    // Exact resolved host bytes; no mutable access is exposed.
    system: Vec<u8>,
    // Verified content selection token, without activation authority.
    evidence: verified::ApplyInput,
}

impl ResolvedConfiguration {
    /// Exact bytes sealed as the resolved topology, also used for stage-retry evidence.
    pub fn topology_bytes(&self) -> &[u8] {
        &self.topology
    }

    /// Exact bytes sealed as the resolved host contract, also used for stage-retry evidence.
    pub fn system_bytes(&self) -> &[u8] {
        &self.system
    }

    /// Restore a staged result only after revalidating its original candidate and both actual output documents.
    pub fn restore(
        validated: ValidatedConfiguration,
        record: ResolutionRecord,
        topology: Vec<u8>,
        system: Vec<u8>,
    ) -> Result<Self> {
        let (topology_value, system_value) = documents(&topology, &system)?;
        resolution_shape(
            validated.topology(),
            validated.system(),
            &topology_value,
            &system_value,
        )?;
        let evidence = validated
            .evidence
            .resolve(
                &record.input.evidence(),
                record.topology.to_string(),
                record.system.to_string(),
            )
            .and_then(|r| {
                r.select(
                    content_hash(&topology).as_str(),
                    content_hash(&system).as_str(),
                )
            })
            .ok_or_else(|| invalid("unchanged original validation and resolved output content"))?;
        Ok(Self {
            record,
            topology,
            system,
            evidence,
        })
    }

    /// Persisted input/output correspondence; reloading it still requires fresh validation of actual artifacts.
    pub fn record(&self) -> &ResolutionRecord {
        &self.record
    }

    /// Publish private input snapshots for the apply adapters, avoiding mutable diagnostic files as authority.
    pub fn inputs(self) -> Result<ApplyInputs> {
        let directory = tempfile::tempdir().map_err(|e| {
            crate::error::io_error(
                "create configuration input directory",
                Path::new("<temporary>"),
                e,
            )
        })?;
        for (name, bytes) in [
            ("topology.json", self.topology),
            ("system.json", self.system),
        ] {
            let path = directory.path().join(name);
            std::fs::write(&path, bytes).map_err(|e| {
                crate::error::io_error("write checked configuration input", &path, e)
            })?;
        }
        Ok(ApplyInputs {
            directory,
            evidence: self.evidence,
        })
    }
}

/// Private snapshots consumed by setup and host apply; possessing these is not permission to execute them.
pub struct ApplyInputs {
    // Owns the exact private snapshots for the entire adapter invocation.
    directory: tempfile::TempDir,
    // Retains the original verified candidate/output correspondence.
    evidence: verified::ApplyInput,
}

impl ApplyInputs {
    /// Original validated commit carried through the production verified resolution gate.
    pub fn commit(&self) -> &str {
        self.evidence.commit()
    }

    /// Exact checked topology snapshot, never an independently supplied filename.
    pub fn topology(&self) -> std::path::PathBuf {
        self.directory.path().join("topology.json")
    }

    /// Exact checked host snapshot, never a mutable diagnostic record.
    pub fn system(&self) -> std::path::PathBuf {
        self.directory.path().join("system.json")
    }
}
