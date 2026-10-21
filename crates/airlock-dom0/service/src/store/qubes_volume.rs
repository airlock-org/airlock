//! Airlock's immutable-volume rules behind Qubes' pool and volume callbacks.
//! Device attachment and running references share the storage engine's admission lock.

use super::{
    fs::{PoolLock, component, ensure_pool},
    qubes_inspect::{Definition, ViewKind, absolute, verify},
    records, views,
};
use airlock_nix_core::identity::GuestName;
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Normalize primitive Qubes configuration before constructing an upstream volume.
pub(super) fn normalize(
    domain: Option<&GuestName>,
    config: Value,
    has_source: bool,
) -> Result<Value> {
    let mut config = config
        .as_object()
        .context("invalid Qubes volume configuration")?
        .clone();
    ensure!(
        config.get("rw").is_none_or(|value| value == false),
        "airlock-closure volumes are read-only"
    );
    ensure!(
        config
            .get("snap_on_start")
            .is_none_or(|value| value == false),
        "airlock-closure volumes do not use Qubes snapshots"
    );
    ensure!(
        !has_source,
        "airlock-closure volumes do not use source volumes"
    );

    if let Some(value) = config
        .get_mut("generation")
        .filter(|value| !value.is_null())
    {
        let generation = if let Some(text) = value.as_str() {
            text.parse::<u64>()?
        } else {
            value.as_u64().context("invalid storage generation")?
        };
        ensure!(generation > 0, "generation must be positive");
        *value = json!(generation.to_string());
    }
    if let Some(value) = config.get("domain_name").filter(|value| !value.is_null()) {
        component(value.as_str().context("invalid storage domain")?)?;
    }

    if !config.contains_key("vid") {
        let domain = domain.context("volume identifier requires its Qubes VM")?;
        let generation = config
            .get("generation")
            .and_then(Value::as_str)
            .context("airlock-closure volume requires generation")?;
        let name = config
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("store");
        component(name)?;
        let fallback = if matches!(name, "builder-store" | "builder-data") {
            format!("{domain}-build-store")
        } else {
            super::runtime_namespace(domain.as_str())
        };
        let namespace = config
            .get("domain_name")
            .and_then(Value::as_str)
            .unwrap_or(&fallback);
        let vid = format!("domains/{namespace}/generations/{generation}/{name}");
        let namespace = namespace.to_owned();
        config.insert("vid".into(), json!(vid));
        config.insert("domain_name".into(), json!(namespace));
    }

    let kind = config
        .entry("view_kind")
        .or_insert(json!("metadata-image"))
        .clone();
    let _: ViewKind = serde_json::from_value(kind.clone())?;
    for key in ["view_path", "blob_map_path", "manifest_path"] {
        let value = config.entry(key).or_insert(Value::Null);
        if let Some(path) = value.as_str().filter(|path| !path.is_empty()) {
            let path = std::path::absolute(path)?;
            absolute(&path)?;
            *value = json!(path);
        } else {
            ensure!(value.is_null() || value == "", "invalid storage path");
            *value = Value::Null;
        }
    }

    // Mapper paths exist only while attached and must never be persisted in qubes.xml.
    if kind == "erofs-blob-map" {
        config.insert("view_path".into(), Value::Null);
    }
    if let Some(value) = config
        .get_mut("blob_logical_size")
        .filter(|value| !value.is_null())
    {
        let size = if let Some(text) = value.as_str() {
            text.parse::<u64>()?
        } else {
            value.as_u64().context("invalid blob logical size")?
        };
        ensure!(size > 0, "blob logical size must be positive");
        *value = json!(size);
    }
    for key in [
        "domain_name",
        "generation",
        "closure_hash",
        "blob_logical_size",
    ] {
        config.entry(key).or_insert(Value::Null);
    }
    config.insert("rw".into(), json!(false));
    config.insert("snap_on_start".into(), json!(false));
    config.insert("save_on_stop".into(), json!(true));

    let mut fields = serde_json::Map::new();
    for key in [
        "domain_name",
        "generation",
        "closure_hash",
        "view_kind",
        "view_path",
        "blob_map_path",
        "blob_logical_size",
        "manifest_path",
    ] {
        fields.insert(
            key.into(),
            config.remove(key).expect("normalized metadata field"),
        );
    }
    let persisted = fields
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_json::Map<_, _>>();
    Ok(json!({"config":config,"fields":fields,"persisted":persisted}))
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Action {
    Create,
    Verify,
    Start,
    Stop,
    BlockDevice,
    Immutable,
}

/// One Qubes callback with its current primitive configuration and attached path.
#[derive(Deserialize)]
pub(super) struct Request {
    // Observable lifecycle operation selected by Qubes, never a shell command.
    action: Action,
    // Pool whose admission lock protects mapper and running-reference changes.
    root: PathBuf,
    // Store namespace; ordinary metadata volumes may have no retained generation.
    domain: Option<String>,
    // Persisted decimal generation identifier shared with upstream Qubes.
    generation: Option<String>,
    // Independent running-reference key, such as store or store-data.
    name: String,
    // Immutable view plus its transient mapper path, if already attached.
    #[serde(flatten)]
    definition: Definition,
}

/// Validate a filtered mapper returned by the engine before exposing it to Qubes.
fn attached_device(value: Value) -> Result<PathBuf> {
    filtered_device(value, |path| fs::canonicalize(path))
}

/// Check the mapper namespace and resolved backing aliases with one filesystem resolver.
fn filtered_device(
    value: Value,
    resolve: impl Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<PathBuf> {
    let device = PathBuf::from(
        value["device"]
            .as_str()
            .context("blob-map attach returned no device")?,
    );
    ensure!(
        device.starts_with("/dev/mapper")
            && device.parent() == Some(std::path::Path::new("/dev/mapper")),
        "blob-map attach returned an unfiltered device"
    );
    let resolved = resolve(&device)?;
    for backing in value["backing_paths"]
        .as_array()
        .context("missing blob backing paths")?
    {
        let backing = backing.as_str().context("invalid blob backing path")?;
        ensure!(
            resolved != resolve(Path::new(backing))?,
            "refusing to expose global Airlock backing storage to a VM"
        );
    }
    Ok(device)
}

/// Execute a complete callback so Python never coordinates device and pin changes.
pub(super) fn execute(mut request: Request) -> Result<Value> {
    if matches!(request.action, Action::Immutable) {
        bail!("airlock-closure volumes are immutable");
    }
    absolute(&request.root)?;
    component(&request.name)?;
    let identity = request
        .domain
        .as_deref()
        .zip(request.generation.as_deref())
        .map(|(domain, generation)| -> Result<_> {
            component(domain)?;
            let generation = generation.parse::<u64>()?;
            ensure!(generation > 0, "generation must be positive");
            Ok((domain, generation))
        })
        .transpose()?;
    let blob = matches!(request.definition.view_kind, ViewKind::ErofsBlobMap);
    let runtime = matches!(
        request.action,
        Action::Start | Action::Stop | Action::Verify | Action::BlockDevice
    );
    let pins = identity.is_some() && matches!(request.action, Action::Start | Action::Stop);
    let _guard = if (blob && runtime) || pins {
        ensure_pool(&request.root)?;
        Some(PoolLock::acquire(&request.root.join("admission.lock"))?)
    } else {
        None
    };

    if matches!(request.action, Action::Stop) {
        if let Some((domain, generation)) = identity {
            if blob {
                views::attach(&request.root, domain, generation, false)?;
                request.definition.view_path = None;
            }
            records::running(&request.root, domain, generation, &request.name, false)?;
        }
    } else {
        verify(&request.definition)?;
        if blob
            && !matches!(request.action, Action::Create)
            && (matches!(request.action, Action::Start)
                || !request
                    .definition
                    .view_path
                    .as_ref()
                    .is_some_and(|path| path.exists()))
        {
            let (domain, generation) =
                identity.context("missing domain/generation for blob-map view")?;
            request.definition.view_path = Some(attached_device(views::attach(
                &request.root,
                domain,
                generation,
                true,
            )?)?);
        }
        if matches!(request.action, Action::Start)
            && let Some((domain, generation)) = identity
        {
            records::running(&request.root, domain, generation, &request.name, true)?;
        }
    }

    Ok(json!({"path":request.definition.view_path,"valid":true}))
}

#[cfg(test)]
mod tests;
