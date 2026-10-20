//! Propose which old releases and retired VM data policy permits removing.
//! These advisory plans do not prune storage or run garbage collection.

use crate::{
    model::{Phase, PruneRecord, Record, ReleasePath, VerifiedGeneration},
    state::{self, Release},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn pruned(root: &Path, release: &ReleasePath) -> Result<bool> {
    use sha2::{Digest, Sha256};
    let key = format!("{:x}", Sha256::digest(release.as_str().as_bytes()));
    let record: Option<PruneRecord> =
        state::optional(&root.join("pruned-releases").join(format!("{key}.json")))?;
    if let Some(r) = record {
        ensure!(r.release == *release, "invalid release pruning record");
        return Ok(true);
    }
    Ok(false)
}

/// Reject malformed verification history before using it to choose recovery points.
pub fn validate_history(history: &BTreeMap<String, VerifiedGeneration>) -> Result<()> {
    for (key, value) in history {
        let e = value;
        ensure!(
            key.len() == 64
                && key
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                && e.first_verified_at.is_finite()
                && e.first_verified_at > 0.0
                && e.first_verified_at <= state::now() + 60.0,
            "invalid verified-generation history"
        );
    }
    Ok(())
}

/// Read retained generations and retired data to propose policy-permitted removals.
///
/// This does not apply the proposal or invoke garbage collection.
pub fn plan(root: &Path) -> Result<Value> {
    let mut history = state::optional::<BTreeMap<String, VerifiedGeneration>>(
        &root.join("maintenance/verified-generations.json"),
    )?
    .unwrap_or_default();

    let mut available = BTreeMap::new();
    for (key, entry) in history {
        if !pruned(root, &entry.release)? {
            available.insert(key, entry);
        }
    }
    history = available;

    let now = state::now();
    for (key, entry) in &history {
        ensure!(
            key.len() == 64
                && key
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                && entry.first_verified_at.is_finite()
                && entry.first_verified_at > 0.0
                && entry.first_verified_at <= now + 60.0,
            "invalid verified-generation history"
        );
    }

    let directory = root.join("transactions");
    if directory.try_exists()? {
        for entry in fs::read_dir(directory)? {
            let p = entry?.path().join("status.json");
            let record: Record = state::read(&p)?;
            if record.phase != Phase::Complete || pruned(root, &record.release)? {
                continue;
            }

            let r = Release::load(record.release.as_path())?;
            if record.host_running.as_ref() != Some(&r.manifest.host) {
                continue;
            }

            let stamp = record
                .generation_verified_at
                .or(record.verified_at)
                .context("complete transaction has no verification timestamp")?;
            ensure!(
                stamp.is_finite() && stamp > 0.0 && stamp <= now + 60.0,
                "invalid verification timestamp"
            );

            let key = state::release_key(&r)?;
            if history
                .get(&key)
                .is_none_or(|e| stamp < e.first_verified_at)
            {
                history.insert(
                    key,
                    VerifiedGeneration {
                        release: record.release,
                        first_verified_at: stamp,
                        extra: BTreeMap::new(),
                    },
                );
            }
        }
    }

    let mut order: Vec<_> = history.keys().cloned().collect();
    order.sort_by(|a, b| {
        history[b]
            .first_verified_at
            .total_cmp(&history[a].first_verified_at)
            .then(b.cmp(a))
    });

    let mut keep: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for key in order.iter().take(2) {
        keep.entry(key.clone()).or_default().push("newest-two");
    }
    if let Some(anchor) = order
        .iter()
        .find(|k| history[*k].first_verified_at <= now - 7.0 * 86400.0)
        .or_else(|| order.last())
    {
        keep.entry(anchor.clone()).or_default().push(
            if history[anchor].first_verified_at <= now - 7.0 * 86400.0 {
                "weekly-anchor"
            } else {
                "oldest-verified-fallback"
            },
        );
        if let Some(successor) = order
            .iter()
            .filter(|k| history[*k].first_verified_at > history[anchor].first_verified_at)
            .min_by(|a, b| {
                history[*a]
                    .first_verified_at
                    .total_cmp(&history[*b].first_verified_at)
                    .then(a.cmp(b))
            })
        {
            keep.entry(successor.clone())
                .or_default()
                .push("aging-weekly-successor");
        }
    }

    let selection: Option<crate::model::Selection> = state::optional(&root.join("selection.json"))?;
    let current = state::host()?;
    let mut retained = Vec::new();
    let mut candidates = Vec::new();
    for (key, entry) in &history {
        let r = Release::load(entry.release.as_path())?;
        let mut reasons = keep.get(key).cloned().unwrap_or_default();
        if r.manifest.host == current {
            reasons.push("running-host");
        }
        if selection
            .as_ref()
            .is_some_and(|s| s.release == entry.release || s.previous == entry.release)
        {
            reasons.push("selected-or-rollback");
        }

        if !reasons.is_empty() {
            retained.push(json!({"key":key,"release":entry.release,"first_verified_at":entry.first_verified_at,"reasons":reasons}));
        } else {
            candidates.push(json!({"key":key,"release":entry.release,"requires_existing_prune_blocker_checks":true}));
        }
    }

    Ok(
        json!({"schema":"airlock.dom0-retention-plan.v1","authority":"planning-only","keep_generations":retained,"prune_candidates":candidates}),
    )
}
