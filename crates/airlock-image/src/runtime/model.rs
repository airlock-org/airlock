//! Qubes gives each VM a label, a kernel bundle and storage volumes. Validate
//! those declarations before its offline API writes the initial machine state.

use airlock_machine_core::model::VmReference;
use airlock_nix_core::identity::GuestName;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
pub(crate) struct Topology {
    // VM definitions keyed by their installed Qubes name.
    pub(crate) domains: BTreeMap<String, Domain>,
    // Kernel bundles copied into the offline Qubes pool.
    pub(super) kernels: BTreeMap<String, Kernel>,
    // Machine-level Qubes properties applied after all VM references exist.
    #[serde(default)]
    pub(super) prefs: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
pub(super) struct Kernel {
    // Immutable kernel bundle supplied by the image build.
    pub(super) source: PathBuf,
}

#[derive(Deserialize)]
pub(crate) struct Domain {
    // Initial images support standalone VMs only, never templates or dom0.
    pub(super) class: String,
    // Existing Qubes label or one managed custom label.
    #[serde(default = "green")]
    pub(super) label: String,
    // Network provider assigned only after all VMs have been created.
    #[serde(default)]
    pub(super) netvm: VmReference,
    // Upstream VM preferences, with kernel and providers handled separately.
    #[serde(default)]
    pub(super) prefs: BTreeMap<String, Value>,
    // Airlock and upstream flags converted to Qubes string feature values.
    #[serde(default)]
    pub(super) features: BTreeMap<String, Value>,
    // Qubes service flags become features prefixed with "service.".
    #[serde(default)]
    pub(super) services: BTreeMap<String, Value>,
    // Immutable software and initial state selected for this VM.
    pub(super) backend: Backend,
}

#[derive(Deserialize)]
pub(super) struct Backend {
    // Only the existing immutable closure backend is supported.
    #[serde(rename = "type")]
    pub(super) kind: String,
    // NixOS system whose init path is passed to the VM kernel.
    pub(super) system: Source,
    // Mutable root seed copied before first boot.
    pub(super) root: Source,
    // Prepared admission descriptor for this VM's software view.
    pub(super) store: Source,
    // Upstream Qubes kernel bundle selected by name.
    pub(super) kernel: KernelName,
    // Optional initial private-disk capacity; upstream owns its default.
    pub(super) state: State,
}

#[derive(Deserialize)]
pub(super) struct Source {
    // Path inside the image's pinned Nix inputs or prepared pool.
    pub(super) source: String,
}

#[derive(Deserialize)]
pub(super) struct KernelName {
    // Single safe path component under vm-kernels.
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct State {
    // Requested private-volume bytes; absent or zero uses the upstream default.
    pub(super) persistent_size: Option<u64>,
}

fn green() -> String {
    "green".into()
}

fn safe_component(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
        "invalid runtime path component: {name:?}"
    );
    Ok(())
}

pub(crate) fn feature(value: &Value) -> Result<String> {
    match value {
        Value::Bool(true) => Ok("1".into()),
        Value::Bool(false) => Ok(String::new()),
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        _ => anyhow::bail!("features must be strings, numbers or booleans"),
    }
}

pub(crate) fn custom_color(domain: &Domain) -> Result<Option<String>> {
    let Some(color) = domain.features.get("airlock-label-color") else {
        return Ok(None);
    };

    let text = color.as_str().context("invalid managed custom label")?;
    ensure!(
        matches!(domain.label.as_str(), "khaki" | "light-gray")
            && text.len() == 8
            && text.starts_with("0x")
            && text[2..].bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid managed custom label"
    );
    Ok(Some(text.to_owned()))
}

pub(crate) fn kernelopts(domain: &Domain) -> Result<String> {
    let initial = match domain.prefs.get("kernelopts") {
        None | Some(Value::Null) => "console=hvc0 init=/sbin/init",
        Some(Value::String(text)) if text.is_empty() => "console=hvc0 init=/sbin/init",
        Some(Value::String(text)) => text,
        _ => anyhow::bail!("kernelopts must be a string or null"),
    };
    let mut words: Vec<String> = initial.split_whitespace().map(str::to_owned).collect();

    for (key, value) in [
        ("init", "/sbin/init"),
        ("airlock.system", &domain.backend.system.source),
        ("airlock.storedev", "/dev/xvde"),
        ("airlock.storefmt", "erofs"),
        ("airlock.storeblobdev", "/dev/xvdf"),
    ] {
        ensure!(
            !value.is_empty() && !value.chars().any(char::is_whitespace),
            "invalid kernel option value: {key}"
        );
        let matches: Vec<_> = words
            .iter()
            .filter_map(|word| word.split_once('='))
            .filter(|(k, _)| *k == key)
            .collect();
        ensure!(
            matches.iter().all(|(_, v)| *v == value),
            "conflicting kernel option: {key}"
        );
        if matches.is_empty() {
            words.push(format!("{key}={value}"));
        }
    }

    Ok(words.join(" "))
}

pub(crate) fn validate(topology: &Topology) -> Result<()> {
    for key in topology.prefs.keys() {
        ensure!(
            !matches!(
                key.as_str(),
                "offline_mode" | "events_enabled" | "pools" | "domains" | "labels" | "store"
            ) && !key.starts_with('_'),
            "machine preference may not override offline runtime state: {key}"
        );
    }
    let mut labels = BTreeMap::new();
    for (name, domain) in &topology.domains {
        let _: GuestName = name.parse()?;
        for key in domain.prefs.keys() {
            ensure!(
                !matches!(
                    key.as_str(),
                    "name"
                        | "uuid"
                        | "qid"
                        | "label"
                        | "dir_path"
                        | "volume_config"
                        | "events_enabled"
                        | "storage"
                        | "app"
                ) && !key.starts_with('_'),
                "VM preference may not override managed identity or storage: {key}"
            );
        }
        ensure!(
            domain.class == "StandaloneVM" && domain.backend.kind == "nix-closure",
            "unsupported prepared domain: {name}"
        );

        let color = custom_color(domain)?;
        if let Some(previous) = labels.insert(&domain.label, color.clone()) {
            ensure!(previous == color, "conflicting custom label declarations");
        }

        kernelopts(domain)?;
        safe_component(&domain.backend.kernel.name)?;
        ensure!(
            topology.kernels.contains_key(&domain.backend.kernel.name),
            "missing kernel bundle for {name}"
        );

        if let Some(netvm) = domain.netvm.as_deref() {
            ensure!(
                topology.domains.contains_key(netvm),
                "missing network provider {netvm}"
            );
        }
        if let Some(gui) = domain.prefs.get("guivm").filter(|v| !v.is_null()) {
            ensure!(
                gui.as_str()
                    .is_some_and(|s| topology.domains.contains_key(s)),
                "missing GUI provider for {name}"
            );
        }
        for value in domain.features.values().chain(domain.services.values()) {
            feature(value)?;
        }
    }

    for name in topology.kernels.keys() {
        safe_component(name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
