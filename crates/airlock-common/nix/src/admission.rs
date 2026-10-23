//! Shared admission claims exchanged by build workers, sys-ctrl and dom0.
//! Decoding establishes shape and identity, not sender authority or committed storage truth.

use crate::{
    Error, Result,
    identity::{ClosureHash, GenerationId, GitRevision, GuestName, LockHash, SourceIdentity},
    root_artifact::RootDescriptor,
    store_path::{StoreArtifact, StoreFile, StorePath},
};
use airlock_machine_planning::admission as verified;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Verified decisions and observations used by dom0's independent storage adapter.
pub use airlock_machine_planning::admission::{Authenticated, Bound, BoundSet, Inspection, Sender};

/// Success or failure reported by a storage helper, independently of deployment status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The named check satisfied its verification condition.
    Passed,
    /// The operation or check failed; its enclosing record carries the details.
    Failed,
}

/// Supported protocol for admitting guest artifacts into pool-backed storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuestTransport {
    #[serde(rename = "direct-pool-v1")]
    /// Admit software directly into the existing Qubes pool using protocol version one.
    DirectPoolV1,
}

/// Transport marker required before treating new storage admission as qrexec-bound.
#[derive(Debug, Serialize, Deserialize)]
pub enum AdmissionTransport {
    #[serde(rename = "qrexec")]
    /// Use the existing Qubes RPC transport for admission.
    Qrexec,
}

/// Recognized submitting VM named in storage admission provenance.
#[derive(Debug, Deserialize)]
pub enum AdmissionSender {
    #[serde(rename = "usr-assistant")]
    /// Accept artifacts from the configured interactive development VM.
    Assistant,
    #[serde(rename = "sys-ctrl")]
    /// Accept artifacts from the configured build controller.
    Controller,
}

/// The optional legacy admission marker is never an installation marker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GenerationReceiptSchema {
    /// Original native admission response; loaded generation projections have no marker.
    #[serde(rename = "airlock.closure-store.admission.v1")]
    Admission,
}

/// Qrexec admission response addressed to a VM, distinct from a namespace inspection or installation.
/// A decoded response is not authority; candidate preparation checks the committed generation separately.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdmissionReceipt {
    /// Closed artifact-admission schema.
    pub schema: GenerationReceiptSchema,
    /// Explicit transport outcome, independent of process exit status.
    pub status: CheckStatus,
    /// Existing authenticated Qubes transport.
    pub transport: AdmissionTransport,
    /// Guest named by the authenticated admission request.
    pub domain: crate::identity::GuestName,
    /// Admitted generation within that guest's runtime namespace.
    pub generation: GenerationId,
    /// Observed selection at admission; an explicit null means no selected generation.
    #[serde(deserialize_with = "required_selection")]
    pub current_generation: Option<GenerationId>,
    /// Whether admission also activated the generation; candidates require false.
    pub activated: bool,
    /// Whether the admitted generation is retained; candidates require true.
    pub retained: bool,
    /// Committed manifest compared with the independently loaded pool record.
    pub manifest: PathBuf,
    /// Exact admitted guest system.
    pub system: StorePath,
    /// Original root source; manual admission permits a local path or an empty path without a root.
    /// Candidate binding separately requires equality with the inspected immutable Nix artifact.
    pub root_source: PathBuf,
    /// Sparse root identity; old storage-only admission may legitimately omit the artifact.
    pub root_artifact: Option<RootDescriptor>,
    /// Boot kernel paired with the admitted system.
    pub kernel_name: String,
    /// Content identity of the admitted closure.
    pub closure_hash: ClosureHash,
    /// Source revision recorded by admission.
    pub source_revision: crate::identity::GitRevision,
    /// Lockfile identity recorded by admission.
    pub lock_hash: crate::identity::LockHash,
    /// Explicit reuse claim; omission keeps the stricter new-source rule and its original wire shape.
    #[serde(
        default,
        deserialize_with = "reuse_claim",
        skip_serializing_if = "Option::is_none"
    )]
    pub generation_reused: Option<bool>,
    /// Transfer counters and physical diagnostics, never interpreted as candidate authority.
    #[serde(flatten)]
    pub diagnostics: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Exact built artifacts expected for one guest in the protected candidate admission intent.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedAdmission {
    /// Exact consuming VM.
    pub domain: GuestName,
    /// Immutable system object built for this VM.
    pub system: StorePath,
    /// Closure descriptor, which may be a child of a prepared store output.
    pub closure_source: StoreArtifact,
    /// Immutable root-image file built for this VM.
    pub root_source: StoreFile,
    /// Paired boot kernel.
    pub kernel_name: String,
    /// Current candidate source revision; protected source approval is checked separately.
    pub source_revision: GitRevision,
    /// Current candidate dependency lock.
    pub lock_hash: LockHash,
    /// Complete root content and boot identity from the built release catalog.
    pub root_artifact: RootDescriptor,
}

/// Validate the root contract before converting its complete identity into proof-compatible primitives.
pub fn root_identity(root: &RootDescriptor) -> Result<verified::Root> {
    root.validate()?;
    Ok(verified::Root {
        digest: root.sha256.clone(),
        logical_bytes: root.logical_bytes(),
        payload_bytes: root.payload_bytes(),
        format: root.format().to_owned(),
        boot_abi: root.boot_abi().to_owned(),
    })
}

/// Preserve exact source identities when crossing the executable Verus boundary.
fn source(revision: &GitRevision, lock_hash: &LockHash) -> verified::Source {
    verified::Source {
        revision: revision.as_str().to_owned(),
        lock_hash: lock_hash.as_str().to_owned(),
    }
}

/// Report malformed or mismatched admission evidence before it can become candidate authority.
fn require(valid: bool, expected: &'static str) -> Result<()> {
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidField {
            field: "admission",
            expected,
        })
    }
}

impl ExpectedAdmission {
    /// Project validated identities for the actual verifier used by controller and coordinator.
    pub fn evidence(&self, candidate: &str) -> Result<verified::Expected> {
        Ok(verified::Expected {
            guest: self.domain.as_str().to_owned(),
            closure_source: self.closure_source.as_str().to_owned(),
            artifacts: verified::Artifacts {
                system: self.system.as_str().to_owned(),
                root_source: self.root_source.as_str().to_owned(),
                kernel: self.kernel_name.clone(),
                root: root_identity(&self.root_artifact)?,
            },
            source: source(&self.source_revision, &self.lock_hash),
            candidate: candidate.to_owned(),
        })
    }
}

impl AdmissionReceipt {
    /// Decode all candidate-relevant claims without treating diagnostic extensions as authority.
    pub fn evidence(&self) -> Result<verified::Claim> {
        let root = self.root_artifact.as_ref().ok_or(Error::InvalidField {
            field: "admission",
            expected: "missing admission root artifact",
        })?;
        Ok(verified::Claim {
            guest: self.domain.as_str().to_owned(),
            generation: self.generation.get(),
            manifest: self
                .manifest
                .to_str()
                .ok_or(Error::InvalidField {
                    field: "manifest",
                    expected: "UTF-8 path",
                })?
                .to_owned(),
            artifacts: verified::Artifacts {
                system: self.system.as_str().to_owned(),
                root_source: self
                    .root_source
                    .to_str()
                    .ok_or(Error::InvalidField {
                        field: "root source",
                        expected: "UTF-8 path",
                    })?
                    .to_owned(),
                kernel: self.kernel_name.clone(),
                root: root_identity(root)?,
            },
            closure_hash: self.closure_hash.as_str().to_owned(),
            source: source(&self.source_revision, &self.lock_hash),
            passed: self.status == CheckStatus::Passed,
            activated: self.activated,
            retained: self.retained,
            reused: self.generation_reused,
        })
    }

    /// Check the worker's exact source/target and retained inactive outcome, including legacy rootless transfers.
    /// This is transport validation; dom0 still independently authenticates any candidate admission.
    pub fn check_transfer(&self, domain: &GuestName, expected: &SourceIdentity) -> Result<()> {
        if let Some(root) = &self.root_artifact {
            root.validate()?;
        }
        require(
            self.status == CheckStatus::Passed
                && self.retained
                && !self.activated
                && &self.domain == domain
                && self.source_revision == expected.revision
                && self.lock_hash == expected.lock_hash,
            "admission differs from submitted source or target",
        )
    }

    /// Check candidate artifacts using production Verus logic; this remains a transport-claim check.
    pub fn check_expected(&self, expected: &ExpectedAdmission) -> Result<()> {
        require(
            verified::check_claim(&self.evidence()?, &expected.evidence("transport-claim")?),
            "guest admission artifacts or fresh provenance differ from candidate",
        )
    }
}

/// Require the selection field even when its value is explicitly null.
fn required_selection<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<GenerationId>, D::Error> {
    Option::deserialize(decoder)
}

/// Distinguish an omitted reuse claim from an explicit boolean; null is malformed evidence.
fn reuse_claim<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<bool>, D::Error> {
    bool::deserialize(decoder).map(Some)
}
