//! Every process consumes the same strict receipt shape; parsing claims never authenticates storage.

use airlock_nix_core::{
    admission::{AdmissionReceipt, ExpectedAdmission},
    identity::SourceIdentity,
};
use serde_json::{Value, json};

/// Actual admission protocol fields, including explicit empty selection and complete root identity.
fn receipt() -> Value {
    json!({
        "schema":"airlock.closure-store.admission.v1", "transport":"qrexec", "status":"passed",
        "domain":"app", "generation":2, "current_generation":null, "activated":false, "retained":true,
        "manifest":"/pool/app/2/manifest.json", "system":format!("/nix/store/{hash}-system",hash="a".repeat(32)),
        "root_source":format!("/nix/store/{hash}-root/root.img",hash="b".repeat(32)), "kernel_name":"kernel",
        "root_artifact":{"schema":"airlock.root-artifact.v1", "sha256":"d".repeat(64), "logical_bytes":65536,
            "payload_bytes":64, "format":"ext4", "boot_abi":"airlock.nix-closure-root.v1"},
        "closure_hash":"e".repeat(64), "source_revision":"f".repeat(40), "lock_hash":"a".repeat(64),
        "objects_transferred":3,
    })
}

/// Exact expected artifacts from the same built release; later source identity can change independently.
fn expected() -> ExpectedAdmission {
    let receipt = receipt();
    serde_json::from_value(json!({
        "domain":receipt["domain"], "system":receipt["system"], "root_source":receipt["root_source"],
        "closure_source":format!("/nix/store/{hash}-prepared/guest/closure",hash="c".repeat(32)),
        "kernel_name":receipt["kernel_name"], "root_artifact":receipt["root_artifact"],
        "source_revision":receipt["source_revision"], "lock_hash":receipt["lock_hash"],
    })).unwrap()
}

#[test]
fn common_wire_contract_preserves_reuse_presence_and_diagnostics() {
    for reused in [None, Some(false), Some(true)] {
        let mut original = receipt();
        if let Some(value) = reused {
            original["generation_reused"] = json!(value);
        }
        let typed: AdmissionReceipt = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(typed.generation.get(), 2);
        assert_eq!(typed.diagnostics["objects_transferred"], 3);
        assert_eq!(serde_json::to_value(&typed).unwrap(), original);
        typed.check_expected(&expected()).unwrap();
    }
}

#[test]
fn missing_contradictory_and_unknown_fields_cannot_mean_success() {
    for field in [
        "status",
        "activated",
        "retained",
        "current_generation",
        "manifest",
        "system",
    ] {
        let mut value = receipt();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<AdmissionReceipt>(value).is_err(),
            "accepted missing {field}"
        );
    }
    for (field, value) in [
        ("schema", json!("unknown")),
        ("transport", json!("local")),
        ("status", json!("unknown")),
        ("generation", json!(0)),
        ("retained", json!("true")),
        ("generation_reused", Value::Null),
        ("closure_hash", json!("not-a-digest")),
        ("domain", json!("dom0")),
    ] {
        let mut invalid = receipt();
        invalid[field] = value;
        assert!(
            serde_json::from_value::<AdmissionReceipt>(invalid).is_err(),
            "accepted {field}"
        );
    }
    let duplicate = serde_json::to_string(&receipt()).unwrap().replacen(
        "\"generation\":2",
        "\"generation\":2,\"generation\":3",
        1,
    );
    assert!(serde_json::from_str::<AdmissionReceipt>(&duplicate).is_err());
}

#[test]
fn old_generation_provenance_is_preserved_only_for_matching_reused_artifacts() {
    let mut desired = expected();
    desired.source_revision = "b".repeat(40).parse().unwrap();
    for reused in [None, Some(false), Some(true)] {
        let mut value = receipt();
        if let Some(value_reused) = reused {
            value["generation_reused"] = json!(value_reused);
        }
        let typed: AdmissionReceipt = serde_json::from_value(value).unwrap();
        assert_eq!(typed.check_expected(&desired).is_ok(), reused == Some(true));
        assert_eq!(typed.source_revision.as_str(), "f".repeat(40));
    }
    let mut value = receipt();
    value["generation_reused"] = json!(true);
    let typed: AdmissionReceipt = serde_json::from_value(value).unwrap();
    desired.root_artifact.sha256 = "c".repeat(64);
    assert!(typed.check_expected(&desired).is_err());
}

#[test]
fn manual_rootless_transfers_remain_distinct_from_candidate_admission_evidence() {
    let mut value = receipt();
    value["root_source"] = json!("");
    value["root_artifact"] = Value::Null;
    let typed: AdmissionReceipt = serde_json::from_value(value).unwrap();
    typed
        .check_transfer(
            &"app".parse().unwrap(),
            &SourceIdentity {
                revision: "f".repeat(40).parse().unwrap(),
                lock_hash: "a".repeat(64).parse().unwrap(),
            },
        )
        .unwrap();
    assert!(typed.evidence().is_err());
    assert!(typed.check_expected(&expected()).is_err());
}
