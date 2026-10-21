//! Compare intended boot artifacts with committed generations before any setup mutation.
//! Record loading, filesystem observations and JSON projection remain adapter duties;
//! the equality and complete-set decision executes the deployment core's Verus code.

use super::{fs::*, generations, prepared, runtime_namespace};
use crate::model::{Backend, GenerationId, Topology};
use airlock_deployment_core::boot::{Handoff, SelectedGeneration, Selection};
use airlock_machine_core::boot::BootInputs;
use airlock_nix_core::identity::GuestName;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

/// Retain every identity and layout field that setup uses to select a guest generation.
pub(crate) fn expected(
    name: &GuestName,
    backend: &Backend,
    generation: GenerationId,
) -> Result<Selection> {
    Ok(Selection {
        domain: name.to_string(),
        generation: generation.to_string().parse()?,
        system: backend.system.source.to_string(),
        root: backend.root.source.to_string(),
        source: backend.store.source.to_string(),
        kernel: backend.kernel.name.clone(),
        mode: serde_json::to_value(backend.store.mode)?
            .as_str()
            .context("invalid store mode")?
            .into(),
        device: backend.store.device.clone(),
        blob_device: backend.store.blob_device.clone(),
        mountpoint: backend.store.mountpoint.clone(),
        backend: backend.store.view_backend.clone(),
    })
}

/// Decode a loaded or described committed generation using its actual identity fields.
pub(crate) fn observed(value: &Value, store: &Value, source: &str) -> Result<Selection> {
    Ok(Selection {
        domain: text(value, "domain")?
            .strip_suffix("-runtime-store")
            .context("boot generation is not a runtime storage view")?
            .to_owned(),
        generation: number(value, "generation")?,
        system: text(value, "system")?,
        root: text(value, "root_source")?,
        source: text(value, source)?,
        kernel: text(value, "kernel_name")?,
        mode: text(store, "mode")?,
        device: text(store, "device")?,
        blob_device: text(store, "blob_device")?,
        mountpoint: text(store, "mountpoint")?,
        backend: text(store, "view_backend")?,
    })
}

/// Compare every explicitly staged VM before privileged boot setup.
/// Releases may stage a subset; the image adapter separately requires complete coverage.
pub(crate) fn selected(
    topology: &Topology,
    mapping: &BTreeMap<GuestName, GenerationId>,
    mut describe: impl FnMut(&GuestName, GenerationId) -> Result<Value>,
) -> Result<Handoff> {
    let mut wanted = Vec::new();
    let mut actual = Vec::new();
    for (name, generation) in mapping {
        let domain = topology
            .domains
            .get(name)
            .with_context(|| format!("staged boot VM is undeclared: {name}"))?;
        let generation = *generation;
        wanted.push(expected(name, &domain.backend, generation)?);
        let described = describe(name, generation)
            .with_context(|| format!("inspect boot generation for {name}"))?;
        // Preserve typed protocol validation in addition to the exact comparison.
        let _: crate::model::GenerationInfo = serde_json::from_value(described.clone())?;
        ensure!(
            described["status"] == "passed",
            "boot generation inspection failed: {name}"
        );
        actual.push(observed(&described, &described["store"], "closure_source")?);
    }
    Handoff::check(wanted, &actual)
        .context("boot artifacts differ from selected committed generations")
}

/// Validate each prepared descriptor and its committed generation as one image handoff.
/// Payload hashes are checked by the existing image-output gate. This additionally
/// checks the metadata hashes and exact generation/descriptor/topology bindings.
pub(super) fn image(topology: Value, physical: &Path, logical: &Path) -> Result<Value> {
    let inputs = BootInputs::new(topology)?;
    let mut wanted = Vec::new();
    let mut actual = Vec::new();
    let mut mapping = BTreeMap::new();
    for (name, domain) in &inputs.topology().domains {
        let backend = &domain.backend;
        let descriptor: Value =
            read(&Path::new(backend.store.source.as_str()).join("prepared.json"))
                .with_context(|| format!("read prepared descriptor for {name}"))?;
        ensure!(
            descriptor["schema"] == "airlock.closure-store.prepared.v1"
                && descriptor["domain"] == runtime_namespace(name.as_str())
                && descriptor["pool_root"].as_str() == logical.to_str(),
            "prepared descriptor binding mismatch: {name}"
        );
        let generation: GenerationId = number(&descriptor, "generation")?.to_string().parse()?;
        let loaded = generations::load(
            physical,
            logical,
            &runtime_namespace(name.as_str()),
            generation.to_string().parse()?,
        )?;
        for field in ["system", "root_source", "kernel_name", "closure_hash"] {
            ensure!(
                descriptor[field] == loaded[field],
                "prepared {field} differs from committed generation: {name}"
            );
        }
        let manifest = super::records::manifest_path(
            physical,
            &runtime_namespace(name.as_str()),
            generation.to_string().parse()?,
        )?;
        let key = manifest
            .strip_prefix(physical)?
            .to_str()
            .context("invalid manifest path")?;
        ensure!(
            descriptor["files"].get(key).is_some(),
            "prepared manifest checksum missing: {name}"
        );
        prepared::verify_files(physical, &descriptor["files"], true)?;

        let kernel = inputs
            .topology()
            .kernels
            .get(&backend.kernel.name)
            .with_context(|| format!("missing boot kernel definition: {name}"))?;
        let kernel = Path::new(
            kernel["source"]
                .as_str()
                .context("missing kernel bundle source")?,
        );
        ensure!(
            fs::metadata(kernel.join("vmlinuz"))?.is_file(),
            "missing boot kernel: {name}"
        );
        wanted.push(expected(name, backend, generation)?);
        actual.push(observed(&loaded, &loaded, "source")?);
        mapping.insert(name.clone(), generation);
    }
    let handoff = Handoff::check(wanted, &actual)
        .context("prepared image topology differs from committed generations")?;
    ensure!(
        handoff.selections().len() == mapping.len(),
        "incomplete checked image handoff"
    );
    Ok(json!({"schema":"airlock.boot-handoff.v1", "status":"passed", "generations":mapping}))
}

/// Revalidate a transported selection against committed records, never admitting new content.
pub(crate) fn authenticate(selected: SelectedGeneration) -> Result<SelectedGeneration> {
    let wanted = selected.selection();
    let root = std::env::var_os("AIRLOCK_CLOSURE_STORE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/var/lib/airlock/closure-store".into());
    let _lock = PoolLock::acquire(&root.join("admission.lock"))?;
    let receipt = generations::load(
        &root,
        &root,
        &runtime_namespace(&wanted.domain),
        wanted.generation,
    )?;
    let actual = observed(&receipt, &receipt, "source")?;
    ensure!(
        selected.setup(&actual).is_some(),
        "committed generation differs from checked setup selection"
    );
    Ok(selected)
}
