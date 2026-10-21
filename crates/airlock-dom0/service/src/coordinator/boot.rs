//! Reuse applied Qubes configuration without treating a cache receipt as authority.
//! Boot still checks saved storage identities and each started guest's readiness.

use super::*;
use airlock_deployment_core::boot::{DomainState, Handoff, Reuse};
use airlock_machine_core::boot::BootInputs;
use airlock_nix_core::identity::LockHash;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

mod snapshot;

#[derive(Serialize, Deserialize)]
struct BootReceipt {
    // Version of the saved configuration optimization receipt.
    schema: String,
    // Hash of setup program, intended topology and selected storage generations.
    identity: LockHash,
    // Hash of shared Qubes configuration and policy.
    shared: LockHash,
    // Per-guest applied configuration and immutable artifact identities.
    domains: BTreeMap<GuestName, LockHash>,
}

struct BootSelection {
    // Setup program from the selected release or current image.
    setup: StoreFile,
    // Complete checked declaration, preserving omitted and explicitly reset settings.
    inputs: BootInputs,
    // Exact staged selections checked against committed generations; not deployment approval.
    handoff: Option<Handoff>,
    // Cache identity for this complete boot selection.
    identity: LockHash,
}

fn configured_path(key: &str, default: &str) -> Result<PathBuf> {
    let value = std::env::var(key).unwrap_or_else(|_| default.into());
    Ok(absolute(&value)?.to_owned())
}

fn runtime() -> Result<PathBuf> {
    configured_path("AIRLOCK_STARTUP_STATE_DIR", "/run/airlock/startup")
}

fn system() -> Result<PathBuf> {
    configured_path("AIRLOCK_SYSTEM_STATE_DIR", "/var/lib/airlock/system")
}

fn public_save(path: &Path, value: &Value) -> Result<()> {
    state::save(path, value)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))?;
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

impl Coordinator {
    fn boot_selection(&self, setup: StoreFile, topology: &Path) -> Result<BootSelection> {
        if let Some(record) = state::optional::<Record>(&self.state.join("latest.json"))? {
            ensure!(
                record.phase.verified(),
                "interrupted deployment; resume or rollback in recovery"
            );
        }
        let selected = self.selection()?;
        let image_boot = selected.is_none();
        let (setup, raw, staged) = if let Some(selected) = selected {
            ensure!(
                selected.profile == state::profile()?,
                "selected deployment belongs to another boot profile"
            );
            let record = self.record(&selected.transaction)?;
            ensure!(
                record.phase.verified() && record.release == selected.release,
                "interrupted or inconsistent deployment selection"
            );
            let release = Release::load(selected.release.as_path())?;
            release.trust()?;
            self.check_recorded_release(&release, &record)?;
            ensure!(
                release.manifest.profile == selected.profile,
                "selected profile differs"
            );
            let staged = if release.manifest.staged_generations.is_empty() {
                None
            } else {
                let path = release.path.join("staged.json");
                ensure!(
                    state::read::<BTreeMap<GuestName, GenerationId>>(&path)?
                        == release.manifest.staged_generations,
                    "selected staged map missing or inconsistent"
                );
                Some(path)
            };
            (release.manifest.setup, release.raw_topology, staged)
        } else {
            (
                setup,
                state::configuration(topology)?,
                self.store().image_staged()?,
            )
        };
        let inputs = BootInputs::new(raw)?;
        let raw = inputs.raw().clone();
        let topology = inputs.topology().clone();
        let mut generations = staged
            .as_ref()
            .map(|path| state::read::<BTreeMap<GuestName, GenerationId>>(path))
            .transpose()?;
        if image_boot {
            let receipt_path = configured_path(
                "AIRLOCK_PREPARED_RUNTIME",
                "/var/lib/airlock/prepared-runtime.json",
            )?;
            let receipt = state::optional::<Value>(&receipt_path)?;
            ensure!(
                receipt.is_some() || !Path::new("/etc/airlock/usb-dev").exists(),
                "prepared USB runtime receipt is missing; use recovery"
            );
            if let Some(receipt) = receipt {
                ensure!(
                    matches!(
                        receipt["schema"].as_str(),
                        Some("airlock.prepared-runtime.v1" | "airlock.prepared-runtime.v2")
                    ),
                    "unsupported prepared runtime receipt"
                );
                // The image receipt is a selection hint, never a successful-validation token.
                // Initial images need not have a host-machine binding yet. All selected
                // generations are independently inspected and compared below.
                let prepared: BTreeMap<GuestName, GenerationId> =
                    if receipt["schema"] == "airlock.prepared-runtime.v2" {
                        ensure!(
                            receipt["boot_handoff"]["schema"] == "airlock.boot-handoff.v1"
                                && receipt["boot_handoff"]["status"] == "passed",
                            "invalid prepared boot handoff record"
                        );
                        serde_json::from_value(receipt["boot_handoff"]["generations"].clone())?
                    } else {
                        topology
                            .domains
                            .iter()
                            .map(|(name, domain)| {
                                let descriptor: Value = state::read(
                                    &Path::new(domain.backend.store.source.as_str())
                                        .join("prepared.json"),
                                )?;
                                ensure!(
                                    descriptor["schema"] == "airlock.closure-store.prepared.v1"
                                        && descriptor["domain"]
                                            == crate::store::runtime_namespace(name.as_str()),
                                    "invalid legacy image descriptor: {name}"
                                );
                                let generation: GenerationId =
                                    serde_json::from_value(descriptor["generation"].clone())?;
                                Ok((name.clone(), generation))
                            })
                            .collect::<Result<_>>()?
                    };
                if let Some(mapping) = &generations {
                    ensure!(
                        prepared == *mapping,
                        "prepared runtime and boot generation map disagree"
                    );
                } else {
                    generations = Some(prepared);
                }
                let mapping = generations
                    .as_ref()
                    .context("prepared image boot generation map is missing")?;
                ensure!(
                    mapping.len() == topology.domains.len()
                        && topology
                            .domains
                            .keys()
                            .all(|name| mapping.contains_key(name)),
                    "prepared image boot generation map is incomplete or contains extra VMs"
                );
            }
        }
        let handoff = generations
            .as_ref()
            .map(|mapping| {
                crate::store::handoff::selected(&topology, mapping, |name, generation| {
                    self.store().describe(name, generation)
                })
            })
            .transpose()?;
        let identity =
            state::digest(&json!({"topology":raw,"setup":setup,"generations":generations}))?
                .parse()?;
        Ok(BootSelection {
            setup,
            inputs,
            handoff,
            identity,
        })
    }

    fn boot_receipt(&self) -> Result<Option<BootReceipt>> {
        let receipt: Option<BootReceipt> =
            state::optional(&self.state.join("boot-configuration.json"))?;
        if let Some(r) = &receipt {
            ensure!(
                r.schema == "airlock.applied-boot-configuration.v1",
                "unsupported applied boot configuration; use recovery"
            );
        }
        Ok(receipt)
    }

    fn boot_cached(&self, selection: &BootSelection, names: &[GuestName]) -> Result<bool> {
        let Some(receipt) = self.boot_receipt()? else {
            return Ok(false);
        };
        let known: Vec<_> = names
            .iter()
            .filter(|n| receipt.domains.contains_key(*n))
            .cloned()
            .collect();
        if known.len() != names.len() || receipt.identity != selection.identity {
            if !known.is_empty() {
                self.boot_snapshot(selection.inputs.topology(), &known)?;
            }
            return Ok(false);
        }
        let (shared, snapshots) = self.boot_snapshot(selection.inputs.topology(), names)?;
        let fresh = snapshots
            .iter()
            .map(|(name, fingerprint)| DomainState {
                domain: name.to_string(),
                fingerprint: fingerprint.to_string(),
            })
            .collect::<Vec<_>>();
        let saved = snapshots
            .keys()
            .map(|name| DomainState {
                domain: name.to_string(),
                fingerprint: receipt
                    .domains
                    .get(name)
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        Ok(Reuse::check(
            selection.identity.to_string(),
            receipt.identity.as_str(),
            shared.as_str(),
            receipt.shared.as_str(),
            &fresh,
            &saved,
        )
        .is_some())
    }

    fn boot_record(&self, selection: &BootSelection, names: &[GuestName]) -> Result<()> {
        let (shared, mut domains) = self.boot_snapshot(selection.inputs.topology(), names)?;
        if let Some(previous) = self.boot_receipt()?
            && previous.identity == selection.identity
            && previous.shared == shared
        {
            for (n, v) in previous.domains {
                domains.entry(n).or_insert(v);
            }
        }
        state::save(
            &self.state.join("boot-configuration.json"),
            &BootReceipt {
                schema: "airlock.applied-boot-configuration.v1".into(),
                identity: selection.identity.clone(),
                shared,
                domains,
            },
        )
    }

    fn gui_domains(&self, topology: &Topology) -> Result<Vec<GuestName>> {
        let gui: GuestName = "sys-gui".parse()?;
        ensure!(
            topology.domains.contains_key(&gui),
            "physical GUI startup requires a declared sys-gui"
        );
        let mut deps = planning::dependencies(topology, false)?;
        if let Some(default) = topology.prefs.get("default_netvm").and_then(Value::as_str) {
            for (name, domain) in &topology.domains {
                if domain.netvm.is_inherited() && default != name.as_str() {
                    let default: GuestName = default.parse()?;
                    if topology.domains.contains_key(&default) {
                        deps.get_mut(name)
                            .context("missing dependencies")?
                            .insert(default);
                    }
                }
            }
        }
        let mut names = BTreeSet::from([gui]);
        loop {
            let mut next = names.clone();
            for n in &names {
                next.extend(
                    deps.get(n)
                        .context("undeclared GUI dependency")?
                        .iter()
                        .cloned(),
                );
            }
            if next == names {
                break;
            }
            names = next;
        }
        Ok(planning::ordered(topology)?
            .into_iter()
            .filter(|n| names.contains(n))
            .collect())
    }

    fn scoped_topology(&self, selection: &BootSelection, names: &[GuestName]) -> Result<Value> {
        let mut raw = selection.inputs.raw().clone();
        raw["domains"]
            .as_object_mut()
            .context("invalid topology domains")?
            .retain(|n, _| names.iter().any(|v| v.as_str() == n));
        if let Some(prefs) = raw.get_mut("prefs").and_then(Value::as_object_mut) {
            prefs.retain(|key, value| {
                ![
                    "clockvm",
                    "default_dispvm",
                    "default_netvm",
                    "default_guivm",
                    "default_audiovm",
                    "default_template",
                    "management_dispvm",
                    "updatevm",
                ]
                .contains(&key.as_str())
                    || value.as_str().is_none_or(|v| {
                        !selection
                            .inputs
                            .topology()
                            .domains
                            .keys()
                            .any(|n| n.as_str() == v)
                            || names.iter().any(|n| n.as_str() == v)
                    })
            });
        }
        let mut kernels = BTreeSet::new();
        let mut templates = BTreeSet::new();
        for name in names {
            let d = &selection.inputs.topology().domains[name];
            kernels.insert(d.backend.kernel.name.clone());
            if let Some(k) = d.prefs.get("kernel").and_then(Value::as_str) {
                kernels.insert(k.into());
            }
            if let Some(t) = d.template.as_deref() {
                templates.insert(t.to_owned());
            }
        }
        if let Some(k) = selection
            .inputs
            .topology()
            .prefs
            .get("default_kernel")
            .and_then(Value::as_str)
        {
            kernels.insert(k.into());
        }
        if let Some(catalog) = raw.get_mut("kernels").and_then(Value::as_object_mut) {
            catalog.retain(|n, _| kernels.contains(n));
        }
        if let Some(catalog) = raw.get_mut("templates").and_then(Value::as_object_mut) {
            catalog.retain(|n, _| templates.contains(n));
        }
        Ok(raw)
    }

    /// Configure the selected boot generation under one machine lock, optionally in GUI/background scopes.
    pub fn boot_setup(
        &self,
        setup: StoreFile,
        args: Vec<OsString>,
        gui: bool,
        background: bool,
    ) -> Result<Value> {
        let _guard = MachineLock::acquire(
            &self.state,
            "Machine startup",
            "Configure VMs from the selected boot generation",
            MachineOperation::Boot,
        )?;
        self.ensure_no_pruning()?;
        let selected = self.boot_selection(setup, Path::new("/etc/airlock/qubes-topology.json"))?;
        let mut env = Vec::new();
        if let Some(handoff) = &selected.handoff {
            let mapping: BTreeMap<_, _> = handoff
                .selections()
                .iter()
                .map(|selection| (selection.domain.as_str(), selection.generation))
                .collect();
            let path = runtime()?.join("checked-generations.json");
            state::save(&path, &mapping)?;
            env.push(("AIRLOCK_STAGED_GENERATIONS".into(), path.into_os_string()));
        }
        if !gui && !background {
            let mut args = args;
            let path = runtime()?.join("topology.json");
            public_save(&path, selected.inputs.raw())?;
            args.extend(["--topology".into(), path.into_os_string()]);
            self.runner.run(
                selected.setup.as_str(),
                &args,
                Duration::from_secs(1800),
                &env,
            )?;
            return Ok(json!({"status":"passed"}));
        }
        ensure!(
            gui != background && args == [OsString::from("--apply")],
            "scoped boot setup accepts only --apply and one scope"
        );
        let started = Instant::now();
        let gui_names = self.gui_domains(selected.inputs.topology())?;
        let names = if gui {
            gui_names.clone()
        } else {
            planning::ordered(selected.inputs.topology())?
                .into_iter()
                .filter(|n| !gui_names.contains(n))
                .collect()
        };
        let runtime = runtime()?;
        let log = configured_path("AIRLOCK_LOG_DIR", "/var/log/airlock")?;
        let reused = self.boot_cached(&selected, &names)?;
        if gui {
            public_save(&runtime.join("topology.json"), selected.inputs.raw())?;
            for unit in [
                "qubes-db-dom0.service",
                "qubesd.service",
                "qubes-qrexec-policy-daemon.service",
            ] {
                self.runner
                    .text("systemctl", &["is-active", "--quiet", unit])?;
            }
            if reused && let Ok(device) = std::env::var("AIRLOCK_SETUP_CONFIG_STATUS_DEVICE") {
                let _ = fs::write(
                    device,
                    "\x1b[H\x1b[J\n  Airlock | Startup\n\n  Starting your desktop…\n",
                );
            }
        } else {
            ensure!(
                self.boot_cached(&selected, &gui_names)?,
                "GUI configuration changed after startup; repair in recovery"
            );
        }
        let mut result = json!({});
        if !reused {
            let path = runtime.join(if gui {
                "gui-topology.json"
            } else {
                "background-topology.json"
            });
            public_save(&path, &self.scoped_topology(&selected, &names)?)?;
            env.push(("AIRLOCK_SETUP_CONFIG_PERSIST_STATE".into(), "0".into()));
            if !gui {
                env.push(("AIRLOCK_SETUP_CONFIG_STATUS_DEVICE".into(), "".into()));
                env.push((
                    "AIRLOCK_GUI_GPU_CHECK_TOPOLOGY".into(),
                    runtime.join("topology.json").into_os_string(),
                ));
            }
            self.runner.run(
                selected.setup.as_str(),
                &["--apply".into(), "--topology".into(), path.into_os_string()],
                Duration::from_secs(1800),
                &env,
            )?;
            result = state::read(&log.join("setup-config.json"))?;
            ensure!(
                result["schema"] == "airlock.setup-config.v1"
                    && result["status"] == "passed"
                    && (result["restart_required"] == false
                        || result["restart_required"] == json!([]))
                    && result["unsupported_domains_count"] == 0,
                "startup configuration left failed or deferred work"
            );
            self.boot_record(&selected, &names)?;
        }
        if gui {
            public_save(&runtime.join("gui-setup-result.json"), &result)?;
            for name in &gui_names {
                if name.as_str() != "sys-gui" {
                    self.runner.run(
                        "airlock-start-qube-ready",
                        &[
                            name.as_str().into(),
                            runtime.join("topology.json").into_os_string(),
                        ],
                        Duration::from_secs(300),
                        &[],
                    )?;
                }
            }
        } else {
            self.boot_record(
                &selected,
                &selected
                    .inputs
                    .topology()
                    .domains
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            )?;
            public_save(
                &system()?.join("current-topology.json"),
                selected.inputs.raw(),
            )?;
        }
        let report = json!({"schema":"airlock.boot-configuration-result.v1","status":"passed","scope":if gui { "gui" } else { "background" },"reused":reused,"domains":names,"elapsed_seconds":started.elapsed().as_secs_f64()});
        public_save(&log.join("boot-configuration.json"), &report)?;
        if background {
            let gui_result: Value = state::read(&runtime.join("gui-setup-result.json"))?;
            let mut combined = json!({"schema":"airlock.setup-config.v1","status":"passed","mode":"apply","scope":"all","reason":"GUI and background configuration are ready","boot_configuration":report,"unsupported_domains_count":0,"restart_required":false});
            for key in [
                "templates_changed",
                "kernels_changed",
                "domains_changed",
                "prefs_changed",
                "features_changed",
                "services_changed",
                "devices_changed",
                "qrexec_policy_changed",
                "root_images_changed",
                "store_volumes_changed",
                "state_volumes_changed",
                "store_views_changed",
                "qubesd_restarts",
            ] {
                combined[key] = json!(
                    gui_result[key]
                        .as_u64()
                        .unwrap_or(0)
                        .checked_add(result[key].as_u64().unwrap_or(0))
                        .context("setup count overflow")?
                );
            }
            public_save(&system()?.join("current-setup.json"), &combined)?;
        }
        Ok(report)
    }

    /// Start only declared autostart guests and dependencies, then verify their running generations.
    pub fn boot_start(&self, topology: &Path, required: &[GuestName]) -> Result<Value> {
        let mut guard = MachineLock::acquire(
            &self.state,
            "Machine startup",
            "Start the declared autostart VMs",
            MachineOperation::Boot,
        )?;
        let setup: StoreFile = fs::canonicalize("/run/current-system/sw/bin/airlock-setup-config")?
            .to_str()
            .context("invalid setup path")?
            .parse()?;
        let selected = self.boot_selection(setup, topology)?;
        self.enforce_trash(selected.inputs.topology())?;
        let running = self.running()?;
        ensure!(
            required
                .iter()
                .all(|n| selected.inputs.topology().domains.contains_key(n) && running.contains(n)),
            "guarded VMs are not declared and running"
        );
        let starts = planning::plan(
            selected.inputs.topology(),
            selected.inputs.topology(),
            &BTreeSet::new(),
            &[],
            false,
        )?
        .start;
        let mut started = Vec::new();
        for name in &starts {
            if !self.running()?.contains(name)
                && self
                    .runner
                    .run(
                        "qvm-start",
                        &["--skip-if-running".into(), name.as_str().into()],
                        Duration::from_secs(180),
                        &[],
                    )
                    .is_ok()
            {
                started.push(name);
            }
            self.wait_guest(
                name,
                &selected.inputs.topology().domains[name]
                    .backend
                    .system
                    .source,
            )?;
        }
        let baseline = self.baseline()?;
        let observed =
            self.verify_topology(&baseline, &starts.iter().chain(required).cloned().collect())?;
        if self.selection()?.is_some() {
            self.verify_locked(&mut guard)?;
        }
        Ok(
            json!({"autostart":starts,"started":started,"observed":observed,"trash_active":self.trash()?.into_iter().filter(|r| r.status.is_active()).map(|r| r.id).collect::<Vec<_>>()}),
        )
    }
}
