//! Production validation and staged resolution reject conflicting sources, checks and content.

use airlock_nix_core::{admission::CheckStatus, configuration::*};
use serde_json::{Value, json};

/// A supported evaluated VM topology with no runtime side effects.
fn topology() -> Value {
    json!({"schema":"airlock.qubes-topology.v1","templates":{},"kernels":{},"domains":{},"prefs":{},"qrexec_policy":{"rules":[]}})
}

/// A supported evaluated host contract with an explicit no-reboot plan.
fn system() -> Value {
    json!({"schema":"airlock.system.v1","dom0":{},"plan":{"live_actions":[],"reboot_actions":[],"reboot_reasons":[],"reboot_required":false}})
}

/// Independently supplied source material, receipt and observed files used by the real constructor.
fn evidence() -> (
    SubmittedConfiguration,
    ValidationRecord,
    CandidateDocuments,
    CandidateFiles,
) {
    let manifest = serde_json::to_vec(
        &json!({"schema":"airlock.config-candidate.v1","commit":"a".repeat(40)}),
    )
    .unwrap();
    let raw = CandidateDocuments {
        manifest,
        topology: serde_json::to_vec(&topology()).unwrap(),
        system: serde_json::to_vec(&system()).unwrap(),
    };
    let source = SubmittedConfiguration {
        commit: "a".repeat(40).parse().unwrap(),
        bundle: content_hash(b"original bundle"),
        manifest: content_hash(&raw.manifest),
    };
    let record = ValidationRecord {
        schema: ValidationSchema::V2,
        status: CheckStatus::Passed,
        reason: "validated".into(),
        validator_role: ValidatorRole::Validator,
        commit: source.commit.clone(),
        artifacts: Some(ArtifactHashes {
            bundle: source.bundle.clone(),
            manifest: source.manifest.clone(),
            topology: content_hash(&raw.topology),
            system: content_hash(&raw.system),
        }),
        prepared_source: None,
        checks: REQUIRED_CHECKS
            .into_iter()
            .map(|name| ValidationCheck {
                name,
                status: CheckOutcome::Passed,
                details: Default::default(),
            })
            .collect(),
    };
    let actual = CandidateFiles {
        bundle_commit: source.commit.clone(),
        bundle: source.bundle.clone(),
        prepared: None,
    };
    (source, record, raw, actual)
}

/// Create checked evidence only through the production constructor.
fn validated() -> ValidatedConfiguration {
    let (source, record, raw, actual) = evidence();
    ValidatedConfiguration::check(&source, record, raw, actual).unwrap()
}

#[test]
fn every_mandatory_check_is_required_and_only_the_full_build_can_be_skipped() {
    for defect in [
        "missing",
        "duplicate",
        "failed",
        "skipped",
        "summary",
        "missing-artifacts",
        "unknown",
        "legacy",
        "wrong-role",
    ] {
        let (source, mut record, raw, actual) = evidence();
        match defect {
            "missing" => {
                record.checks.pop();
            }
            "duplicate" => record.checks.push(ValidationCheck {
                name: CheckKind::Clean,
                status: CheckOutcome::Passed,
                details: Default::default(),
            }),
            "failed" => record.checks[0].status = CheckOutcome::Failed,
            "skipped" => record.checks[0].status = CheckOutcome::Skipped,
            "summary" => record.status = CheckStatus::Failed,
            "missing-artifacts" => record.artifacts = None,
            other => {
                let mut wire = serde_json::to_value(record).unwrap();
                match other {
                    "unknown" => wire["checks"][0]["name"] = json!("unknown-check"),
                    "legacy" => wire["schema"] = json!("airlock.config-validation.v1"),
                    "wrong-role" => wire["validator_role"] = json!("config-staging-local"),
                    _ => unreachable!(),
                }
                assert!(serde_json::from_value::<ValidationRecord>(wire).is_err());
                continue;
            }
        }
        assert!(
            ValidatedConfiguration::check(&source, record, raw, actual).is_err(),
            "accepted {defect}"
        );
    }
    let (source, mut record, raw, actual) = evidence();
    record.checks.push(ValidationCheck {
        name: CheckKind::FullBuild,
        status: CheckOutcome::Skipped,
        details: Default::default(),
    });
    ValidatedConfiguration::check(&source, record, raw, actual).unwrap();
}

#[test]
fn submission_manifest_bundle_and_all_evaluated_content_must_agree() {
    for defect in [
        "source",
        "bundle",
        "head",
        "manifest",
        "topology",
        "system",
        "prepared-missing",
        "prepared-extra",
        "prepared-changed",
    ] {
        let (mut source, mut record, mut raw, mut actual) = evidence();
        match defect {
            "source" => source.commit = "b".repeat(40).parse().unwrap(),
            "bundle" => actual.bundle = content_hash(b"substituted bundle"),
            "head" => actual.bundle_commit = "b".repeat(40).parse().unwrap(),
            "manifest" => {
                raw.manifest = serde_json::to_vec(
                    &json!({"schema":"airlock.config-candidate.v1","commit":"b".repeat(40)}),
                )
                .unwrap()
            }
            "topology" => raw.topology = serde_json::to_vec(&json!({"new":"unvalidated"})).unwrap(),
            "system" => raw.system = serde_json::to_vec(&json!({"new":"unvalidated"})).unwrap(),
            "prepared-missing" => {
                record.prepared_source = Some(PreparedSource {
                    archive: PreparedArchive::Source,
                    sha256: content_hash(b"prepared"),
                })
            }
            "prepared-extra" => actual.prepared = Some(content_hash(b"prepared")),
            "prepared-changed" => {
                actual.prepared = Some(content_hash(b"substituted"));
                record.prepared_source = Some(PreparedSource {
                    archive: PreparedArchive::Source,
                    sha256: content_hash(b"prepared"),
                });
            }
            _ => unreachable!(),
        }
        assert!(
            ValidatedConfiguration::check(&source, record, raw, actual).is_err(),
            "accepted {defect}"
        );
    }
}

#[test]
fn resolution_retry_retains_original_input_and_exact_outputs() {
    let resolved = validated().resolved(&topology(), &system()).unwrap();
    let record = serde_json::to_value(resolved.record()).unwrap();
    let original_topology = resolved.topology_bytes().to_vec();
    let original_system = resolved.system_bytes().to_vec();
    for defect in ["input", "topology", "system"] {
        let mut wire = record.clone();
        let mut topology = original_topology.clone();
        let mut system = original_system.clone();
        match defect {
            "input" => wire["input"]["source"]["commit"] = json!("b".repeat(40)),
            "topology" => topology.push(b' '),
            "system" => system.push(b' '),
            _ => unreachable!(),
        }
        assert!(
            ResolvedConfiguration::restore(
                validated(),
                serde_json::from_value(wire).unwrap(),
                topology,
                system
            )
            .is_err(),
            "accepted {defect}"
        );
    }
    let restored = ResolvedConfiguration::restore(
        validated(),
        serde_json::from_value(record).unwrap(),
        original_topology.clone(),
        original_system.clone(),
    )
    .unwrap();
    let inputs = restored.inputs().unwrap();
    assert_eq!(inputs.commit(), "a".repeat(40));
    assert_eq!(std::fs::read(inputs.topology()).unwrap(), original_topology);
    assert_eq!(std::fs::read(inputs.system()).unwrap(), original_system);
}

#[test]
fn resolution_cannot_add_vm_policy_or_host_actions() {
    let mut extra_vm = topology();
    extra_vm["domains"]["unrequested"] = json!({"class":"StandaloneVM"});
    assert!(validated().resolved(&extra_vm, &system()).is_err());
    let mut policy = topology();
    policy["qrexec_policy"]["rules"] = json!([{"action":"allow"}]);
    assert!(validated().resolved(&policy, &system()).is_err());
    let mut host = system();
    host["plan"]["live_actions"] = json!([{"type":"unrequested-action"}]);
    assert!(validated().resolved(&topology(), &host).is_err());
}
