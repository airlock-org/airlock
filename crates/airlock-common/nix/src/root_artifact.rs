//! Sparse boot-image identities shared by build workers, controllers and dom0.
//! This metadata performs no transfer or filesystem operation; bytes are authenticated by storage adapters.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Fixed sparse stream header, excluding its encoded logical size.
pub(crate) const MAGIC: &[u8] = b"AIRLOCK-ROOT-SPARSE-1\n";

/// Size of one sparse stream data block.
pub(crate) const BLOCK: u64 = 65536;

/// Keep validation failures compatible with the transport's existing diagnostics.
fn require(valid: bool, reason: &'static str) -> Result<()> {
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidRootArtifact { reason })
    }
}

/// Identity of the canonical sparse representation, rather than filesystem allocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootDescriptor {
    /// Wire format recognized by image preparation and admission.
    pub(crate) schema: String,
    /// SHA-256 of the complete sparse stream, including its header.
    pub sha256: String,
    /// Length of the fully reconstructed boot disk.
    pub(crate) logical_bytes: u64,
    /// Length of the canonical sparse stream.
    pub(crate) payload_bytes: u64,
    /// Filesystem format supported by the guest boot machinery.
    pub(crate) format: String,
    /// Guest boot contract required to mount the retained disk.
    pub(crate) boot_abi: String,
}

impl RootDescriptor {
    /// Reject unknown formats, malformed digests and impossible stream sizes.
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == "airlock.root-artifact.v1"
                && self.format == "ext4"
                && self.boot_abi == "airlock.nix-closure-root.v1",
            "unsupported root artifact format",
        )?;
        require(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid root digest",
        )?;
        require(
            self.logical_bytes > 0 && self.logical_bytes <= 64 * 1024u64.pow(3),
            "root size out of bounds",
        )?;
        require(
            (MAGIC.len() as u64 + 8
                ..=self.logical_bytes
                    + 12 * self.logical_bytes.div_ceil(BLOCK)
                    + MAGIC.len() as u64
                    + 8)
                .contains(&self.payload_bytes),
            "root payload size out of bounds",
        )?;
        Ok(())
    }
}

impl RootDescriptor {
    /// Reconstructed logical disk length, independent of sparse allocation.
    pub fn logical_bytes(&self) -> u64 {
        self.logical_bytes
    }

    /// Complete encoded stream length.
    pub fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }

    /// Filesystem format declared for this image; use [`Self::validate`] to check support.
    pub fn format(&self) -> &str {
        &self.format
    }

    /// Boot contract declared for this image; use [`Self::validate`] to check support.
    pub fn boot_abi(&self) -> &str {
        &self.boot_abi
    }
}
