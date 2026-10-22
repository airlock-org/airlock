//! Exercise production setup aggregation and completion with conflicting and interrupted evidence.

use airlock_deployment_core::setup::{CompletedSetup, Evidence};

/// A successful final application for one invocation and complete effective input.
fn evidence() -> Evidence {
    Evidence {
        run: "123".into(),
        topology: "topology".into(),
        scope: "all".into(),
        passed: true,
        applied: true,
        final_phase: true,
        pending: false,
        unsupported: 0,
    }
}

#[test]
fn aggregation_preserves_blockers_and_refuses_substitution_or_overflow() {
    let mut child = evidence();
    child.pending = true;
    child.unsupported = 2;
    let combined = evidence().merge(&child).unwrap();
    assert!(combined.pending);
    assert_eq!(combined.unsupported, 2);
    assert!(CompletedSetup::check("123", "topology", "all", combined).is_none());

    child.run = "other".into();
    assert!(evidence().merge(&child).is_none());
    child.run = "123".into();
    child.topology = "other".into();
    assert!(evidence().merge(&child).is_none());
    let mut root = evidence();
    root.unsupported = u64::MAX;
    child = evidence();
    child.unsupported = 1;
    assert!(root.merge(&child).is_none());
}

#[test]
fn completion_cannot_promote_preparation_prediction_or_failed_helpers() {
    let complete = CompletedSetup::check("123", "topology", "all", evidence()).unwrap();
    assert!(complete.matches("123", "topology", "all"));
    assert!(!complete.matches("124", "topology", "all"));
    assert!(!complete.matches("123", "other", "all"));
    assert!(!complete.matches("123", "topology", "deferred"));
    let mut child = evidence();
    child.passed = false;
    assert!(
        CompletedSetup::check("123", "topology", "all", evidence().merge(&child).unwrap())
            .is_none()
    );
    let mut child = evidence();
    child.applied = false;
    assert!(
        CompletedSetup::check("123", "topology", "all", evidence().merge(&child).unwrap())
            .is_none()
    );
    let mut prepare = evidence();
    prepare.final_phase = false;
    assert!(
        CompletedSetup::check(
            "123",
            "topology",
            "all",
            prepare.merge(&evidence()).unwrap()
        )
        .is_none()
    );
    assert!(
        CompletedSetup::check(
            "123",
            "topology",
            "all",
            evidence().with_pending(true).with_pending(false)
        )
        .is_none()
    );
}
