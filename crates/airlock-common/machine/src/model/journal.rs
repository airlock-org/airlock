//! A deployment journal records intended effects and the last attempted phase.
//! Native entries bind both release snapshots so recovery can reject changed inputs.

use super::{ApprovalFingerprint, BootId, Phase, ReleaseFingerprint, ReleasePath, TransactionId};
use airlock_nix_core::identity::{GuestName, ReviewId};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Supported native journal format controlling recovery compatibility.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NativeVersion {
    #[serde(rename = "airlock-dom0-service-v1")]
    /// Version one of this enclosing wire record.
    V1,
    #[serde(rename = "airlock-dom0-service-v2")]
    /// Current intent is authoritative and carries a durable operation binding.
    V2,
}

/// Complete native version and candidate/baseline fingerprints required for recovery.
#[derive(Clone, Debug, Serialize)]
pub struct NativeJournal {
    /// Native journal version identifying supported recovery semantics.
    pub native_coordinator: NativeVersion,
    /// Content digest binding the candidate release to this transaction.
    pub release_fingerprint: ReleaseFingerprint,
    /// Content digest binding the rollback baseline to this transaction.
    pub previous_fingerprint: ReleaseFingerprint,
}

/// Journal origin distinguishing fingerprint-bound native recovery from legacy inspection.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum JournalOrigin {
    /// Retain the metadata recorded by the native coordinator.
    Native(
        /// Complete native journal metadata required for native recovery.
        NativeJournal,
    ),
    /// Accept a historical record without native coordinator metadata.
    Legacy {},
}

impl Default for JournalOrigin {
    fn default() -> Self {
        Self::Legacy {}
    }
}

impl JournalOrigin {
    /// Return complete native fingerprints or reject legacy recovery for this coordinator.
    pub fn require_native(&self) -> Result<&NativeJournal> {
        match self {
            Self::Native(native) => Ok(native),
            Self::Legacy {} => {
                bail!("recovery requires a fingerprint-bound native transaction")
            }
        }
    }
}

/// GUI provider and guest boot identity captured before a display-provider restart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuiSnapshot {
    /// GUI domain displaying the preserved guest windows.
    pub provider: GuestName,
    /// Guest boot identity used to confirm it survived GUI reconnection.
    pub boot_id: BootId,
}

/// Authority checked before publishing an operation's intent; recovery rechecks it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationAuthority {
    /// A protected human decision bounds the candidate and all recovery effects.
    Reviewed {
        /// Protected request supplying the decision and maximum effect scope.
        request_id: ReviewId,
        /// Digest of the immutable decision, source, scope and reviewed baseline.
        decision: ApprovalFingerprint,
        /// Original candidate, retained even while rolling back to its baseline.
        release: ReleasePath,
        /// Original candidate contents; a path alone is insufficient identity.
        fingerprint: ReleaseFingerprint,
    },
    /// The installed machine policy did not require human source review.
    ReviewNotRequired,
    /// Explicit dom0 administrative repair, available only in development mode.
    Administrative,
}

/// Stable identity and checked authority shared by deployment recovery attempts.
///
/// This is evidence in a trusted journal, not a transferable capability. A reader
/// must validate the journal, current policy and protected decision before effects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentOperation {
    /// Identity fixed at first publication or legacy adoption, preserved across recovery.
    pub id: TransactionId,
    /// Authority must remain identical throughout this operation's recovery chain.
    pub authority: OperationAuthority,
}

/// Optional recovery links and diagnostics retained alongside an activation journal.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JournalDetails {
    /// Durable authorization-to-execution handoff; absent only in older journals.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub operation: Option<DeploymentOperation>,
    /// GUI-provider and guest boot identities captured before disruption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gui_preserved: Option<BTreeMap<GuestName, GuiSnapshot>>,
    /// Reconnection result retained for recovery diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gui_reconnected: Option<Value>,
    /// Original transaction this recovery attempt continues or rolls back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_of: Option<TransactionId>,
    /// Later recovery transaction that supersedes this journal entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovered_by: Option<TransactionId>,
    /// Last activation phase attempted before failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_phase: Option<Phase>,
    /// Human-readable failure reason saved for recovery inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional protocol metadata retained without treating it as authority.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Journal origin and recovery details, preserving legacy inspection compatibility.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "JournalWire")]
pub struct JournalMetadata {
    /// Native or legacy journal origin controlling supported recovery.
    #[serde(flatten)]
    pub origin: JournalOrigin,
    /// Recovery links, verification evidence and diagnostic metadata.
    #[serde(flatten)]
    pub details: JournalDetails,
}

// Missing native fields describe a legacy record. Present-but-null and partial
// groups are errors, rather than a way to fall back to legacy verification.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Deserialize)]
struct JournalWire {
    /// Native journal version identifying supported recovery semantics.
    #[serde(default, deserialize_with = "present")]
    native_coordinator: Option<NativeVersion>,
    /// Content digest binding the candidate release to this transaction.
    #[serde(default, deserialize_with = "present")]
    release_fingerprint: Option<ReleaseFingerprint>,
    /// Content digest binding the rollback baseline to this transaction.
    #[serde(default, deserialize_with = "present")]
    previous_fingerprint: Option<ReleaseFingerprint>,
    /// Recovery links, verification evidence and diagnostic metadata.
    #[serde(flatten)]
    details: JournalDetails,
}

impl TryFrom<JournalWire> for JournalMetadata {
    type Error = anyhow::Error;

    fn try_from(w: JournalWire) -> Result<Self> {
        let origin = match (
            w.native_coordinator,
            w.release_fingerprint,
            w.previous_fingerprint,
        ) {
            (None, None, None) => JournalOrigin::Legacy {},
            (Some(native_coordinator), Some(release_fingerprint), Some(previous_fingerprint)) => {
                JournalOrigin::Native(NativeJournal {
                    native_coordinator,
                    release_fingerprint,
                    previous_fingerprint,
                })
            }
            _ => bail!("incomplete native transaction identity"),
        };
        let durable = matches!(&origin, JournalOrigin::Native(native) if matches!(native.native_coordinator, NativeVersion::V2));
        ensure!(
            durable == w.details.operation.is_some(),
            "journal version and durable operation binding disagree"
        );

        Ok(Self {
            origin,
            details: w.details,
        })
    }
}
