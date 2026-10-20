//! Exercise exact plan-bound network handoff, complete ownership checks and durable execution.

mod support;

use airlock_deployment_core::{
    operation::{AttemptId, AuthorityBinding, OperationId, PlanBinding},
    pci::*,
};
use support::*;

/// A current network controller; unrelated PCI classes are supplied in individual tests.
fn device(port: &str) -> Device {
    Device {
        port: port.into(),
        identity: "8086:1234::p020000".into(),
        class: 0x020000,
    }
}

/// Persistent or current use retains exact wildcard/device identity.
fn usage(owner: &str, port: &str, kind: Kind) -> Usage {
    Usage {
        owner: owner.into(),
        backend: "dom0".into(),
        port: port.into(),
        identity: "*".into(),
        mode: "required".into(),
        options: vec![],
        kind,
    }
}

/// The genuine review contains both endpoints specifically in the handoff fields.
fn plan() -> airlock_deployment_core::plan::AuthorizedPlan {
    let mut p = effects();
    p.pci_sources = vec!["usr-assistant"];
    p.pci_targets = vec!["sys-net"];
    approved(&p, vec!["usr-assistant", "sys-net"]).unwrap()
}

/// Validate a supported handoff; observations alone cannot execute it.
fn checked(uses: Vec<Usage>, recovery: bool) -> Option<CheckedObservation> {
    Observation {
        devices: vec![device("01_00.0")],
        uses,
    }
    .check("usr-assistant".into(), "sys-net".into(), recovery)
}

#[test]
fn exact_assignment_and_attachments_are_not_interchangeable() {
    let plan = plan();
    let pci = plan.pci_handoff();
    let gate = durable(plan, true);
    let observed = checked(
        vec![
            usage("usr-assistant", "01_00.0", Kind::Assigned),
            usage("usr-assistant", "01_00.0", Kind::Attached),
        ],
        false,
    )
    .unwrap();
    let execution = pci.execution(&gate, &context(), &observed).unwrap();
    let request = execution.request(0).unwrap();
    assert_eq!(request.owner(), "usr-assistant");
    assert_eq!(request.device(), ("dom0", "01_00.0", "*"));
    assert!(execution.request(1).is_none());
    assert!(execution.request(2).is_none());
}

#[test]
fn all_observed_owners_are_checked_before_any_request_exists() {
    for kind in [Kind::Assigned, Kind::Attached] {
        assert!(
            checked(
                vec![
                    usage("usr-assistant", "01_00.0", Kind::Assigned),
                    usage("foreign", "01_00.0", kind)
                ],
                false
            )
            .is_none()
        );
        assert!(
            checked(
                vec![
                    usage("usr-assistant", "01_00.0", Kind::Assigned),
                    usage("sys-net", "01_00.0", kind)
                ],
                false
            )
            .is_none()
        );
    }
    let observation = Observation {
        devices: vec![device("01_00.0"), device("02_00.0")],
        uses: vec![
            usage("usr-assistant", "01_00.0", Kind::Assigned),
            usage("foreign", "02_00.0", Kind::Assigned),
        ],
    };
    assert!(
        observation
            .check("usr-assistant".into(), "sys-net".into(), false)
            .is_none()
    );
    assert!(checked(vec![usage("usr-assistant", "*", Kind::Assigned)], false).is_none());
}

#[test]
fn class_hardware_identity_and_original_source_constrain_selection() {
    let p = plan();
    let pci = p.pci_handoff();
    let gate = durable(p, true);
    let mut nonnetwork = device("02_00.0");
    nonnetwork.class = 0x030000;
    let mut future = usage("usr-assistant", "01_00.0", Kind::Assigned);
    future.identity = "different-device".into();
    let observed = Observation {
        devices: vec![device("01_00.0"), nonnetwork],
        uses: vec![
            usage("sys-net", "01_00.0", Kind::Assigned),
            usage("usr-assistant", "02_00.0", Kind::Assigned),
            future,
        ],
    }
    .check("usr-assistant".into(), "sys-net".into(), false)
    .unwrap();
    let execution = pci.execution(&gate, &context(), &observed).unwrap();
    assert!((0..execution.len()).all(|i| execution.request(i).is_none()));
}

#[test]
fn recovery_no_op_never_relaxes_ownership_or_expands_endpoints() {
    assert!(checked(vec![], false).is_none());
    let observed = checked(vec![], true).unwrap();
    let p = plan();
    let pci = p.pci_handoff();
    let gate = durable(p, true);
    assert!(
        pci.execution(&gate, &context(), &observed)
            .unwrap()
            .is_empty()
    );
    assert!(checked(vec![usage("foreign", "01_00.0", Kind::Attached)], true).is_none());
    assert!(
        Observation {
            devices: vec![],
            uses: vec![]
        }
        .check("usr-assistant".into(), "sys-net".into(), true)
        .is_none()
    );
}

#[test]
fn scope_identity_gate_and_plan_field_cannot_be_substituted() {
    let mut effects = effects();
    effects.pci_sources = vec!["usr-assistant"];
    effects.pci_targets = vec!["sys-net"];
    assert!(approved(&effects, vec!["usr-assistant"]).is_err());
    let observed = checked(
        vec![usage("usr-assistant", "01_00.0", Kind::Assigned)],
        false,
    )
    .unwrap();
    let p = plan();
    let pci = p.pci_handoff();
    let gate = durable(p, false);
    assert!(pci.execution(&gate, &context(), &observed).is_none());
    let p = plan();
    let pci = p.pci_handoff();
    let gate = durable(p, true);
    let mut wrong = context();
    wrong.plan = PlanBinding::new("other-candidate-baseline-plan".into());
    assert!(pci.execution(&gate, &wrong, &observed).is_none());
    let mut wrong = context();
    wrong.attempt = AttemptId::new("other-attempt".into());
    assert!(pci.execution(&gate, &wrong, &observed).is_none());
    let mut wrong = context();
    wrong.operation = OperationId::new("other-operation".into());
    assert!(pci.execution(&gate, &wrong, &observed).is_none());
    let mut wrong = context();
    wrong.authority = AuthorityBinding::new("other-authority".into());
    assert!(pci.execution(&gate, &wrong, &observed).is_none());
    let reverse = Observation {
        devices: vec![device("01_00.0")],
        uses: vec![],
    }
    .check("sys-net".into(), "usr-assistant".into(), true)
    .unwrap();
    assert!(pci.execution(&gate, &context(), &reverse).is_none());
    let empty = approved(&support::effects(), vec!["usr-assistant", "sys-net"]).unwrap();
    assert!(
        empty
            .pci_handoff()
            .execution(&gate, &context(), &observed)
            .is_none()
    );
}
