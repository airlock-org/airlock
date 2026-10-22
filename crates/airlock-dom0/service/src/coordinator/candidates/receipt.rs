//! Candidate admission keeps typed transport claims distinct from freshly inspected storage records.
//! Validation here is ordinary Rust; deployment approval and effect execution retain their Verus gates.

use crate::model::{
    AdmissionSender, CheckStatus, GenerationApproval, GenerationInfo, GenerationProvenance,
};
use crate::store::AdmissionReceipt;
use airlock_nix_core::artifact::RootDescriptor;
use anyhow::{Context, Result, ensure};

/// A receipt matched against its retained, verified generation and recorded admission provenance.
pub(super) struct CheckedAdmission {
    // Original typed transport claims, available only through immutable access.
    receipt: AdmissionReceipt,
    // Fresh inspection; never converted back to generic JSON for decisions.
    info: GenerationInfo,
    // Strict modern admission metadata; absent legacy metadata cannot stage a candidate.
    approval: GenerationApproval,
}

impl CheckedAdmission {
    /// Refuse inactive/retention claims or identities that disagree with the actual committed generation.
    pub(super) fn check(receipt: AdmissionReceipt, info: GenerationInfo) -> Result<Self> {
        ensure!(
            receipt.status == CheckStatus::Passed && !receipt.activated && receipt.retained,
            "receipt is not a retained inactive qrexec admission"
        );
        ensure!(
            info.status == CheckStatus::Passed
                && info.domain.as_str() == crate::store::runtime_namespace(receipt.domain.as_str())
                && info.generation == receipt.generation,
            "generation identity differs"
        );
        ensure!(
            info.verification.status == CheckStatus::Passed
                && info.retention.as_ref().is_some_and(|r| r.retain),
            "admission generation is not verified and retained"
        );
        ensure!(
            info.manifest == receipt.manifest
                && info.system == receipt.system
                && receipt.root_source.to_str() == Some(info.root_source.as_str())
                && info.kernel_name == receipt.kernel_name,
            "admission receipt identity mismatch"
        );
        ensure!(
            info.store.closure_hash == receipt.closure_hash,
            "admission receipt closure mismatch"
        );
        let root = receipt
            .root_artifact
            .as_ref()
            .context("missing admission root artifact")?;
        root.validate()?;
        let actual: RootDescriptor = serde_json::from_value(
            info.root_artifact
                .clone()
                .context("missing generation root artifact")?,
        )?;
        actual.validate()?;
        ensure!(
            root == &actual,
            "admission receipt identity mismatch: root artifact"
        );

        let provenance: GenerationProvenance = serde_json::from_value(info.provenance.clone())
            .context("invalid generation provenance")?;
        ensure!(
            provenance.source_revision == receipt.source_revision
                && provenance.lock_hash == receipt.lock_hash,
            "receipt provenance differs from pool generation"
        );
        let approval: GenerationApproval =
            serde_json::from_value(info.approval.clone()).context("invalid generation approval")?;
        ensure!(
            approval.source == info.closure_source,
            "generation approval differs from admitted closure"
        );
        Ok(Self {
            receipt,
            info,
            approval,
        })
    }

    /// Original claims after comparison with the independently inspected generation.
    pub(super) fn receipt(&self) -> &AdmissionReceipt {
        &self.receipt
    }

    /// Publish transport evidence at the candidate's serialization boundary after validation.
    pub(super) fn into_receipt(self) -> AdmissionReceipt {
        self.receipt
    }

    /// Complete inspection without erasing its typed identity and selection fields.
    pub(super) fn info(&self) -> &GenerationInfo {
        &self.info
    }

    /// Whole-machine binding requires the submitting controller, not a different recognized sender.
    pub(super) fn controller(&self) -> bool {
        matches!(self.approval._approved_by, AdmissionSender::Controller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// Independent committed inspection and admission message for one retained, inactive generation.
    fn records() -> (Value, Value) {
        let system = format!("/nix/store/{hash}-system", hash = "a".repeat(32));
        let root = format!("/nix/store/{hash}-root/root.img", hash = "b".repeat(32));
        let source = format!("/nix/store/{hash}-closure", hash = "c".repeat(32));
        let artifact = json!({"schema":"airlock.root-artifact.v1", "sha256":"d".repeat(64),
            "logical_bytes":65536, "payload_bytes":64, "format":"ext4", "boot_abi":"airlock.nix-closure-root.v1"});
        let info = json!({"schema":"airlock.closure-store.generation-info.v1", "status":"passed",
            "domain":"app-runtime-store", "generation":2, "selected":false, "manifest":"/pool/app/2/manifest.json",
            "system":system, "root_source":root, "root_artifact":artifact, "kernel_name":"kernel", "closure_source":source,
            "store":{"mode":"shared-closure", "closure_hash":"e".repeat(64), "mountpoint":"/nix/store", "device":"/dev/xvde", "blob_device":"/dev/xvdf", "view_backend":"erofs-objectmap"},
            "verification":{"status":"passed"}, "retention":{"retain":true},
            "approval":{"transport":"qrexec", "approved_by":"sys-ctrl", "source":source},
            "provenance":{"source_revision":"f".repeat(40), "lock_hash":"a".repeat(64)}});
        let receipt = json!({"schema":"airlock.closure-store.admission.v1", "status":"passed", "transport":"qrexec",
            "domain":"app", "generation":2, "current_generation":1, "activated":false, "retained":true,
            "manifest":"/pool/app/2/manifest.json", "system":system, "root_source":root, "root_artifact":artifact,
            "kernel_name":"kernel", "closure_hash":"e".repeat(64), "source_revision":"f".repeat(40), "lock_hash":"a".repeat(64)});
        (receipt, info)
    }

    /// Nominal receipt shape and independently loaded authority must both validate.
    fn checked(receipt: Value, info: Value) -> Result<CheckedAdmission> {
        CheckedAdmission::check(
            serde_json::from_value(receipt)?,
            serde_json::from_value(info)?,
        )
    }

    #[test]
    fn candidate_identity_is_checked_without_erasing_inspection_types() -> Result<()> {
        let (receipt, info) = records();
        let admission = checked(receipt.clone(), info.clone())?;
        assert!(admission.controller());
        // Candidate files compare retained receipts on retry; typing must preserve the wire record.
        assert_eq!(serde_json::to_value(admission.receipt())?, receipt);
        for reused in [false, true] {
            let mut original = receipt.clone();
            original["generation_reused"] = json!(reused);
            let typed: AdmissionReceipt = serde_json::from_value(original.clone())?;
            assert_eq!(serde_json::to_value(typed)?, original);
        }
        assert_eq!(admission.info().generation.get(), 2);
        assert!(!admission.info().selected);

        for (field, value) in [
            ("domain", json!("other")),
            ("generation", json!(3)),
            ("manifest", json!("/other/manifest.json")),
            ("root_source", json!("/tmp/manual-root.img")),
            ("kernel_name", json!("other")),
            ("closure_hash", json!("b".repeat(64))),
            ("source_revision", json!("a".repeat(40))),
            ("lock_hash", json!("b".repeat(64))),
            ("activated", json!(true)),
            ("retained", json!(false)),
            ("root_artifact", Value::Null),
        ] {
            let mut invalid = receipt.clone();
            invalid[field] = value;
            assert!(checked(invalid, info.clone()).is_err(), "accepted {field}");
        }
        let mut invalid = info.clone();
        invalid["verification"]["status"] = json!("failed");
        assert!(checked(receipt.clone(), invalid).is_err());
        let mut invalid = info.clone();
        invalid["retention"]["retain"] = json!(false);
        assert!(checked(receipt.clone(), invalid).is_err());
        let mut wrong_root = receipt.clone();
        wrong_root["root_artifact"]["sha256"] = json!("c".repeat(64));
        assert!(checked(wrong_root, info.clone()).is_err());

        let mut other_sender = info.clone();
        other_sender["approval"]["approved_by"] = json!("usr-assistant");
        assert!(!checked(receipt, other_sender)?.controller());
        Ok(())
    }

    #[test]
    fn missing_selection_or_unknown_protocol_cannot_mean_inactive_admission() {
        let (receipt, info) = records();
        for field in ["activated", "retained", "current_generation"] {
            let mut missing = receipt.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                checked(missing, info.clone()).is_err(),
                "accepted missing {field}"
            );
        }
        let mut missing = info.clone();
        missing.as_object_mut().unwrap().remove("selected");
        assert!(checked(receipt.clone(), missing).is_err());
        for (field, value) in [
            ("schema", json!("unknown")),
            ("transport", json!("local")),
            ("retained", json!("true")),
        ] {
            let mut invalid = receipt.clone();
            invalid[field] = value;
            assert!(checked(invalid, info.clone()).is_err());
        }
        let mut empty_selection = receipt;
        empty_selection["current_generation"] = Value::Null;
        assert!(checked(empty_selection, info).is_ok());
    }
}
