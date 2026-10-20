//! Assemble immutable local release candidates from independently checked pool receipts.
//! A receipt identifies admitted storage; it does not authorize activation.

use super::*;
use airlock_nix_core::identity::{GitRevision, LockHash};

impl Coordinator {
    pub(super) fn local_candidate(&self, path: &Path) -> Result<PathBuf> {
        let r = Release::load(path)?;
        r.trust()?;
        let manifest: Value = state::read(&r.path.join("release.json"))?;
        self.publish_candidate(&manifest, &r.raw_topology, BTreeMap::new())
    }

    pub(super) fn pin(&self, name: &str, path: &Path) -> Result<()> {
        ensure!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)),
            "invalid root name"
        );
        let roots = self.state.join("roots");
        state::directory(&roots)?;
        self.runner.run(
            "nix-store",
            &[
                "--add-root".into(),
                roots.join(name).into_os_string(),
                "--realise".into(),
                path.as_os_str().into(),
            ],
            Duration::from_secs(600),
            &[],
        )?;
        fs::File::open(roots)?.sync_all()?;
        Ok(())
    }

    pub(super) fn generation_value(
        &self,
        name: &GuestName,
        generation: GenerationId,
    ) -> Result<Value> {
        let value: Value = self.store().describe(name, generation)?;
        let info: GenerationInfo = serde_json::from_value(value.clone())?;
        ensure!(
            info.domain == *name
                && info.generation == generation
                && info.status == CheckStatus::Passed,
            "generation identity differs"
        );
        Ok(value)
    }

    fn receipt_identity(
        &self,
        receipt: &Value,
    ) -> Result<(GuestName, GenerationId, GitRevision, LockHash, Value)> {
        ensure!(
            receipt["schema"] == "airlock.closure-store.admission.v1"
                && receipt["status"] == "passed"
                && receipt["transport"] == "qrexec"
                && receipt["activated"] == false
                && receipt["retained"] == true,
            "receipt is not a retained inactive qrexec admission"
        );
        let name: GuestName = serde_json::from_value(receipt["domain"].clone())?;
        let generation: GenerationId = serde_json::from_value(receipt["generation"].clone())?;
        let revision: GitRevision = serde_json::from_value(receipt["source_revision"].clone())?;
        let lock_hash: LockHash = serde_json::from_value(receipt["lock_hash"].clone())?;
        let actual = self.generation_value(&name, generation)?;
        for key in [
            "domain",
            "generation",
            "manifest",
            "system",
            "root_source",
            "root_artifact",
            "kernel_name",
        ] {
            ensure!(
                !actual[key].is_null() && actual[key] == receipt[key],
                "admission receipt identity mismatch: {key}"
            );
        }
        ensure!(
            actual["store"]["closure_hash"].is_string()
                && actual["store"]["closure_hash"] == receipt["closure_hash"],
            "admission receipt closure mismatch"
        );
        ensure!(
            actual["provenance"]["source_revision"] == receipt["source_revision"]
                && actual["provenance"]["lock_hash"] == receipt["lock_hash"],
            "receipt provenance differs from pool generation"
        );
        Ok((name, generation, revision, lock_hash, actual))
    }

    fn publish_candidate(
        &self,
        manifest: &Value,
        topology: &Value,
        mut files: BTreeMap<&str, Value>,
    ) -> Result<PathBuf> {
        let id = state::digest(&json!({"release":manifest,"topology":topology}))?;
        let parent = self.state.join("candidates");
        state::directory(&parent)?;
        let path = parent.join(&id);
        files.insert("release.json", manifest.clone());
        files.insert("topology.json", topology.clone());
        files.insert("staged.json", manifest["stagedGenerations"].clone());
        if path.try_exists()? {
            state::trusted(&path)?;
            for (name, expected) in &files {
                ensure!(
                    state::read::<Value>(&path.join(name))? == *expected,
                    "existing candidate differs"
                );
            }
        } else {
            let temp = tempfile::Builder::new()
                .prefix(".candidate-")
                .tempdir_in(&parent)?;
            for (name, value) in files {
                state::save(&temp.path().join(name), &value)?;
            }
            let candidate = Release::load(temp.path())?;
            candidate.trust()?;
            fs::rename(temp.path(), &path)?;
            fs::File::open(parent)?.sync_all()?;
        }
        let r = Release::load(&path)?;
        self.pin(
            &format!("pending-{id}"),
            Path::new(
                r.manifest
                    .source_release
                    .as_ref()
                    .map_or(r.manifest.host.as_str(), StorePath::as_str),
            ),
        )?;
        Ok(path)
    }

    /// Bind exact guest receipts and the retained host view into an immutable machine candidate.
    pub fn machine_bind(&self, path: &Path, receipts: Value) -> Result<Value> {
        let _guard = MachineLock::acquire(
            &self.state,
            "Candidate preparation",
            "Bind admitted guest artifacts to their machine release",
            MachineOperation::Candidate,
        )?;
        self.ensure_no_pruning()?;
        let r = Release::load(path)?;
        r.trust()?;
        self.ensure_available(&r)?;
        ensure!(
            r.manifest.guest_transport == Some(GuestTransport::DirectPoolV1),
            "release does not use direct pool admission"
        );
        let receipts = receipts
            .as_object()
            .context("invalid admission receipt map")?;
        let roots: BTreeMap<GuestName, Value> = state::read(&r.path.join("root-artifacts.json"))?;
        ensure!(
            receipts.keys().map(String::as_str).collect::<BTreeSet<_>>()
                == r.topology.domains.keys().map(GuestName::as_str).collect()
                && roots.keys().collect::<BTreeSet<_>>() == r.topology.domains.keys().collect(),
            "admission receipts or root catalog differ from guests"
        );
        let mut manifest: Value = state::read(&r.path.join("release.json"))?;
        manifest["guestRootArtifacts"] = serde_json::to_value(roots)?;
        manifest["guestProvenance"] = json!({});
        manifest["stagedGenerations"] = json!({});
        for (key, receipt) in receipts {
            let (name, generation, revision, lock_hash, info) = self.receipt_identity(receipt)?;
            ensure!(
                key == name.as_str(),
                "admission domain differs from release"
            );
            ensure!(
                (revision == r.manifest.revision && lock_hash == r.manifest.lock_hash)
                    || receipt["generation_reused"] == true,
                "admission provenance differs from release"
            );
            ensure!(
                info["approval"]["approved_by"] == "sys-ctrl",
                "whole-machine admission requires sys-ctrl"
            );
            manifest["guestProvenance"][key] = json!({"revision":revision,"lockHash":lock_hash});
            manifest["stagedGenerations"][key] = json!(generation);
        }
        let mut prepared = Release::load(&r.path)?;
        prepared.manifest = serde_json::from_value(manifest.clone())?;
        for (name, generation) in &prepared.manifest.staged_generations {
            self.validate_generation(&prepared, name, *generation, false)?;
        }
        r.recheck()?;

        let mut host: Value = self.store().publish_host(&r.manifest.host)?;
        let timings = host
            .as_object_mut()
            .context("invalid host publication")?
            .remove("timings")
            .unwrap_or(json!({}));
        manifest["hostPool"] = host;
        manifest["sourceRelease"] = json!(r.path);
        let path = self.publish_candidate(
            &manifest,
            &r.raw_topology,
            BTreeMap::from([("receipts.json", json!(receipts))]),
        )?;
        Ok(json!({"candidate":path,"timings":timings}))
    }

    /// Create a one-guest candidate while retaining all unchanged baseline identities.
    pub fn guest_candidate(&self, receipt: Value) -> Result<PathBuf> {
        let _guard = MachineLock::acquire(
            &self.state,
            "Guest preparation",
            "Bind a guest admission to the deployed machine baseline",
            MachineOperation::Candidate,
        )?;
        self.ensure_no_pruning()?;
        let (name, generation, revision, lock_hash, info) = self.receipt_identity(&receipt)?;
        ensure!(
            info["selected"] == false
                && receipt.get("current_generation") != Some(&json!(generation)),
            "receipt generation is already active"
        );
        let baseline = self.baseline()?;
        baseline.trust()?;
        let before = baseline
            .topology
            .domains
            .get(&name)
            .context("receipt target is not declared")?;
        ensure!(
            json!(before.backend.system.source) != info["system"]
                || json!(before.backend.root.source) != info["root_source"],
            "admitted guest system is already selected"
        );
        let mut topology = baseline.raw_topology.clone();
        let domain = &mut topology["domains"][name.as_str()];
        for (part, key) in [
            ("system", "system"),
            ("root", "root_source"),
            ("store", "closure_source"),
        ] {
            domain["backend"][part]["source"] = info[key].clone();
        }
        if let Some(opts) = domain["prefs"]["kernelopts"].as_str() {
            let mut tokens: Vec<_> = opts.split_whitespace().map(str::to_owned).collect();
            let matches: Vec<_> = tokens
                .iter()
                .filter_map(|s| s.strip_prefix("airlock.system="))
                .collect();
            ensure!(
                matches.is_empty() || matches == [before.backend.system.source.as_str()],
                "baseline guest boot system differs from topology"
            );
            for token in &mut tokens {
                if token.starts_with("airlock.system=") {
                    *token = format!(
                        "airlock.system={}",
                        info["system"].as_str().context("invalid guest system")?
                    );
                }
            }
            domain["prefs"]["kernelopts"] = json!(tokens.join(" "));
        }

        let mut manifest: Value = state::read(&baseline.path.join("release.json"))?;
        manifest
            .as_object_mut()
            .context("invalid baseline manifest")?
            .remove("hostRepair");
        if let Ok(program) = std::env::var("AIRLOCK_DOM0_SERVICE") {
            let _: StoreFile = program.parse()?;
            manifest["coordinator"] = json!(program);
        }
        manifest["revision"] = json!(revision);
        manifest["lockHash"] = json!(lock_hash);
        manifest["baseRelease"] = json!(baseline.path);
        manifest["retireDomains"] = json!([]);
        manifest["privateVolumeCopies"] = json!({});
        for field in ["stagedGenerations", "guestRootArtifacts", "guestProvenance"] {
            if manifest.get(field).is_none() {
                manifest[field] = json!({});
            }
        }
        manifest["stagedGenerations"][name.as_str()] = json!(generation);
        manifest["guestRootArtifacts"][name.as_str()] = info["root_artifact"].clone();
        manifest["guestProvenance"][name.as_str()] =
            json!({"revision":revision,"lockHash":lock_hash});
        manifest["guestAdmission"] =
            json!({"domain":name,"generation":generation,"manifest":info["manifest"]});
        let mut prepared = Release::load(&baseline.path)?;
        prepared.manifest = serde_json::from_value(manifest.clone())?;
        prepared.topology = serde_json::from_value(topology.clone())?;
        prepared.manifest.validate(&prepared.topology)?;
        self.validate_generation(&prepared, &name, generation, false)?;
        self.publish_candidate(
            &manifest,
            &topology,
            BTreeMap::from([("receipt.json", receipt)]),
        )
    }

    pub(super) fn preparation_state(&self) -> Result<String> {
        let live = self.runner.text("nix-store", &["--gc", "--print-live"])?;
        state::digest(
            &json!({"selection":self.selection()?,"latest":state::optional::<Value>(&self.state.join("latest.json"))?,"running":self.running()?,"host_live_paths":live.lines().collect::<BTreeSet<_>>(),"booted_host":state::host()?}),
        )
    }

    /// Revalidate admitted storage and return a mutable-state token for a later apply.
    pub fn prepared_check(&self, path: &Path) -> Result<Value> {
        let _guard = MachineLock::acquire(
            &self.state,
            "Candidate validation",
            "Validate prepared release before activation",
            MachineOperation::Candidate,
        )?;
        let r = Release::load(path)?;
        r.trust()?;
        let plan = self.plan(&r)?;
        self.supported_apply(&r, &plan)?;
        Ok(json!({"plan":plan,"state":self.preparation_state()?}))
    }
}
