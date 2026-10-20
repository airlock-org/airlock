//! Check that ordinary Cargo's proof erasure retains the gate's execution behavior.

use airlock_deployment_core::publication::{Event, Publication};

#[test]
fn interruption_requires_new_acknowledgments() {
    let mut initial = Publication::new("attempt-1".into(), "intent-1".into());
    initial.advance(Event::IntentSaved);
    assert!(!initial.begin("attempt-1", "intent-1"));
    initial.advance(Event::ApprovalSaved);
    assert!(!initial.permits("attempt-1", "intent-1"));

    let mut recovered = Publication::new("attempt-2".into(), "intent-2".into());
    recovered.advance(Event::IntentSaved);
    recovered.advance(Event::ApprovalSaved);
    assert!(recovered.begin("attempt-2", "intent-2"));
    recovered.advance(Event::Interrupted);
    assert!(!recovered.begin("attempt-2", "intent-2"));
}
