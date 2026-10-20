//! Exercise the erased verified selection and graph handoff with decoded production inputs.

use airlock_machine_planning::{
    deployment::{Change, Configuration, SeedPlan, running},
    topology::{Declaration, Defaults, InvalidTopology, Reference},
};
use std::collections::BTreeSet;

/// Build declarations with no implicit external providers.
fn configuration(names: &[&str]) -> Configuration {
    Configuration {
        names: names.iter().map(|name| (*name).to_owned()).collect(),
        domains: names
            .iter()
            .map(|name| Declaration {
                name: (*name).into(),
                netvm: Reference::Disabled,
                template: Reference::Disabled,
                guivm: Reference::Omitted,
                audiovm: Reference::Disabled,
                dispvm: Reference::Disabled,
                default_dispvm: Reference::Disabled,
                devices: vec![],
                omit_display: false,
            })
            .collect(),
        defaults: Defaults {
            netvm: Reference::Omitted,
            template: Reference::Omitted,
            guivm: Reference::Omitted,
            audiovm: Reference::Omitted,
            dispvm: Reference::Omitted,
        },
        globals: "unchanged defaults and kernels".into(),
    }
}

/// Retain absence separately from equal and unequal configuration values.
fn change(name: &str, before: Option<&str>, after: Option<&str>) -> Change {
    Change {
        name: name.into(),
        before: before.map(str::to_owned),
        after: after.map(str::to_owned),
        autostart: None,
        reconnect: None,
    }
}

/// Supply proposed canonical tables, leaving all membership and identity checks to production.
fn planner(
    before: Configuration,
    after: Configuration,
    rows: &[Change],
    running: &[&str],
) -> Result<SeedPlan, InvalidTopology> {
    let impact = before
        .names
        .iter()
        .chain(&after.names)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    SeedPlan::new(
        before,
        after,
        impact,
        rows,
        running.iter().map(|name| (*name).to_owned()).collect(),
    )
}

#[test]
fn roots_distinguish_changes_removal_autostart_and_running_reconciliation() {
    let before = configuration(&["changed", "removed", "running", "stopped"]);
    let after = configuration(&["added", "changed", "running", "stopped"]);
    let mut rows = vec![
        change("added", None, Some("new")),
        change("changed", Some("before"), Some("after")),
        change("removed", Some("old"), None),
        change("running", Some("same"), Some("same")),
        change("stopped", Some("same"), Some("same")),
    ];
    rows[1].autostart = Some(true);
    rows[3].autostart = Some(false);
    let plan = planner(before, after, &rows, &["removed", "running", "unmanaged"]).unwrap();
    assert_eq!(plan.changed(), ["added", "changed"]);
    assert_eq!(plan.removed(), ["removed"]);
    assert_eq!(plan.affected(), ["added", "changed", "removed"]);
    assert_eq!(plan.start().unwrap(), ["changed", "running"]);
    assert_eq!(plan.stop().unwrap(), ["removed"]);
    assert!(plan.reconnect().is_empty());
}

#[test]
fn configuration_coverage_and_graph_identity_cannot_silently_disagree() {
    let row = change("client", Some("same"), Some("same"));
    assert!(
        planner(
            configuration(&["client"]),
            configuration(&["client"]),
            &[],
            &[]
        )
        .is_err()
    );
    assert!(planner(configuration(&[]), configuration(&["client"]), &[row], &[]).is_err());
    let rows = [
        change("client", Some("same"), Some("same")),
        change("client", Some("same"), Some("same")),
    ];
    assert!(
        planner(
            configuration(&["client"]),
            configuration(&["client"]),
            &rows,
            &[]
        )
        .is_err()
    );
    let rows = [change("other", Some("same"), Some("same"))];
    assert!(
        planner(
            configuration(&["client"]),
            configuration(&["client"]),
            &rows,
            &[]
        )
        .is_err()
    );
    let rows = [change("client", Some("same"), Some("same"))];
    let mut before = configuration(&["client"]);
    before.names[0] = "other".into();
    assert!(planner(before, configuration(&["client"]), &rows, &[]).is_err());
    let mut before = configuration(&["client"]);
    before.domains[0].omit_display = true;
    assert!(planner(before, configuration(&["client"]), &rows, &[]).is_err());
}

#[test]
fn gui_clients_require_exact_opt_in_unchanged_configuration_and_no_hard_dependency() {
    let mut before = configuration(&["client", "sys-gui"]);
    before.defaults.guivm = Reference::Named("sys-gui".into());
    let mut after = configuration(&["client", "sys-gui"]);
    after.defaults.guivm = Reference::Named("sys-gui".into());
    let mut rows = [
        change("client", Some("same"), Some("same")),
        change("sys-gui", Some("old-gui"), Some("new-gui")),
    ];
    rows[0].reconnect = Some("1".into());
    let plan = planner(before, after, &rows, &["client", "sys-gui"]).unwrap();
    assert_eq!(plan.stop().unwrap(), ["sys-gui"]);
    assert_eq!(plan.reconnect(), ["client"]);

    // Device backends are hard dependencies even when the display relationship is inherited.
    let mut before = configuration(&["client", "sys-gui"]);
    before.defaults.guivm = Reference::Named("sys-gui".into());
    before.domains[0].devices = vec![Reference::Named("sys-gui".into())];
    let mut after = configuration(&["client", "sys-gui"]);
    after.defaults.guivm = Reference::Named("sys-gui".into());
    after.domains[0].devices = vec![Reference::Named("sys-gui".into())];
    let plan = planner(before, after, &rows, &["client", "sys-gui"]).unwrap();
    assert_eq!(plan.stop().unwrap(), ["client", "sys-gui"]);
    assert!(plan.reconnect().is_empty());
}

#[test]
fn equivalent_rows_and_declarations_preserve_seed_sets_and_lifecycle_order() {
    let mut before = configuration(&["client", "provider"]);
    before.domains[0].netvm = Reference::Named("provider".into());
    let mut after = configuration(&["client", "provider"]);
    after.domains[0].netvm = Reference::Named("provider".into());
    let rows = [
        change("client", Some("same"), Some("same")),
        change("provider", Some("old"), Some("new")),
    ];
    let first = planner(before, after, &rows, &["client", "provider"]).unwrap();
    let mut before = configuration(&["client", "provider"]);
    before.domains[0].netvm = Reference::Named("provider".into());
    before.domains.reverse();
    let mut after = configuration(&["client", "provider"]);
    after.domains[0].netvm = Reference::Named("provider".into());
    after.domains.reverse();
    let rows = [
        change("provider", Some("old"), Some("new")),
        change("client", Some("same"), Some("same")),
    ];
    let second = planner(before, after, &rows, &["provider", "client"]).unwrap();
    assert_eq!(first.changed(), second.changed());
    assert_eq!(first.affected(), second.affected());
    assert_eq!(first.start(), second.start());
    assert_eq!(first.stop(), second.stop());
}

#[test]
fn only_stable_known_power_states_supply_deployment_planning_inputs() {
    assert_eq!(running("Running"), Some(true));
    assert_eq!(running("Halted"), Some(false));
    for state in ["Paused", "Suspended", "Transient", "Halting", "Unknown", ""] {
        assert_eq!(running(state), None, "{state}");
    }
}
