//! User-facing effects derived from the supplied plan, without changing its contents.
use serde_json::Value;
use std::collections::BTreeSet;

pub struct PlannedEffects {
    pub restarting: Vec<String>,
    pub starting: Vec<String>,
    pub stopping: Vec<String>,
    pub reboot: Option<bool>,
}

impl PlannedEffects {
    pub fn from_plan(plan: &Value) -> Self {
        let names = |key| {
            plan.get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        };
        let stop = names("stop");
        let start = names("start");
        Self {
            restarting: stop.intersection(&start).cloned().collect(),
            starting: start.difference(&stop).cloned().collect(),
            stopping: stop.difference(&start).cloned().collect(),
            reboot: plan.get("host_reboot_required").and_then(Value::as_bool),
        }
    }
}
