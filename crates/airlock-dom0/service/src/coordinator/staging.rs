//! Resolve generation obligations before VM disruption and retain the checked result in memory.
//! The numeric map is a compatibility projection; native setup never reconstructs authority from it.

use super::*;
use crate::store::{GenerationMatchOutcome, GenerationMatchSchema};
use airlock_deployment_core::{
    boot::SelectedGeneration,
    storage::staging::{Requirement, Resolution, StagedGenerations},
};
use airlock_nix_core::identity::StoreNamespace;
use serde::Deserialize;

/// Immutable prepared descriptors suggest a generation; committed storage independently authenticates it.
#[derive(Deserialize)]
struct PreparedHint {
    /// Known prepared descriptor protocol, not an admission/installation result.
    schema: PreparedSchema,
    /// Guest runtime namespace in which the positive generation must exist.
    domain: StoreNamespace,
    /// Candidate generation; no authority is gained by decoding it.
    generation: GenerationId,
}

/// Closed prepared protocol used only to discover the already committed generation.
#[derive(Deserialize)]
enum PreparedSchema {
    /// Descriptor generated during image preparation.
    #[serde(rename = "airlock.closure-store.prepared.v1")]
    V1,
}

impl Coordinator {
    /// Resolve the complete original storage namespace under an executing durable operation.
    pub(super) fn stage_images(
        &self,
        release: &Release,
        record: &Record,
        directory: &Path,
        durable: &super::approval::DurableAuthorization,
    ) -> Result<StagedGenerations> {
        let mut entries = Vec::new();
        for (index, definition) in durable.storage_definitions().iter().enumerate() {
            let entry = match durable.staging_requirement(index)? {
                Requirement::NonNative => Resolution::NonNative,
                Requirement::Existing => Resolution::Existing,
                Requirement::Resolve(pin) => {
                    let name: GuestName = definition.name.parse()?;
                    let native = definition
                        .native
                        .as_ref()
                        .context("missing original native backend")?;
                    let backend = &release
                        .topology
                        .domains
                        .get(&name)
                        .context("staged VM is undeclared")?
                        .backend;
                    let generation = if let Some(pin) = pin {
                        pin.to_string().parse()?
                    } else {
                        let prepared = Path::new(&native.artifact).join("prepared.json");
                        if prepared.try_exists()? {
                            // Prepared descriptors are immutable configuration and may use store symlinks.
                            let hint: PreparedHint = state::configuration(&prepared)?;
                            let PreparedSchema::V1 = hint.schema;
                            ensure!(
                                hint.domain.as_str()
                                    == crate::store::runtime_namespace(name.as_str()),
                                "prepared staging namespace differs"
                            );
                            hint.generation
                        } else {
                            let reply = self.store().find(&name, backend, "usr-assistant")?;
                            let GenerationMatchSchema::V1 = reply.schema;
                            ensure!(
                                reply.domain.as_str()
                                    == crate::store::runtime_namespace(name.as_str()),
                                "retained lookup namespace differs"
                            );
                            match reply.outcome {
                                GenerationMatchOutcome::Found { generation, .. } => generation,
                                GenerationMatchOutcome::Missing => bail!(
                                    "retained image generation unavailable; refusing implicit admission"
                                ),
                            }
                        }
                    };
                    let wanted = crate::store::handoff::expected(&name, backend, generation)?;
                    let info = self.generation(&name, generation)?;
                    let observed = crate::store::handoff::described(&info)?;
                    Resolution::Retained(Box::new(
                        SelectedGeneration::check(wanted, &observed).context(
                            "staged generation differs from complete desired boot identity",
                        )?,
                    ))
                }
            };
            entries.push(entry);
        }
        let staged = durable.check_staged(record, entries)?;
        let mapping = staged
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Resolution::Retained(selected) => Some((
                    selected.selection().domain.clone(),
                    selected.selection().generation,
                )),
                _ => None,
            })
            .map(|(name, generation)| -> Result<_> {
                Ok((
                    name.parse::<GuestName>()?,
                    generation.to_string().parse::<GenerationId>()?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        // Legacy admitted helpers consume this projection. Its publication failure stops activation.
        // Current native execution retains `staged` itself and never re-reads the map.
        state::save(&directory.join("staged.json"), &mapping)?;
        Ok(staged)
    }
}
