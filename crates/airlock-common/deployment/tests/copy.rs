//! Private replacement requests retain the checked pair, identities and durable gate.

mod support;

use airlock_deployment_core::{copy::*, inventory::PowerState, operation::*, publication::Event};

/// Stable source or destination disk belonging to one registered VM.
fn volume(name: &str) -> Volume {
    Volume {
        name: name.into(),
        uuid: format!("uuid-{name}"),
        pool: "file".into(),
        vid: format!("{name}-private"),
    }
}

/// Original destination may be created only after intent publication.
fn definition() -> Definition {
    Definition {
        source: volume("usr-old"),
        status: Status::Planned,
        target: "usr-new".into(),
        configuration: "config".into(),
        pinned: None,
    }
}

/// Initial protected receipt has never requested a clone.
fn receipt() -> Receipt {
    Receipt {
        source: volume("usr-old"),
        target: "usr-new".into(),
        configuration: "config".into(),
        pinned: None,
        status: Status::Planned,
    }
}

/// Independent stopped disks with sufficient destination capacity.
fn observed() -> Observation {
    Observation {
        source: volume("usr-old"),
        target: volume("usr-new"),
        source_power: PowerState::classify("Halted"),
        target_power: PowerState::classify("Halted"),
        source_size: 4096,
        target_size: 4096,
    }
}

/// Genuine scope checking and publication produce the resource authority used in production.
fn authority(executing: bool) -> (CopyPlan, DurableOperation) {
    let mut effects = support::effects();
    effects.copy_sources = vec!["usr-old"];
    effects.copy_targets = vec!["usr-new"];
    assert!(support::approved(&effects, vec!["usr-new"]).is_err());
    let plan = support::approved(&effects, vec!["usr-old", "usr-new"]).unwrap();
    let copies = plan.copies(vec![definition()]).unwrap();
    let gate = support::durable(plan, executing);
    (copies, gate)
}

/// Pair selection cannot add, omit, reorder or replace checked endpoints.
#[test]
fn copy_definitions_preserve_the_checked_pairs() {
    let mut effects = support::effects();
    effects.copy_sources = vec!["usr-old"];
    effects.copy_targets = vec!["usr-new"];
    let plan = support::approved(&effects, vec!["usr-old", "usr-new"]).unwrap();
    assert!(plan.copies(vec![]).is_none());
    assert!(plan.copies(vec![definition(), definition()]).is_none());
    let mut wrong = definition();
    wrong.target = "usr-old".into();
    assert!(plan.copies(vec![wrong]).is_none());
}

/// Identity and power/capacity checks constrain the actual CloneFrom/CloneTo names.
#[test]
fn copy_requests_require_exact_resources_and_halted_endpoints() {
    let (copies, gate) = authority(true);
    let observation = observed();
    let intent = copies
        .prepare(&gate, &support::context(), 0, &receipt(), &observation)
        .unwrap()
        .unwrap();
    assert_eq!(intent.target().uuid, "uuid-usr-new");
    let request = intent.published();
    assert_eq!(request.source(), "usr-old");
    assert_eq!(request.target(), "usr-new");
    for field in [
        "name",
        "uuid",
        "pool",
        "vid",
        "configuration",
        "pinned",
        "alias",
        "size",
        "paused",
        "unknown",
    ] {
        let mut receipt = receipt();
        let mut observation = observed();
        match field {
            "name" => receipt.target = "usr-other".into(),
            "uuid" => observation.source.uuid = "recreated".into(),
            "pool" => observation.source.pool = "other".into(),
            "vid" => observation.source.vid = "other".into(),
            "configuration" => receipt.configuration = "substituted".into(),
            "pinned" => receipt.pinned = Some(volume("other")),
            "alias" => observation.target.vid = observation.source.vid.clone(),
            "size" => observation.target_size = 4095,
            "paused" => observation.target_power = PowerState::classify("Paused"),
            "unknown" => observation.source_power = PowerState::classify("Unknown"),
            _ => unreachable!(),
        }
        assert!(
            copies
                .prepare(&gate, &support::context(), 0, &receipt, &observation)
                .is_err(),
            "{field}"
        );
    }
}

/// No intent before publication, after interruption, or under a substituted recovery context.
#[test]
fn copy_execution_requires_the_original_durable_context() {
    let observation = observed();
    let (copies, gate) = authority(false);
    assert!(
        copies
            .prepare(&gate, &support::context(), 0, &receipt(), &observation)
            .is_err()
    );
    let (copies, mut gate) = authority(true);
    for field in ["attempt", "operation", "authority", "plan"] {
        let mut c = support::context();
        match field {
            "attempt" => c.attempt = AttemptId::new("old".into()),
            "operation" => c.operation = OperationId::new("other".into()),
            "authority" => c.authority = AuthorityBinding::new("other".into()),
            "plan" => c.plan = PlanBinding::new("other".into()),
            _ => unreachable!(),
        }
        assert!(
            copies
                .prepare(&gate, &c, 0, &receipt(), &observation)
                .is_err()
        );
    }
    gate.advance(Event::Interrupted);
    assert!(
        copies
            .prepare(&gate, &support::context(), 0, &receipt(), &observation)
            .is_err()
    );
}

/// Recovery skips completed copies and refuses uncertainty even when both VMs are currently halted.
#[test]
fn copy_completion_and_uncertainty_never_overwrite_again() {
    let (copies, gate) = authority(true);
    let observation = observed();
    let mut saved = receipt();
    saved.pinned = Some(volume("usr-new"));
    for _ in 0..3 {
        saved.status = Status::Complete;
        assert!(
            copies
                .prepare(&gate, &support::context(), 0, &saved, &observation)
                .unwrap()
                .is_none()
        );
        saved.status = Status::Copying;
        assert!(matches!(
            copies.prepare(&gate, &support::context(), 0, &saved, &observation),
            Err(Denial::Uncertain)
        ));
    }
    saved.status = Status::Complete;
    saved.pinned = None;
    assert!(
        copies
            .prepare(&gate, &support::context(), 0, &saved, &observation)
            .is_err()
    );
}

/// A stale planned receipt cannot regain overwrite authority after preflight saw uncertainty or completion.
#[test]
fn copy_progress_cannot_regress() {
    for status in [Status::Copying, Status::Complete] {
        let mut effects = support::effects();
        effects.copy_sources = vec!["usr-old"];
        effects.copy_targets = vec!["usr-new"];
        let plan = support::approved(&effects, vec!["usr-old", "usr-new"]).unwrap();
        let mut d = definition();
        d.status = status;
        let copies = plan.copies(vec![d]).unwrap();
        let gate = support::durable(plan, true);
        assert!(
            copies
                .prepare(&gate, &support::context(), 0, &receipt(), &observed())
                .is_err()
        );
    }
}

/// A fresh recovery plan cannot silently recapture a replacement source or regress a prior recorded copy.
#[test]
fn recovery_preflight_preserves_original_copy_resources() {
    let original = definition();
    assert!(preserves(&original, &definition()));
    assert!(!recovery(None, &definition()));
    let mut completed = definition();
    completed.status = Status::Complete;
    assert!(recovery(None, &completed));
    let mut changed = definition();
    changed.source.uuid = "replacement".into();
    assert!(!preserves(&original, &changed));
    let mut original = definition();
    original.pinned = Some(volume("usr-new"));
    assert!(!preserves(&original, &definition()));
    original.status = Status::Complete;
    assert!(!preserves(&original, &definition()));
}
