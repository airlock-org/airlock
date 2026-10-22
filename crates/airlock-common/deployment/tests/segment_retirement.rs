//! Exact segment permissions depend on the complete trace, lifecycle and current selection.

use airlock_deployment_core::retention::{
    segments::{Candidate, SegmentIndex, State},
    tracing::{Extent, Manifest, Trace},
};

/// Observe one original segment identity; permission is decided separately.
fn candidate(id: &str, state: State) -> Candidate {
    Candidate {
        id: id.into(),
        state,
    }
}

#[test]
fn full_trace_index_preserves_repeated_pins_and_distinct_spelling() {
    let mut trace = Trace::new();
    assert!(trace.include_manifest(&Manifest {
        objects: vec!["000999".into()],
        view: "000999".into(),
        root: Some("000999".into()),
    }));
    assert!(trace.include_extents(&[
        Extent {
            segment: "000001".into(),
            offset: 0,
            length: 512
        },
        Extent {
            segment: "000001".into(),
            offset: 512,
            length: 512
        },
        Extent {
            segment: "000002".into(),
            offset: 0,
            length: 512
        },
        Extent {
            segment: "000001".into(),
            offset: 1024,
            length: 512
        },
    ]));
    let index = SegmentIndex::new("/pool".into(), &trace).unwrap();
    for id in ["000001", "000002"] {
        assert!(index.reclaim(candidate(id, State::Sealed), &None).is_none());
    }
    // Equal numeric values do not identify equal filenames. Other resource
    // categories with the same spelling do not invent a physical-segment pin.
    for id in ["0000001", "000999"] {
        let request = index.reclaim(candidate(id, State::Sealed), &None).unwrap();
        assert_eq!(request.id(), id);
        assert_eq!(request.pool(), "/pool");
    }
}

#[test]
fn open_selected_and_incomplete_targets_cannot_gain_permission() {
    let trace = Trace::new();
    assert!(SegmentIndex::new(String::new(), &trace).is_none());
    let index = SegmentIndex::new("/pool".into(), &trace).unwrap();
    assert!(
        index
            .reclaim(candidate("000001", State::Open), &None)
            .is_none()
    );
    assert!(index.reclaim(candidate("", State::Sealed), &None).is_none());
    assert!(
        index
            .reclaim(candidate("000001", State::Sealed), &Some("000001".into()))
            .is_none()
    );
    assert!(
        index
            .reclaim(candidate("000001", State::Sealed), &Some(String::new()))
            .is_none()
    );
    assert!(
        index
            .reclaim(candidate("000001", State::Sealed), &Some("000002".into()))
            .is_some()
    );
}

#[test]
fn publication_state_must_be_known_and_consistent() {
    assert!(matches!(State::decode("open", false), Some(State::Open)));
    assert!(matches!(State::decode("sealed", true), Some(State::Sealed)));
    for (status, sealed) in [
        ("open", true),
        ("sealed", false),
        ("unknown", true),
        ("", false),
    ] {
        assert!(State::decode(status, sealed).is_none());
    }
}

#[test]
fn later_observations_cannot_replace_or_reopen_the_checked_target() {
    let trace = Trace::new();
    let index = SegmentIndex::new("/original-pool".into(), &trace).unwrap();
    let request = index
        .reclaim(candidate("000001", State::Sealed), &None)
        .unwrap();
    assert!(!request.revalidate(&candidate("000002", State::Sealed), &None));
    assert!(!request.revalidate(&candidate("000001", State::Open), &None));
    assert!(!request.revalidate(&candidate("000001", State::Sealed), &Some("000001".into())));
    assert!(!request.revalidate(&candidate("000001", State::Sealed), &Some(String::new())));
    assert!(request.revalidate(&candidate("000001", State::Sealed), &Some("000002".into())));
    assert_eq!(request.id(), "000001");
    assert_eq!(request.pool(), "/original-pool");
}
