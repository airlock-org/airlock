//! Typed root lookup response shared by the storage producer and native setup receiver.

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
