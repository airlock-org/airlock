//! Compares evaluated VM configuration without predicting live deployment actions.
//! The normalizer owns the topology format; the existing runtime owns activation planning.

// Reuse the runtime identity that excludes dom0 from VM targets.
use airlock_nix_core::identity::GuestName as VmName;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

/// Review operations can fail on filesystem access, JSON decoding or invalid identities.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Reads normalized topology domains into a map keyed by validated VM names.
/// Missing domains, malformed data and invalid Airlock identities are errors.
fn vms(value: &Value) -> Result<BTreeMap<VmName, Value>> {
    let domains = value
        .pointer("/topology/domains")
        .ok_or("missing evaluated topology domains")?;
    Ok(serde_json::from_value(domains.clone())?)
}

/// Appends field differences using escaped JSON Pointer paths.
/// Objects are compared recursively; arrays and other values are compared whole.
/// Presence flags distinguish an omitted field from one explicitly set to null.
fn changes(path: &str, before: &Value, after: &Value, output: &mut Vec<Value>) {
    if before == after {
        return;
    }

    if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
        let keys: std::collections::BTreeSet<_> = left.keys().chain(right.keys()).collect();
        for key in keys {
            let escaped = key.replace('~', "~0").replace('/', "~1");
            let path = format!("{path}/{escaped}");
            match (left.get(key), right.get(key)) {
                (Some(before), Some(after)) => changes(&path, before, after, output),
                (before, after) => output
                    .push(json!({"path": path, "before": before, "after": after,
                    "beforePresent": before.is_some(), "afterPresent": after.is_some()})),
            }
        }
    } else {
        output.push(json!({"path": path, "before": before, "after": after,
            "beforePresent": true, "afterPresent": true}));
    }
}

/// Reads two evaluated topologies and reports added, removed and changed VMs.
/// The result describes configuration differences only; the existing service
/// separately determines staging and lifecycle actions. Input errors are propagated.
pub fn compare_files(before: &Path, after: &Path) -> Result<Value> {
    let before: Value = serde_json::from_slice(&std::fs::read(before)?)?;
    let after: Value = serde_json::from_slice(&std::fs::read(after)?)?;
    let left = vms(&before)?;
    let right = vms(&after)?;

    let mut changed = BTreeMap::new();
    for (name, previous) in &left {
        if let Some(next) = right.get(name) {
            let mut fields = Vec::new();
            changes("", previous, next, &mut fields);
            if !fields.is_empty() {
                changed.insert(name, fields);
            }
        }
    }

    Ok(json!({
        "addedVMs": right.iter()
            .filter(|(name, _)| !left.contains_key(*name))
            .collect::<BTreeMap<_, _>>(),
        "removedVMs": left.keys().filter(|name| !right.contains_key(*name)).collect::<Vec<_>>(),
        "changedVMs": changed,
    }))
}
