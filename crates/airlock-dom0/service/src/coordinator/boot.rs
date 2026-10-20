//! Reuse applied Qubes configuration without treating a cache receipt as authority.
//! Boot still checks saved storage identities and each started guest's readiness.

use super::*;
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
    // Complete intended VM inventory, even during scoped GUI setup.
    topology: Topology,
    // Original JSON preserving omitted and explicitly reset settings.
    raw: Value,
    // Exact admitted generation map supplied to setup, when present.
    staged: Option<PathBuf>,
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
        let (setup, raw, staged) = if let Some(selected) = self.selection()? {
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
            (setup, state::read(topology)?, self.store().image_staged()?)
        };
        let topology: Topology = serde_json::from_value(raw.clone())?;
        topology.validate()?;
        let generations = staged
            .as_ref()
            .map(|p| state::read::<Value>(p))
            .transpose()?
            .unwrap_or(json!({}));
        let identity =
            state::digest(&json!({"topology":raw,"setup":setup,"generations":generations}))?
                .parse()?;
        Ok(BootSelection {
            setup,
            topology,
            raw,
            staged,
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
                self.boot_snapshot(&selection.topology, &known)?;
            }
            return Ok(false);
        }
        let (shared, snapshots) = self.boot_snapshot(&selection.topology, names)?;
        Ok(shared == receipt.shared
            && snapshots
                .iter()
                .all(|(n, v)| receipt.domains.get(n) == Some(v)))
    }

    fn boot_record(&self, selection: &BootSelection, names: &[GuestName]) -> Result<()> {
        let (shared, mut domains) = self.boot_snapshot(&selection.topology, names)?;
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
        let mut raw = selection.raw.clone();
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
                        !selection.topology.domains.keys().any(|n| n.as_str() == v)
                            || names.iter().any(|n| n.as_str() == v)
                    })
            });
        }
        let mut kernels = BTreeSet::new();
        let mut templates = BTreeSet::new();
        for name in names {
            let d = &selection.topology.domains[name];
            kernels.insert(d.backend.kernel.name.clone());
            if let Some(k) = d.prefs.get("kernel").and_then(Value::as_str) {
                kernels.insert(k.into());
            }
            if let Some(t) = d.template.as_deref() {
                templates.insert(t.to_owned());
            }
        }
        if let Some(k) = selection
            .topology
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
        if let Some(path) = &selected.staged {
            env.push(("AIRLOCK_STAGED_GENERATIONS".into(), path.as_os_str().into()));
        }
        if !gui && !background {
            let mut args = args;
            if self.selection()?.is_some() {
                let path = runtime()?.join("topology.json");
                public_save(&path, &selected.raw)?;
                args.extend(["--topology".into(), path.into_os_string()]);
            }
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
        let gui_names = self.gui_domains(&selected.topology)?;
        let names = if gui {
            gui_names.clone()
        } else {
            planning::ordered(&selected.topology)?
                .into_iter()
                .filter(|n| !gui_names.contains(n))
                .collect()
        };
        let runtime = runtime()?;
        let log = configured_path("AIRLOCK_LOG_DIR", "/var/log/airlock")?;
        let reused = self.boot_cached(&selected, &names)?;
        if gui {
            public_save(&runtime.join("topology.json"), &selected.raw)?;
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
                    .topology
                    .domains
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            )?;
            public_save(&system()?.join("current-topology.json"), &selected.raw)?;
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
        self.enforce_trash(&selected.topology)?;
        let running = self.running()?;
        ensure!(
            required
                .iter()
                .all(|n| selected.topology.domains.contains_key(n) && running.contains(n)),
            "guarded VMs are not declared and running"
        );
        let starts = planning::plan(
            &selected.topology,
            &selected.topology,
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
            self.wait_guest(name, &selected.topology.domains[name].backend.system.source)?;
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
