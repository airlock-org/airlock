//! Distinct typed storage responses shared by producers and setup/candidate receivers.

use airlock_nix_core::{artifact::RootDescriptor, identity::StoreNamespace};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A resolved retained root belongs to a storage namespace and its original source.
/// Filesystem authentication stays with the loader, and setup checks the requested binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolvedRoot {
    /// Namespace searched by the storage service.
    pub domain: StoreNamespace,
    /// Original boot-root identity used in the lookup.
    pub root_source: String,
    /// Retained physical root image to import.
    pub image: PathBuf,
    /// Sparse artifact descriptor authenticated by the storage service.
    pub artifact: RootDescriptor,
}

/// Qrexec admission response addressed to a VM, distinct from a namespace inspection or installation.
/// A decoded response is not authority; candidate preparation checks the committed generation separately.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AdmissionReceipt {
    /// Closed artifact-admission schema.
    pub schema: crate::model::GenerationReceiptSchema,
    /// Explicit transport outcome, independent of process exit status.
    pub status: crate::model::CheckStatus,
    /// Existing authenticated Qubes transport.
    pub transport: crate::model::AdmissionTransport,
    /// Guest named by the authenticated admission request.
    pub domain: airlock_nix_core::identity::GuestName,
    /// Admitted generation within that guest's runtime namespace.
    pub generation: crate::model::GenerationId,
    /// Observed selection at admission; an explicit null means no selected generation.
    #[serde(deserialize_with = "required_selection")]
    pub current_generation: Option<crate::model::GenerationId>,
    /// Whether admission also activated the generation; candidates require false.
    pub activated: bool,
    /// Whether the admitted generation is retained; candidates require true.
    pub retained: bool,
    /// Committed manifest compared with the independently loaded pool record.
    pub manifest: PathBuf,
    /// Exact admitted guest system.
    pub system: crate::model::StorePath,
    /// Original root source; manual admission permits a local path or an empty path without a root.
    /// Candidate binding separately requires equality with the inspected immutable Nix artifact.
    pub root_source: PathBuf,
    /// Sparse root identity; old storage-only admission may legitimately omit the artifact.
    pub root_artifact: Option<RootDescriptor>,
    /// Boot kernel paired with the admitted system.
    pub kernel_name: String,
    /// Content identity of the admitted closure.
    pub closure_hash: String,
    /// Source revision recorded by admission.
    pub source_revision: airlock_nix_core::identity::GitRevision,
    /// Lockfile identity recorded by admission.
    pub lock_hash: airlock_nix_core::identity::LockHash,
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

/// Require the selection field even when its value is explicitly null.
fn required_selection<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<crate::model::GenerationId>, D::Error> {
    Option::deserialize(decoder)
}

/// Distinguish an omitted reuse claim from an explicit boolean; null is malformed evidence.
fn reuse_claim<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<bool>, D::Error> {
    bool::deserialize(decoder).map(Some)
}
