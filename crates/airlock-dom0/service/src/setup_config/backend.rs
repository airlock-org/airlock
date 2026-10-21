//! Native closure boot metadata and private/root/store volume reconciliation through existing adapters.

use super::runtime::{Session, path};
use airlock_deployment_core::storage as verified;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Translate logical store paths only for the explicitly selected private Nix root.
fn physical(source: &str) -> PathBuf {
    let root = std::env::var_os("AIRLOCK_NIX_ROOT").map(PathBuf::from);
    if std::env::var("AIRLOCK_USE_HOST_NIX_STORE").as_deref() != Ok("1")
        && source.starts_with("/nix/store/")
        && let Some(root) = root
        && root != Path::new("/")
        && !root.as_os_str().is_empty()
    {
        return root.join(source.trim_start_matches('/'));
    }
    source.into()
}

/// Compare immediate source files, matching upstream kernel adoption rules.
fn kernel_matches(source: &Path, target: &Path) -> Result<bool> {
    if !target.join("vmlinuz").is_file() {
        return Ok(false);
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && (!target.join(entry.file_name()).is_file()
                || fs::read(entry.path())? != fs::read(target.join(entry.file_name()))?)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Install or adopt an exact kernel source; refuse to overwrite unrelated Qubes kernels.
pub(super) fn kernel(session: &mut Session, domain: &str, name: &str, source: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name != "." && name != ".." && !name.contains('/'),
        "invalid kernel name: {name}"
    );
    let source_path = physical(source);
    ensure!(
        source_path.is_dir() && source_path.join("vmlinuz").is_file(),
        "kernel source vmlinuz is missing: {source}"
    );
    let base = path("AIRLOCK_QUBES_KERNEL_BASE", "/var/lib/qubes/vm-kernels");
    let target = base.join(name);
    let marker = target.join("MANAGED_BY_AIRLOCK");
    let matches = kernel_matches(&source_path, &target)?;
    let source_line = format!("source={source}", source = source_path.display());
    if matches
        && fs::read_to_string(&marker)
            .is_ok_and(|text| text.lines().any(|line| line == source_line))
    {
        return Ok(());
    }
    let adopt = target.is_dir() && !marker.is_file();
    ensure!(
        !adopt || matches,
        "kernel exists but is not managed by Airlock: {target}",
        target = target.display()
    );
    if session.apply {
        if !adopt {
            fs::create_dir_all(&base)?;
            let temporary = tempfile::Builder::new()
                .prefix(".airlock-kernel-")
                .tempdir_in(&base)?;
            session.command(
                "cp",
                &[
                    "-a",
                    &format!("{source}/.", source = source_path.display()),
                    temporary.path().to_str().context("kernel temporary path")?,
                ],
            )?;
            fs::write(
                temporary.path().join("MANAGED_BY_AIRLOCK"),
                format!("{source_line}\nschema=airlock.qubes-kernel.v1\n"),
            )?;
            if target.try_exists()? {
                fs::remove_dir_all(&target)?;
            }
            fs::rename(temporary.path(), &target)?;
        } else {
            fs::write(
                marker,
                format!("{source_line}\nschema=airlock.qubes-kernel.v1\n"),
            )?;
        }
    }
    session.count("kernels_changed")?;
    session.action(
        "kernel",
        if adopt { "adopt" } else { "install" },
        domain,
        name,
        source,
    )?;
    Ok(())
}

/// Append a required boot option or reject a conflicting user-specified value.
fn option(options: &mut String, key: &str, value: &str) -> Result<()> {
    let prefix = format!("{key}=");
    let declared = options
        .split(' ')
        .filter_map(|word| word.strip_prefix(&prefix))
        .collect::<Vec<_>>();
    if !declared.is_empty() {
        ensure!(
            declared.iter().all(|current| *current == value),
            "nix-closure kernelopts {key} conflicts with expected {value}"
        );
    } else {
        if !options.is_empty() {
            options.push(' ');
        }
        options.push_str(&format!("{key}={value}"));
    }
    Ok(())
}

/// Return a backend string default without accepting malformed object/array values.
fn field<'a>(value: &'a Value, key: &str, default: &'a str) -> Result<&'a str> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => value
            .as_str()
            .with_context(|| format!("backend {key} must be a string")),
    }
}

/// Read a declared source and reject an empty identity before any volume mutation.
fn source<'a>(backend: &'a Value, component: &str) -> Result<&'a str> {
    backend[component]["source"]
        .as_str()
        .filter(|source| !source.is_empty())
        .with_context(|| format!("Nix-native domain {component} source is missing"))
}

/// Read one observed feature only in apply mode, keeping plan mode independent of mutable metadata.
fn feature(session: &mut Session, domain: &str, key: &str) -> Option<String> {
    if session.apply {
        session.command("qvm-features", &[domain, key]).ok()
    } else {
        None
    }
}

/// Enlarge private state only; never shrink it or resize a running VM's volume.
fn private(
    session: &mut Session,
    domain: &str,
    desired: &Value,
    authority: Option<&verified::Backend<'_>>,
) -> Result<()> {
    if desired.is_null() {
        return Ok(());
    }
    let desired = desired
        .as_u64()
        .context("invalid persistent state volume size")?;
    let mut current_size = None;
    if session.apply {
        let current = session.command("qvm-volume", &["info", &format!("{domain}:private")]);
        current_size = current.as_ref().ok().and_then(|current| {
            current.lines().find_map(|line| {
                let mut words = line.split_whitespace();
                (words.next() == Some("size"))
                    .then(|| words.next().and_then(|value| value.parse::<u64>().ok()))
                    .flatten()
            })
        });
        if current_size.is_some_and(|current| current >= desired) {
            return Ok(());
        }
        if session.running(domain)? {
            return session.defer(
                domain,
                "domain-state-volume",
                "private",
                &desired.to_string(),
            );
        }
        // Missing capacity can justify deferring a known active VM, never resizing it.
        current_size.context("private capacity is missing or invalid")?;
    }
    if let Some(authority) = authority {
        let observed = power(session, domain)?;
        let request = authority
            .resize(&observed, current_size.context("missing private capacity")?)
            .context("private resize differs from checked capacity or halted target")?;
        resize(session, request)?;
    } else {
        session.change(
            "qvm-volume",
            &["resize", &format!("{domain}:private"), &desired.to_string()],
        )?;
    }
    session.count("state_volumes_changed")?;
    session.action(
        "domain-state-volume",
        "resize",
        domain,
        "private",
        &desired.to_string(),
    )?;
    Ok(())
}

/// Reconcile the native backend without changing a running VM's recorded boot identity prematurely.
pub(super) fn reconcile(session: &mut Session, domain: &str) -> Result<()> {
    reconcile_with(session, domain, None, None)
}

/// Execute a selected backend after rechecking its topology, including explicit backend-only invocations.
pub(super) fn reconcile_selected(
    session: &mut Session,
    domain: &str,
    selected: &airlock_deployment_core::boot::SelectedGeneration,
) -> Result<()> {
    reconcile_with(session, domain, None, Some(selected))
}

/// Share reconciliation observations while deployment mutation consumes opaque requests.
fn reconcile_with(
    session: &mut Session,
    domain: &str,
    authority: Option<&verified::Backend<'_>>,
    selected: Option<&airlock_deployment_core::boot::SelectedGeneration>,
) -> Result<()> {
    let definition = session.topology["domains"][domain].clone();
    if definition["backend"]["type"] != "nix-closure" {
        ensure!(
            selected.is_none(),
            "selected boot backend is missing or unsupported"
        );
        return Ok(());
    }
    ensure!(
        field(&definition, "class", "StandaloneVM")? == "StandaloneVM",
        "nix-closure backend requires StandaloneVM class for {domain}"
    );
    let backend = &definition["backend"];
    let checked = selected
        .map(|selected| -> Result<_> {
            let backend: crate::model::Backend = serde_json::from_value(backend.clone())?;
            let generation = selected.selection().generation.to_string().parse()?;
            let wanted = crate::store::handoff::expected(&domain.parse()?, &backend, generation)?;
            selected
                .setup(&wanted)
                .context("backend differs from checked boot selection")
        })
        .transpose()?;
    let chosen = checked.as_ref().map(|request| request.selection());
    let system = match chosen {
        Some(s) => s.system.as_str(),
        None => source(backend, "system")?,
    };
    let root = match chosen {
        Some(s) => s.root.as_str(),
        None => source(backend, "root")?,
    };
    let store = match chosen {
        Some(s) => s.source.as_str(),
        None => source(backend, "store")?,
    };
    let store_config = &backend["store"];
    let mode = field(store_config, "mode", "shared-closure")?;
    let view = field(store_config, "view_backend", "erofs-objectmap")?;
    let mount = field(store_config, "mountpoint", "/nix/store")?;
    let device = field(store_config, "device", "/dev/xvde")?;
    let blob_device = field(store_config, "blob_device", "/dev/xvdf")?;
    ensure!(
        mode == "shared-closure",
        "unsupported Nix-native domain store mode for {domain}: {mode}"
    );
    ensure!(
        view == "erofs-objectmap"
            && mount == "/nix/store"
            && device == "/dev/xvde"
            && blob_device == "/dev/xvdf",
        "unsupported Nix-native store layout for {domain}"
    );
    let default_kernel = format!("airlock-{domain}");
    let declared_kernel = field(&backend["kernel"], "name", "")?;
    let kernel_name = if declared_kernel.is_empty() || declared_kernel == "null" {
        &default_kernel
    } else {
        declared_kernel
    };
    let kernel_source = field(&backend["kernel"], "source", "")?;

    let staged: Value = match &session.staged {
        Some(path) if path.is_file() => crate::state::read(path)?,
        _ => Value::Null,
    };
    let generation = staged
        .get(domain)
        .filter(|value| !value.is_null())
        .map(|value| match value {
            Value::String(value) => value.clone(),
            _ => value.to_string(),
        });
    let generation = match chosen {
        Some(s) => {
            if let Some(ambient) = &generation {
                ensure!(
                    ambient.parse::<u64>()? == s.generation,
                    "ambient generation conflicts with checked setup"
                );
            }
            Some(s.generation.to_string())
        }
        None => generation,
    };
    let kernel_name = chosen.map_or(kernel_name, |s| s.kernel.as_str());
    if !kernel_source.is_empty() {
        if let Some(authority) = authority {
            let request = authority
                .kernel()
                .context("kernel absent from checked backend")?;
            install_kernel(session, request)?;
        } else {
            kernel(session, domain, kernel_name, kernel_source)?;
        }
    }

    if let Some(authority) = authority {
        let native = authority
            .definition()
            .native
            .as_ref()
            .context("missing native backend")?;
        if let Some(expected) = native.generation {
            ensure!(
                generation.as_ref().and_then(|g| g.parse::<u64>().ok()) == Some(expected),
                "staged generation differs from original release"
            );
        }
    }
    let selected_generation = generation.as_ref().map(|g| g.parse::<u64>()).transpose()?;
    let store_request = authority
        .map(|a| {
            a.store(selected_generation)
                .context("store generation differs from checked backend")
        })
        .transpose()?;
    let artifact = if generation.is_some() {
        PathBuf::from(store)
    } else {
        let artifact = physical(store);
        ensure!(
            artifact.is_dir()
                && artifact.join("store-paths").is_file()
                && artifact.join("manifest.json").is_file(),
            "Nix-native shared closure source is missing store-paths or manifest.json: {store}"
        );
        artifact
    };
    let names = session.command(
        "qvm-ls",
        &["--raw-data", "--fields", "NAME", "--no-spinner"],
    )?;
    let exists = names.lines().any(|name| name == domain);
    if session.apply || exists {
        ensure!(exists, "Rust configuration has not created {domain}");
        let classes = session.command(
            "qvm-ls",
            &["--raw-data", "--fields", "NAME,CLASS", "--no-spinner"],
        )?;
        ensure!(
            classes
                .lines()
                .any(|line| line == format!("{domain}|StandaloneVM")),
            "nix-closure domain class is not StandaloneVM: {domain}"
        );
    }

    let mut root_image = PathBuf::new();
    let mut root_hash = String::new();
    let mut resolved = None;
    if generation.is_some() || !Path::new(root).exists() {
        let namespace = crate::store::runtime_namespace(domain);
        let selected_args = checked
            .as_ref()
            .map(|request| super::selection::arguments(request.root_arguments()));
        let mut args = vec![
            "storage",
            "pool",
            "resolve-root",
            "--domain",
            &namespace,
            "--root-source",
            root,
        ];
        if let Some(generation) = &generation {
            args.extend(["--generation", generation]);
        }
        let record: Value = serde_json::from_str(&if let Some(args) = selected_args {
            session.service(&args.iter().map(String::as_str).collect::<Vec<_>>())?
        } else {
            session.service(&args)?
        })?;
        ensure!(
            record["domain"] == namespace,
            "resolved root namespace mismatch"
        );
        if checked.is_some() {
            ensure!(
                record["root_source"] == root,
                "resolved root differs from checked boot selection"
            );
        }
        root_image = PathBuf::from(
            record["image"]
                .as_str()
                .context("missing resolved root image")?,
        );
        root_hash = record["artifact"]["sha256"]
            .as_str()
            .context("missing resolved root artifact hash")?
            .into();
        resolved = Some(record);
    }
    let root_current = feature(session, domain, "airlock-root-source").as_deref() == Some(root)
        && (root_hash.is_empty()
            || feature(session, domain, "airlock-root-artifact").as_deref() == Some(&root_hash));
    let root_deferred = !root_current && session.running(domain)?;
    if root_deferred {
        session.defer(domain, "domain-root", "source", root)?;
    } else if !root_current {
        if root_hash.is_empty() {
            root_image = physical(root);
            if root_image.is_dir() {
                root_image = root_image.join("root.img");
            }
            ensure!(
                root_image.is_file(),
                "Nix-native root image source is not a file: {root}"
            );
        }
        let service = path("AIRLOCK_DOM0_SERVICE", "airlock-dom0-service");
        session.change(
            service.to_str().context("service path")?,
            &["storage", "domain", "verify-native-volumes", domain],
        )?;
        if let Some(authority) = authority {
            let artifact = verified::RootArtifact {
                name: domain.into(),
                source: resolved
                    .as_ref()
                    .map_or(Ok(root), |r| {
                        r["root_source"]
                            .as_str()
                            .context("resolved root source missing")
                    })?
                    .into(),
                image: root_image.to_str().context("root image path")?.into(),
                hash: root_hash.clone(),
            };
            let observed = power(session, domain)?;
            import_root(
                session,
                authority
                    .root(&artifact, &observed)
                    .context("root import differs from checked source or halted target")?,
            )?;
        } else if let Some(request) = &checked {
            let artifact = verified::RootArtifact {
                name: domain.into(),
                source: root.into(),
                image: root_image.to_str().context("root image path")?.into(),
                hash: root_hash.clone(),
            };
            let observed = power(session, domain)?;
            let request = request
                .root(&artifact, &observed)
                .context("root import differs from selected generation or halted VM")?;
            import_selected_root(session, request)?;
        } else {
            session.change(
                "qvm-volume",
                &[
                    "import",
                    &format!("{domain}:root"),
                    root_image.to_str().context("root image path")?,
                ],
            )?;
        }
        session.count("root_images_changed")?;
        session.action("domain-root", "import", domain, "source", root)?;
    }
    private(
        session,
        domain,
        &backend["state"]["persistent_size"],
        authority,
    )?;

    let current_mode = feature(session, domain, "airlock-store-mode")
        .filter(|mode| !mode.is_empty())
        .unwrap_or_else(|| "shared-closure".into());
    let store_current = feature(session, domain, "airlock-store-source").as_deref() == Some(store)
        && current_mode == mode;
    let store_deferred = !store_current && session.running(domain)?;
    let mut store_result = Value::Null;
    let mut observed_store = None;
    if store_deferred {
        session.defer(domain, "domain-store", "source", store)?;
    } else if session.apply {
        store_result = serde_json::from_str(&if let Some(request) = &checked {
            let args = super::selection::arguments(request.store_arguments());
            session.service(&args.iter().map(String::as_str).collect::<Vec<_>>())?
        } else if authority.is_some() {
            install_store(
                session,
                store_request
                    .as_ref()
                    .context("missing checked native store")?,
            )?
        } else {
            session.service(&[
                "storage",
                "domain",
                "ensure-store",
                domain,
                artifact.to_str().context("store artifact path")?,
                store,
                "--mode",
                mode,
                "--system",
                system,
                "--root",
                root,
                "--kernel",
                kernel_name,
                "--device",
                device,
                "--blob-device",
                blob_device,
                "--mountpoint",
                mount,
                "--view-backend",
                view,
            ])?
        })?;
        if let Some(request) = &checked {
            let observed = crate::store::handoff::installed(&store_result)?;
            ensure!(
                selected
                    .context("missing selected generation")?
                    .setup(&observed)
                    .is_some(),
                "storage outcome differs from checked setup selection"
            );
            ensure!(
                request.selection().generation == observed.generation,
                "storage selected another generation"
            );
        }
        if authority.is_some() {
            let evidence = store_evidence(&store_result)?;
            store_request
                .as_ref()
                .context("missing checked store request")?
                .outcome(&evidence)
                .context("store receipt differs from approved boot identity or generation")?;
            observed_store = Some(evidence);
        }
        let volumes = match store_result.get("volumes_created") {
            Some(value) => value
                .as_array()
                .context("invalid created volumes")?
                .iter()
                .map(|value| value.as_str().context("invalid created volume"))
                .collect::<Result<Vec<_>>>()?,
            None if store_result["volume_created"] == true => vec!["store"],
            _ => vec![],
        };
        for volume in volumes {
            session.count("store_volumes_changed")?;
            session.action("domain-store-volume", "create", domain, "volume", volume)?;
        }
        if store_result["view_created"] == true {
            session.count("store_views_changed")?;
            session.action("domain-store-view", "materialize", domain, "source", store)?;
        }
        if store_result["qubesd_restarted"] == true
            || store_result["qubesd_restart_required"] == true
        {
            if store_result["qubesd_restarted"] != true {
                if let Some(request) = &store_request {
                    let outcome = request
                        .outcome(observed_store.as_ref().context("missing storage outcome")?)
                        .context("storage outcome changed")?;
                    restart_daemon(
                        session,
                        outcome
                            .restart()
                            .context("storage outcome does not require daemon restoration")?,
                    )?;
                } else {
                    session.change("systemctl", &["try-restart", "qubesd.service"])?;
                }
            }
            session.count("qubesd_restarts")?;
            session.action("systemd", "restart", "", "service", "qubesd.service")?;
        }
    } else {
        for volume in ["store", "store-data"] {
            session.count("store_volumes_changed")?;
            session.action("domain-store-volume", "create", domain, "volume", volume)?;
        }
        session.count("store_views_changed")?;
        session.action("domain-store-view", "materialize", domain, "source", store)?;
    }
    let accepted = match (store_request.as_ref(), observed_store.as_ref()) {
        (Some(request), Some(evidence)) => Some(
            request
                .outcome(evidence)
                .context("storage outcome changed")?,
        ),
        _ => None,
    };
    let root_evidence = verified::RootArtifact {
        name: domain.into(),
        source: root.into(),
        image: root_image.to_str().context("root image path")?.into(),
        hash: root_hash.clone(),
    };
    if session.apply || exists {
        let deferred = root_deferred || store_deferred;
        if deferred {
            if feature(session, domain, "airlock-system-source").as_deref() != Some(system) {
                session.defer(domain, "domain-system", "source", system)?;
            }
        } else {
            setting(
                session,
                authority,
                accepted.as_ref(),
                &root_evidence,
                checked.as_ref(),
                ("qvm-prefs", domain, "kernel", kernel_name),
            )?;
            let mut options = field(&definition["prefs"], "kernelopts", "")?.to_owned();
            if options.is_empty() || options == "null" {
                options = "console=hvc0 init=/sbin/init".into();
            }
            for (key, value) in [
                ("init", "/sbin/init"),
                ("airlock.system", system),
                ("airlock.storedev", device),
                ("airlock.storefmt", "erofs"),
                ("airlock.storeblobdev", blob_device),
            ] {
                option(&mut options, key, value)?;
            }
            setting(
                session,
                authority,
                accepted.as_ref(),
                &root_evidence,
                checked.as_ref(),
                ("qvm-prefs", domain, "kernelopts", &options),
            )?;
        }
        setting(
            session,
            authority,
            accepted.as_ref(),
            &root_evidence,
            checked.as_ref(),
            ("qvm-features", domain, "airlock-backend", "nix-closure"),
        )?;
        if !deferred {
            for (key, value) in [
                ("airlock-system-source", system),
                ("airlock-root-source", root),
                ("airlock-root-artifact", root_hash.as_str()),
                ("airlock-store-source", store),
                ("airlock-store-mode", mode),
                ("airlock-store-view-backend", view),
                ("airlock-store-filesystem", "erofs"),
                ("airlock-store-mountpoint", mount),
                ("airlock-store-device", device),
                ("airlock-store-blob-device", blob_device),
                ("airlock-store-read-only", "True"),
            ] {
                setting(
                    session,
                    authority,
                    accepted.as_ref(),
                    &root_evidence,
                    checked.as_ref(),
                    ("qvm-features", domain, key, value),
                )?;
            }
            for (key, field) in [
                ("airlock-store-generation", "generation"),
                ("airlock-store-closure-hash", "closure_hash"),
                ("airlock-store-view-image", "view_image"),
                ("airlock-store-blob-map", "blob_map"),
                ("airlock-store-blob-volume", "blob_volume"),
            ] {
                let value = if field == "view_image" && store_result[field].is_null() {
                    &store_result["image"]
                } else {
                    &store_result[field]
                };
                if !value.is_null() {
                    let value = match value {
                        Value::String(value) => value.clone(),
                        _ => value.to_string(),
                    };
                    if !value.is_empty() {
                        setting(
                            session,
                            authority,
                            accepted.as_ref(),
                            &root_evidence,
                            checked.as_ref(),
                            ("qvm-features", domain, key, &value),
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Decode every native backend from the admitted topology before authority publication.
pub(crate) fn definitions(
    topology: &Value,
    service: &str,
    generations: &std::collections::BTreeMap<
        airlock_nix_core::identity::GuestName,
        crate::model::GenerationId,
    >,
) -> Result<Vec<verified::Definition>> {
    topology["domains"]
        .as_object()
        .context("missing topology domains")?
        .iter()
        .map(|(name, d)| {
            let native = if d["backend"]["type"] == "nix-closure" {
                let b = &d["backend"];
                ensure!(
                    field(d, "class", "StandaloneVM")? == "StandaloneVM",
                    "native backend requires StandaloneVM"
                );
                let system = source(b, "system")?;
                let root = source(b, "root")?;
                let store = source(b, "store")?;
                ensure!(
                    field(&b["store"], "mode", "shared-closure")? == "shared-closure"
                        && field(&b["store"], "view_backend", "erofs-objectmap")?
                            == "erofs-objectmap"
                        && field(&b["store"], "mountpoint", "/nix/store")? == "/nix/store"
                        && field(&b["store"], "device", "/dev/xvde")? == "/dev/xvde"
                        && field(&b["store"], "blob_device", "/dev/xvdf")? == "/dev/xvdf",
                    "unsupported native store layout"
                );
                let declared = field(&b["kernel"], "name", "")?;
                let kernel = if declared.is_empty() || declared == "null" {
                    format!("airlock-{name}")
                } else {
                    declared.into()
                };
                ensure!(
                    !kernel.is_empty() && kernel != "." && kernel != ".." && !kernel.contains('/'),
                    "invalid native kernel name"
                );
                let private_size = match &b["state"]["persistent_size"] {
                    Value::Null => None,
                    value => Some(value.as_u64().context("invalid private capacity")?),
                };
                let generation = generations
                    .get(&name.parse()?)
                    .map(|g| g.to_string().parse())
                    .transpose()?;
                let mut kernelopts = field(&d["prefs"], "kernelopts", "")?.to_owned();
                if kernelopts.is_empty() || kernelopts == "null" {
                    kernelopts = "console=hvc0 init=/sbin/init".into();
                }
                for (key, value) in [
                    ("init", "/sbin/init"),
                    ("airlock.system", system),
                    ("airlock.storedev", "/dev/xvde"),
                    ("airlock.storefmt", "erofs"),
                    ("airlock.storeblobdev", "/dev/xvdf"),
                ] {
                    option(&mut kernelopts, key, value)?;
                }
                Some(verified::Native {
                    system: system.into(),
                    root: root.into(),
                    source: store.into(),
                    artifact: physical(store)
                        .to_str()
                        .context("invalid artifact path")?
                        .into(),
                    kernel,
                    kernel_source: field(&b["kernel"], "source", "")?.into(),
                    kernelopts,
                    private_size,
                    generation,
                })
            } else {
                None
            };
            Ok(verified::Definition {
                name: name.clone(),
                native,
                service: service.into(),
            })
        })
        .collect()
}

/// Decode storage's full boot identity; all fields are compared in verified executable Rust.
fn store_evidence(value: &Value) -> Result<verified::StoreEvidence> {
    let required = |key: &str| {
        value[key]
            .as_str()
            .with_context(|| format!("missing storage receipt {key}"))
    };
    let optional = |key: &str| -> Result<String> {
        match &value[key] {
            Value::Null => Ok(String::new()),
            Value::String(v) => Ok(v.clone()),
            _ => anyhow::bail!("invalid storage receipt {key}"),
        }
    };
    let generation = value["generation"]
        .as_u64()
        .context("invalid storage receipt generation")?;
    Ok(verified::StoreEvidence {
        name: required("domain")?.into(),
        system: required("system")?.into(),
        root: required("root_source")?.into(),
        source: required("source")?.into(),
        kernel: required("kernel_name")?.into(),
        generation,
        generation_text: generation.to_string(),
        closure_hash: optional("closure_hash")?,
        view_image: if value["view_image"].is_null() {
            optional("image")?
        } else {
            optional("view_image")?
        },
        blob_map: optional("blob_map")?,
        blob_volume: optional("blob_volume")?,
        restart_required: value["qubesd_restart_required"] == true,
        restarted: value["qubesd_restarted"] == true,
    })
}

/// Select derived settings by typed fields; unknown keys cannot fall back to a raw mutation.
fn setting(
    session: &mut Session,
    authority: Option<&verified::Backend<'_>>,
    outcome: Option<&verified::StoreOutcome<'_>>,
    root: &verified::RootArtifact,
    selected: Option<&airlock_deployment_core::boot::SetupRequest<'_>>,
    proposed: (&str, &str, &str, &str),
) -> Result<()> {
    let (program, domain, key, value) = proposed;
    if let Some(selected) = selected {
        use airlock_deployment_core::boot::BootField;
        let field = match (program, key) {
            ("qvm-prefs", "kernel") => Some(BootField::Kernel),
            ("qvm-features", "airlock-system-source") => Some(BootField::System),
            ("qvm-features", "airlock-root-source") => Some(BootField::Root),
            ("qvm-features", "airlock-store-source") => Some(BootField::Store),
            _ => None,
        };
        if let Some(field) = field {
            return selected_setting(session, selected, field);
        }
    }
    let Some(authority) = authority else {
        return session.setting(program, domain, key, value);
    };
    if session
        .command(program, &[domain, key])
        .is_ok_and(|current| {
            airlock_deployment_core::configuration::value_matches(value, Some(&current))
        })
    {
        return Ok(());
    }
    let observed = power(session, domain)?;
    let request = if key == "airlock-root-artifact" {
        authority.root_setting(root, &observed)
    } else if let Some(field) = match key {
        "airlock-store-generation" => Some(verified::ReceiptField::Generation),
        "airlock-store-closure-hash" => Some(verified::ReceiptField::ClosureHash),
        "airlock-store-view-image" => Some(verified::ReceiptField::ViewImage),
        "airlock-store-blob-map" => Some(verified::ReceiptField::BlobMap),
        "airlock-store-blob-volume" => Some(verified::ReceiptField::BlobVolume),
        _ => None,
    } {
        outcome
            .context("missing accepted generation receipt")?
            .setting(field, &observed)
    } else {
        let field = match key {
            "kernel" => verified::BootField::Kernel,
            "kernelopts" => verified::BootField::Kernelopts,
            "airlock-backend" => verified::BootField::Backend,
            "airlock-system-source" => verified::BootField::System,
            "airlock-root-source" => verified::BootField::Root,
            "airlock-store-source" => verified::BootField::Source,
            "airlock-store-mode" => verified::BootField::Mode,
            "airlock-store-view-backend" => verified::BootField::ViewBackend,
            "airlock-store-filesystem" => verified::BootField::Filesystem,
            "airlock-store-mountpoint" => verified::BootField::Mountpoint,
            "airlock-store-device" => verified::BootField::Device,
            "airlock-store-blob-device" => verified::BootField::BlobDevice,
            "airlock-store-read-only" => verified::BootField::ReadOnly,
            _ => anyhow::bail!("unclassified native boot setting: {key}"),
        };
        authority.setting(field, &observed)
    }
    .context("boot setting requires original identity and named halted state")?;
    let args = request.arguments();
    ensure!(
        request.program() == program && args == [domain, key, value],
        "derived boot setting differs from checked backend"
    );
    apply_setting(session, request)
}

/// Consume a checked setting without separate VM, feature or value parameters.
fn apply_setting(session: &mut Session, request: verified::BootRequest<'_>) -> Result<()> {
    let args = request.arguments();
    session.setting(request.program(), &args[0], &args[1], &args[2])
}

/// Restore only qubesd when a validated storage outcome requires it.
fn restart_daemon(session: &mut Session, request: verified::RestartRequest<'_>) -> Result<()> {
    let args = request.arguments();
    session.change(
        "systemctl",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Query the exact named state again immediately before a destructive volume request.
fn power(session: &mut Session, name: &str) -> Result<verified::Power> {
    Ok(verified::Power {
        name: name.into(),
        state: airlock_deployment_core::inventory::PowerState::classify(&session.state(name)?),
    })
}

/// Consume the checked kernel identity at the filesystem installation boundary.
fn install_kernel(session: &mut Session, request: verified::KernelRequest<'_>) -> Result<()> {
    let (domain, name, source) = request.parameters();
    kernel(session, domain, name, source)
}

/// Forward only the verified root-volume target and admitted image.
fn import_root(session: &mut Session, request: verified::RootRequest<'_>) -> Result<()> {
    let args = request.arguments();
    session.change(
        "qvm-volume",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Consume the checked root request without accepting an independent VM or image path.
fn import_selected_root(
    session: &mut Session,
    request: airlock_deployment_core::boot::RootRequest<'_>,
) -> Result<()> {
    let args = request.arguments();
    session.change(
        "qvm-volume",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Keep the fixed property/feature key and value selected by the verified request.
fn selected_setting(
    session: &mut Session,
    request: &airlock_deployment_core::boot::SetupRequest<'_>,
    field: airlock_deployment_core::boot::BootField,
) -> Result<()> {
    let program = request.setting_program(field);
    let args = request.setting_arguments(field);
    session.setting(program, &args[0], &args[1], &args[2])
}

/// Decimal rendering cannot substitute a different capacity or volume selector.
fn resize(session: &mut Session, request: verified::ResizeRequest<'_>) -> Result<()> {
    session.change(
        "qvm-volume",
        &["resize", &request.volume(), &request.size().to_string()],
    )
}

/// The storage service retains admission, locking and offline-attachment safeguards.
fn install_store(session: &mut Session, request: &verified::StoreRequest<'_>) -> Result<String> {
    let mut args = request.arguments();
    if let Some(generation) = request.generation() {
        args.extend(["--generation".into(), generation.to_string()]);
    }
    session.command(
        request.program(),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Reconcile a selected boot generation in-process and retain a run-specific helper receipt.
pub(crate) fn apply_boot(
    selected: &airlock_deployment_core::boot::SelectedGeneration,
    definition: &airlock_deployment_core::configuration::Definition,
    log: &Path,
    apply: bool,
) -> Result<Option<Value>> {
    let helper = definition
        .helper
        .as_ref()
        .context("missing setup topology")?;
    let directory = super::report::helper_directory(log, &definition.name, "--backend-only")?;
    let mut session = Session::at(
        Path::new(&helper.topology),
        apply,
        "single",
        "backend-only",
        directory,
    )?;
    let result = reconcile_with(&mut session, &definition.name, None, Some(selected));
    session.finish(&result)?;
    result?;
    Ok(Some(session.result))
}

/// Run native deployment backend reconciliation inside the coordinator, with typed effect gates.
pub(crate) fn apply_deployment(
    authority: &crate::coordinator::approval::DurableAuthorization,
    record: &crate::model::Record,
    index: usize,
    definition: &airlock_deployment_core::configuration::Definition,
    log: &Path,
) -> Result<Option<Value>> {
    let Some(backend) = authority.storage(record, index)? else {
        return Ok(None);
    };
    ensure!(
        backend.definition().name == definition.name,
        "backend/configuration index differs"
    );
    let helper = definition
        .helper
        .as_ref()
        .context("missing admitted topology")?;
    let directory = super::report::helper_directory(log, &definition.name, "--backend-only")?;
    let mut session = Session::at(
        Path::new(&helper.topology),
        true,
        "single",
        "backend-only",
        directory,
    )?;
    session.staged = Some(
        log.parent()
            .context("missing transaction directory")?
            .join("staged.json"),
    );
    session.result["run_id"] = serde_json::json!(record.id);
    let result = reconcile_with(&mut session, &definition.name, Some(&backend), None);
    session.finish(&result)?;
    result?;
    Ok(Some(session.result))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Required boot identities cannot be replaced by conflicting command-line options.
    #[test]
    fn boot_options_preserve_declared_values() -> Result<()> {
        let mut options = "console=hvc0".into();
        option(&mut options, "init", "/sbin/init")?;
        let original = options.clone();
        option(&mut options, "init", "/sbin/init")?;
        assert_eq!(options, original);
        assert!(option(&mut options, "init", "/wrong").is_err());
        let mut duplicate = "init=/sbin/init init=/wrong".into();
        assert!(option(&mut duplicate, "init", "/sbin/init").is_err());
        let mut duplicate = "init=/sbin/init init=/sbin/init".into();
        option(&mut duplicate, "init", "/sbin/init")?;
        Ok(())
    }
}
