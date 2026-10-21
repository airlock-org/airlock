//! Reversible lifecycle requests preserve checked configuration, UUID, storage and operation identity.

mod support;

use airlock_deployment_core::{
    inventory::PowerState, operation::*, publication::Event, retirement::*,
};

/// Configuration captured before publication from a baseline or compatible restoration target.
fn definition() -> Definition {
    Definition {
        service: "/native/storage".into(),
        baseline: "/nix/store/baseline".into(),
        name: "usr-old".into(),
        configuration: "config".into(),
        system: "/nix/store/system".into(),
    }
}

/// A retained, stopped VM belongs to its original transaction-and-name trash record.
fn receipt() -> Receipt {
    Receipt {
        baseline: "/nix/store/baseline".into(),
        id: "old-attempt-usr-old".into(),
        transaction: "old-attempt".into(),
        name: "usr-old".into(),
        uuid: "uuid".into(),
        configuration: "config".into(),
        resources: "volumes".into(),
        generation: "7".into(),
        system: "/nix/store/system".into(),
        manifest: "/pool/7/manifest.json".into(),
        status: Status::Trashed,
    }
}

/// Named observations agree with the protected trash record.
fn observed() -> Observation {
    Observation {
        name: "usr-old".into(),
        uuid: "uuid".into(),
        resources: "volumes".into(),
        generation: "7".into(),
        system: "/nix/store/system".into(),
        manifest: "/pool/7/manifest.json".into(),
        power: PowerState::classify("Halted"),
    }
}

/// A genuinely reviewed field determines the allowed namespace and operation kind.
fn authority(kind: Kind, executing: bool) -> (RetirementPlan, DurableOperation) {
    let mut effects = support::effects();
    match kind {
        Kind::Retire => effects.retire = vec!["usr-old"],
        Kind::Restore => effects.restore = vec!["usr-old"],
    }
    assert!(support::approved(&effects, vec![]).is_err());
    let plan = support::approved(&effects, vec!["usr-old"]).unwrap();
    assert!(plan.retirement(vec![], kind).is_none());
    let mut wrong = definition();
    wrong.name = "usr-other".into();
    assert!(plan.retirement(vec![wrong], kind).is_none());
    let retirement = plan.retirement(vec![definition()], kind).unwrap();
    (retirement, support::durable(plan, executing))
}

/// Fixed commands never turn reversible retirement/restoration into arbitrary tags, VMs or purge.
#[test]
fn reversible_requests_select_exact_vm_and_retention_commands() {
    for kind in [Kind::Retire, Kind::Restore] {
        let (plan, gate) = authority(kind, true);
        let receipt = receipt();
        let request = plan
            .request(&gate, &support::context(), 0, &receipt, &observed())
            .unwrap();
        assert_eq!(request.target(), "usr-old");
        assert_eq!(request.generation(), "7");
        assert_eq!(request.trash_id(), "old-attempt-usr-old");
        match kind {
            Kind::Retire => {
                assert_eq!(
                    request.tag_arguments(),
                    ["usr-old", "add", "airlock-retired"]
                );
                assert_eq!(
                    request.autostart_arguments().unwrap(),
                    ["usr-old", "autostart", "False"]
                );
                assert_eq!(request.retention_program(), "/native/storage");
                assert_eq!(
                    request.retention_arguments().unwrap(),
                    [
                        "storage",
                        "pool",
                        "set-retention",
                        "--pool-root",
                        "/var/lib/airlock/closure-store",
                        "--domain",
                        "usr-old-runtime-store",
                        "--generation",
                        "7",
                        "--retain",
                        "true"
                    ]
                );
                let pin = plan.pin(&gate, &support::context(), 0, &receipt).unwrap();
                assert_eq!(pin.trash_id(), "old-attempt-usr-old");
                assert_eq!(
                    pin.arguments("/root"),
                    ["--add-root", "/root", "--realise", "/nix/store/baseline"]
                );
            }
            Kind::Restore => {
                assert_eq!(
                    request.tag_arguments(),
                    ["usr-old", "del", "airlock-retired"]
                );
                assert!(request.autostart_arguments().is_none());
                assert!(request.retention_arguments().is_none());
                assert!(plan.pin(&gate, &support::context(), 0, &receipt).is_none());
            }
        }
    }
}

/// Malformed, substituted, running and purge-state evidence fails before a privileged request exists.
#[test]
fn reversible_requests_reject_changed_resources_and_unsafe_states() {
    for kind in [Kind::Retire, Kind::Restore] {
        let (plan, gate) = authority(kind, true);
        for changed in [
            "name",
            "uuid",
            "configuration",
            "resources",
            "id",
            "paused",
            "unknown",
            "purging",
            "purged",
            "restored",
        ] {
            let mut r = receipt();
            let mut o = observed();
            match changed {
                "name" => r.name = "other".into(),
                "uuid" => o.uuid = "recreated".into(),
                "configuration" => r.configuration = "substituted".into(),
                "resources" => o.resources = "new-private-volume".into(),
                "id" => r.id = "old-attempt-other-vm".into(),
                "paused" => o.power = PowerState::classify("Paused"),
                "unknown" => o.power = PowerState::classify("Unknown"),
                "purging" => r.status = Status::Purging,
                "purged" => r.status = Status::Purged,
                "restored" => r.status = Status::Restored,
                _ => unreachable!(),
            }
            assert!(
                plan.request(&gate, &support::context(), 0, &r, &o)
                    .is_none(),
                "{changed}"
            );
        }
    }
}

/// Restoration can install a new root but cannot substitute private storage or VM registration.
#[test]
fn root_generation_checks_preserve_distinct_retire_and_restore_semantics() {
    for kind in [Kind::Retire, Kind::Restore] {
        let (plan, gate) = authority(kind, true);
        let r = receipt();
        let mut o = observed();
        o.generation = "8".into();
        o.system = "/nix/store/updated-system".into();
        o.manifest = "/pool/8/manifest.json".into();
        assert_eq!(
            plan.request(&gate, &support::context(), 0, &r, &o)
                .is_some(),
            matches!(kind, Kind::Restore)
        );
        o.resources = "different-private-volume".into();
        assert!(
            plan.request(&gate, &support::context(), 0, &r, &o)
                .is_none()
        );
    }
}

/// Missing publication, substituted identities and interruption cannot be used as reversible lifecycle authority.
#[test]
fn reversible_requests_require_the_original_executing_gate() {
    for kind in [Kind::Retire, Kind::Restore] {
        let (plan, gate) = authority(kind, false);
        assert!(
            plan.request(&gate, &support::context(), 0, &receipt(), &observed())
                .is_none()
        );
        let (plan, mut gate) = authority(kind, true);
        for field in ["attempt", "operation", "authority", "plan"] {
            let mut context = support::context();
            match field {
                "attempt" => context.attempt = AttemptId::new("stale".into()),
                "operation" => context.operation = OperationId::new("other".into()),
                "authority" => context.authority = AuthorityBinding::new("other".into()),
                "plan" => context.plan = PlanBinding::new("other".into()),
                _ => unreachable!(),
            }
            assert!(
                plan.request(&gate, &context, 0, &receipt(), &observed())
                    .is_none()
            );
            assert!(plan.pin(&gate, &context, 0, &receipt()).is_none());
        }
        gate.advance(Event::Interrupted);
        assert!(
            plan.request(&gate, &support::context(), 0, &receipt(), &observed())
                .is_none()
        );
    }
}
