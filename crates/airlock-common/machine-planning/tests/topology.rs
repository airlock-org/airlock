//! Exercise verified topology interpretation and named planning as ordinary production Rust.

use airlock_machine_planning::topology::{
    Declaration, Defaults, NamedGraph, Reference, interpret, valid_guest,
};
use std::collections::BTreeSet;

/// Keep omitted host defaults distinct from explicit per-domain resets.
fn defaults() -> Defaults {
    Defaults {
        netvm: Reference::Omitted,
        template: Reference::Omitted,
        guivm: Reference::Omitted,
        audiovm: Reference::Omitted,
        dispvm: Reference::Omitted,
    }
}

/// Construct a declaration whose properties all inherit unless the test overrides them.
fn domain(name: &str) -> Declaration {
    Declaration {
        name: name.into(),
        netvm: Reference::Omitted,
        template: Reference::Omitted,
        guivm: Reference::Omitted,
        audiovm: Reference::Omitted,
        dispvm: Reference::Omitted,
        default_dispvm: Reference::Omitted,
        devices: vec![],
        omit_display: false,
    }
}

/// Preserve exact spellings at the decoded-reference boundary.
fn named(name: &str) -> Reference {
    Reference::Named(name.into())
}

/// Proposed canonical tables are checked by production, even when assembled by a sorted map.
fn graph(domains: &[Declaration], defaults: &Defaults, extra: &[String]) -> NamedGraph {
    let names: BTreeSet<_> = domains
        .iter()
        .map(|d| d.name.clone())
        .chain(extra.iter().cloned())
        .collect();
    NamedGraph::new(names.into_iter().collect(), domains, defaults, extra).unwrap()
}

/// Compare effective relationship sets without relying on incidental property enumeration.
fn providers(d: &Declaration, defaults: &Defaults) -> BTreeSet<String> {
    interpret(d, defaults).unwrap().into_iter().collect()
}

#[test]
fn effective_settings_preserve_overrides_resets_and_shared_defaults() {
    let mut defaults = defaults();
    defaults.netvm = Reference::Invalid;
    defaults.template = named("tpl-base");
    defaults.guivm = named("sys-gui");
    defaults.audiovm = named("sys-audio");
    defaults.dispvm = named("disp-default");
    let mut home = domain("usr-home");
    home.netvm = named("sys-net");
    home.template = Reference::Disabled;
    home.audiovm = named("");
    home.dispvm = named("disp-explicit");
    home.devices = vec![
        named("sys-usb"),
        named("dom0"),
        named("usr-home"),
        Reference::Omitted,
    ];
    assert_eq!(
        providers(&home, &defaults),
        BTreeSet::from([
            "sys-net".into(),
            "sys-gui".into(),
            "disp-explicit".into(),
            "disp-default".into(),
            "sys-usb".into(),
        ])
    );

    home.netvm = Reference::Omitted;
    assert!(interpret(&home, &defaults).is_err());
    home.netvm = Reference::Disabled;
    home.guivm = Reference::Invalid;
    assert!(interpret(&home, &defaults).is_err());
    home.omit_display = true;
    assert!(!providers(&home, &defaults).contains("sys-gui"));
    home.dispvm = Reference::Invalid;
    assert!(interpret(&home, &defaults).is_err());
}

#[test]
fn canonical_tables_reject_missing_extra_duplicate_and_misindexed_identities() {
    let domains = [domain("sys-net"), domain("usr-home")];
    let defaults = defaults();
    for table in [
        vec!["usr-home", "sys-net"],
        vec!["sys-net"],
        vec!["sys-net", "usr-home", "usr-unjustified"],
        vec!["sys-net", "sys-net", "usr-home"],
    ] {
        assert!(
            NamedGraph::new(
                table.into_iter().map(String::from).collect(),
                &domains,
                &defaults,
                &[]
            )
            .is_err()
        );
    }
    let ambiguous = [domain("usr-home"), domain("usr-home")];
    assert!(NamedGraph::new(vec!["usr-home".into()], &ambiguous, &defaults, &[]).is_err());
    assert!(
        NamedGraph::new(
            vec!["sys-net".into(), "usr-home".into()],
            &domains,
            &defaults,
            &["new-provider".into()]
        )
        .is_err()
    );
    for name in [
        "",
        "dom0",
        "../escape",
        "1guest",
        "guest space",
        "éguest",
        "abcdefghijklmnopqrstuvwxyzabcdef",
    ] {
        assert!(!valid_guest(name));
        assert!(NamedGraph::new(vec![name.into()], &[domain(name)], &defaults, &[]).is_err());
    }
    assert!(valid_guest("abcdefghijklmnopqrstuvwxyzabcde"));
    assert!(valid_guest("VM_01.test-name"));
}

#[test]
fn named_planning_preserves_branching_relationships_and_target_identity() {
    let mut home = domain("usr-home");
    home.netvm = named("sys-net");
    home.guivm = named("sys-gui");
    home.devices = vec![named("sys-external")];
    let mut net = domain("sys-net");
    net.netvm = named("sys-base");
    let mut gui = domain("sys-gui");
    gui.netvm = named("sys-base");
    let domains = [home, net, gui, domain("sys-base"), domain("usr-unrelated")];
    let defaults = defaults();
    let graph = graph(&domains, &defaults, &[]);
    let roots = vec!["usr-home".into()];
    assert_eq!(
        graph.startup(&roots).unwrap(),
        ["sys-base", "sys-gui", "sys-net", "usr-home"]
    );
    assert_eq!(
        graph.expand(&["sys-base".into()], true),
        ["sys-base", "sys-gui", "sys-net", "usr-home"]
    );
    assert_eq!(
        graph
            .shutdown(
                &graph.expand(&roots, false),
                &["sys-base".into(), "usr-home".into()]
            )
            .unwrap(),
        ["usr-home", "sys-base"]
    );
    assert_eq!(
        graph.startup(&["sys-external".into()]).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(graph.startup(&[]).unwrap(), Vec::<String>::new());
    assert_eq!(
        graph.ordered(&["usr-unrelated".into()], false).unwrap(),
        ["usr-unrelated"]
    );
}

#[test]
fn equivalent_declarations_and_duplicate_devices_produce_identical_named_plans() {
    let defaults = defaults();
    let mut home = domain("usr-home");
    home.netvm = named("sys-net");
    home.devices = vec![named("sys-gui"), named("sys-net"), named("sys-gui")];
    let mut domains = vec![home, domain("sys-net"), domain("sys-gui")];
    let first = graph(&domains, &defaults, &[]);
    domains[0].devices.reverse();
    domains.reverse();
    let second = graph(&domains, &defaults, &[]);
    let roots = vec!["usr-home".into()];
    assert_eq!(first.startup(&roots), second.startup(&roots));
    assert_eq!(
        first.expand(&["sys-gui".into()], true),
        second.expand(&["sys-gui".into()], true)
    );
}

#[test]
fn external_impact_seeds_empty_universes_and_cycles_keep_existing_semantics() {
    let defaults = defaults();
    let empty = graph(&[], &defaults, &[]);
    assert_eq!(empty.startup(&[]).unwrap(), Vec::<String>::new());
    assert_eq!(
        empty.expand(&["outside".into()], false),
        Vec::<String>::new()
    );
    let mut home = domain("usr-home");
    home.netvm = named("sys-new");
    let extras = vec!["sys-new".into()];
    let external = graph(&[home], &defaults, &extras);
    assert_eq!(external.expand(&extras, true), ["sys-new", "usr-home"]);
    assert_eq!(
        external.startup(&["usr-home".into()]).unwrap(),
        ["sys-new", "usr-home"]
    );
    let mut a = domain("sys-a");
    a.netvm = named("sys-b");
    let mut b = domain("sys-b");
    b.guivm = named("sys-a");
    let cycle = graph(&[a, b], &defaults, &[]);
    assert!(cycle.startup(&[]).is_none());
    assert_eq!(cycle.expand(&["sys-a".into()], false), ["sys-a", "sys-b"]);
}
