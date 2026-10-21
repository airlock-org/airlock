//! Validate boot configuration once while retaining the original declaration.
//! Raw JSON preserves omitted settings; the typed projection supplies identities
//! and dependency semantics. Neither representation grants deployment approval.

use crate::{model::Topology, planning};
use anyhow::{Context, Result};
use serde_json::Value;

/// One immutable boot input accepted by the runtime topology and graph decoders.
/// Keep this value across planning so later code cannot replace the declaration
/// while retaining an earlier successful validation.
#[derive(Clone)]
pub struct BootInputs {
    // Original settings, including omitted, null and explicit overrides.
    raw: Value,
    // Runtime identities decoded from these exact settings.
    topology: Topology,
}

impl BootInputs {
    /// Decode identities and reject invalid dependency graphs before setup effects.
    pub fn new(raw: Value) -> Result<Self> {
        let topology: Topology =
            serde_json::from_value(raw.clone()).context("decode boot topology")?;
        topology.validate().context("validate boot topology")?;
        planning::ordered(&topology).context("validate boot dependency graph")?;
        Ok(Self { raw, topology })
    }

    /// Borrow the unchanged declaration for adapters which must preserve defaults.
    pub fn raw(&self) -> &Value {
        &self.raw
    }

    /// Borrow the typed projection used by dependency and boot planning.
    pub fn topology(&self) -> &Topology {
        &self.topology
    }
}
