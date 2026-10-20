//! Exercise production device selections against real review and durable-publication gates.

mod support;

use airlock_deployment_core::{
    devices::*,
    inventory::PowerState,
    operation::*,
    pci::{Device, Kind, Usage},
};
use support::*;

/// One physical network function with a stable Qubes device identity.
fn hardware(port: &str) -> Device {
    Device {
        port: port.into(),
        identity: "8086:1234::p020000".into(),
        class: 0x020000,
    }
}

/// Declared identity, matching semantics and options remain fixed under the plan.
fn assignment(matching: Matching) -> Assignment {
    Assignment {
        backend: "dom0".into(),
        port: "01_00.0".into(),
        identity: None,
        mode: Mode::Required,
        matching,
        options: vec![OptionValue {
            name: "no-strict-reset".into(),
            value: "True".into(),
        }],
    }
}

/// Root provider configuration whose explicit PCI list controls automatic discovery.
fn definition(assignments: Vec<Assignment>) -> Definition {
    Definition {
        name: "sys-net".into(),
        assignments,
        provides_network: true,
        auto_feature: "".into(),
        netvm: "none".into(),
        excludes: vec![],
    }
}

/// Full named observations, not a caller-supplied authorization result.
fn observation(state: &str) -> Observation {
    Observation {
        name: "sys-net".into(),
        power: PowerState::classify(state),
        hardware: vec![hardware("01_00.0")],
        uses: vec![],
        recovery_routes: vec![],
    }
}

/// Review both the VM scope and the complete classified configuration effect.
fn checked() -> airlock_deployment_core::plan::AuthorizedPlan {
    let mut p = effects();
    p.configure = vec!["sys-net"];
    approved(&p, vec!["sys-net"]).unwrap()
}

/// A structured assignment/attachment row can constrain ownership without granting permission.
fn usage(owner: &str, port: &str) -> Usage {
    Usage {
        owner: owner.into(),
        backend: "dom0".into(),
        port: port.into(),
        identity: "*".into(),
        kind: Kind::Assigned,
        mode: "required".into(),
        options: vec![],
    }
}

#[test]
fn exact_port_device_and_options_are_preserved() {
    for (matching, expected_port, expected_id) in [
        (Matching::Exact, "01_00.0", "8086:1234::p020000"),
        (Matching::Port, "01_00.0", "*"),
        (Matching::Device, "*", "8086:1234::p020000"),
    ] {
        let p = checked();
        let devices = p
            .devices(vec![definition(vec![assignment(matching)])])
            .unwrap();
        let gate = durable(p, true);
        let o = observation("Halted");
        let h = &o.hardware[0];
        let request = devices
            .explicit(&gate, &context(), 0, 0, h, &o)
            .unwrap()
            .permit(&o)
            .unwrap();
        assert_eq!(request.owner(), "sys-net");
        assert_eq!(request.resource(), ("dom0", expected_port, expected_id));
        assert_eq!(request.mode(), "required");
        assert_eq!(request.options()[0].name, "no-strict-reset");
        assert_eq!(request.options()[0].value, "True");
        let mut existing = usage("sys-net", expected_port);
        existing.identity = expected_id.into();
        assert!(request.replacement(&existing).is_some());
        existing.owner = "foreign".into();
        assert!(request.replacement(&existing).is_none());
        existing.owner = "sys-net".into();
        existing.port = "different-port".into();
        assert!(request.replacement(&existing).is_none());
    }
}

#[test]
fn selection_does_not_grant_running_unknown_or_mismatched_mutation() {
    let p = checked();
    let devices = p
        .devices(vec![definition(vec![assignment(Matching::Port)])])
        .unwrap();
    let gate = durable(p, true);
    for state in ["Running", "Paused", "Transient", "unknown", ""] {
        let o = observation(state);
        assert!(
            devices
                .explicit(&gate, &context(), 0, 0, &o.hardware[0], &o)
                .unwrap()
                .permit(&o)
                .is_none()
        );
    }
    let mut o = observation("Halted");
    o.name = "foreign".into();
    assert!(
        devices
            .explicit(&gate, &context(), 0, 0, &o.hardware[0], &o)
            .is_none()
    );
}

#[test]
fn automatic_network_discovery_preserves_exclusions_and_provider_rules() {
    let p = checked();
    let mut d = definition(vec![]);
    d.excludes = vec!["02_00.0".into()];
    let devices = p.devices(vec![d]).unwrap();
    let gate = durable(p, true);
    let mut o = observation("Halted");
    o.recovery_routes = vec!["03_00.0".into()];
    o.hardware
        .extend([hardware("02_00.0"), hardware("03_00.0")]);
    let mut nonnetwork = hardware("04_00.0");
    nonnetwork.class = 0x030000;
    o.hardware.push(nonnetwork);
    assert!(
        devices
            .automatic(&gate, &context(), 0, &o.hardware[0], &o)
            .is_some()
    );
    for i in 1..o.hardware.len() {
        assert!(
            devices
                .automatic(&gate, &context(), 0, &o.hardware[i], &o)
                .is_none()
        );
    }
    let mut d = definition(vec![]);
    d.netvm = "sys-firewall".into();
    assert!(!is_automatic(&d));
    d.netvm.clear();
    d.provides_network = false;
    assert!(!is_automatic(&d));
    d.provides_network = true;
    d.name = "ordinary-app".into();
    assert!(!is_automatic(&d));
    d.auto_feature = "1".into();
    assert!(is_automatic(&d));
    d.assignments.push(assignment(Matching::Port));
    assert!(!is_automatic(&d));
}

#[test]
fn wildcard_identity_checks_every_matching_physical_port() {
    let p = checked();
    let devices = p
        .devices(vec![definition(vec![assignment(Matching::Device)])])
        .unwrap();
    let gate = durable(p, true);
    let mut o = observation("Halted");
    o.hardware.push(hardware("02_00.0"));
    o.uses.push(usage("foreign", "02_00.0"));
    assert!(
        devices
            .explicit(&gate, &context(), 0, 0, &o.hardware[0], &o)
            .is_none()
    );
    o.uses[0].kind = Kind::Attached;
    assert!(
        devices
            .explicit(&gate, &context(), 0, 0, &o.hardware[0], &o)
            .is_none()
    );
    o.uses.clear();
    assert!(
        devices
            .explicit(&gate, &context(), 0, 0, &hardware("unobserved"), &o)
            .is_none()
    );
}

#[test]
fn checked_namespace_durable_gate_and_all_context_identities_are_required() {
    let p = checked();
    assert!(p.devices(vec![]).is_none());
    let mut wrong = definition(vec![]);
    wrong.name = "foreign".into();
    assert!(p.devices(vec![wrong]).is_none());
    let devices = p
        .devices(vec![definition(vec![assignment(Matching::Port)])])
        .unwrap();
    let gate = durable(p, false);
    let o = observation("Halted");
    assert!(
        devices
            .explicit(&gate, &context(), 0, 0, &o.hardware[0], &o)
            .is_none()
    );
    let p = checked();
    let devices = p
        .devices(vec![definition(vec![assignment(Matching::Port)])])
        .unwrap();
    let gate = durable(p, true);
    let mut cases = Vec::new();
    let mut c = context();
    c.attempt = AttemptId::new("wrong".into());
    cases.push(c);
    let mut c = context();
    c.operation = OperationId::new("wrong".into());
    cases.push(c);
    let mut c = context();
    c.authority = AuthorityBinding::new("wrong".into());
    cases.push(c);
    let mut c = context();
    c.plan = PlanBinding::new("wrong".into());
    cases.push(c);
    for c in cases {
        assert!(
            devices
                .explicit(&gate, &c, 0, 0, &o.hardware[0], &o)
                .is_none()
        );
    }
    assert!(
        devices
            .explicit(&gate, &context(), 0, 1, &o.hardware[0], &o)
            .is_none()
    );
    assert!(
        devices
            .explicit(&gate, &context(), 1, 0, &o.hardware[0], &o)
            .is_none()
    );
}
