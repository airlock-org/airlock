//! Protection decisions bind exact namespaces, generation numbers, sources and retention classes.

use airlock_deployment_core::retention::{
    Candidate, Generation, Pin, ProtectedGenerations, Protection,
};

/// Independently observed candidate in one storage namespace.
fn candidate(domain: &str, number: u64, system: &str) -> Candidate {
    Candidate {
        generation: Generation {
            domain: domain.into(),
            number,
        },
        system: system.into(),
    }
}

/// Original exact-generation evidence, not a latest-generation approximation.
fn exact(domain: &str, number: u64) -> Protection {
    Protection::Exact(Generation {
        domain: domain.into(),
        number,
    })
}

#[test]
fn original_pins_and_sources_are_complete_namespace_scoped_protections() {
    let protected = ProtectedGenerations::new(vec![
        exact("vm", 2),
        Protection::System {
            domain: "other".into(),
            system: "/old-system".into(),
        },
    ])
    .unwrap();
    assert!(
        protected
            .reclaim(candidate("vm", 2, "/same-system"))
            .is_none()
    );
    assert!(
        protected
            .release(candidate("vm", 2, "/same-system"), Pin::Automatic)
            .is_none()
    );
    assert!(
        protected
            .reclaim(candidate("other", 9, "/old-system"))
            .is_none()
    );
    for _ in 0..2 {
        let request = protected
            .reclaim(candidate("other", 2, "/different"))
            .unwrap();
        assert_eq!(request.domain(), "other");
        assert_eq!(request.generation(), 2);
    }
    assert!(
        protected
            .reclaim(candidate("vm", 3, "/same-system"))
            .is_some()
    );
}

#[test]
fn explicit_pins_and_malformed_evidence_never_authorize_release() {
    let protected = ProtectedGenerations::new(vec![]).unwrap();
    assert!(
        protected
            .release(candidate("vm", 1, "/system"), Pin::Explicit)
            .is_none()
    );
    assert!(
        protected
            .release(candidate("vm", 1, "/system"), Pin::Released)
            .is_none()
    );
    let request = protected
        .release(candidate("vm", 1, "/system"), Pin::Automatic)
        .unwrap();
    assert_eq!(request.domain(), "vm");
    assert_eq!(request.generation(), 1);
    assert!(protected.reclaim(candidate("", 1, "/system")).is_none());
    assert!(protected.reclaim(candidate("vm", 0, "/system")).is_none());
    assert!(protected.reclaim(candidate("vm", 1, "")).is_none());
    assert!(ProtectedGenerations::new(vec![exact("vm", 0)]).is_none());
    assert!(
        ProtectedGenerations::new(vec![Protection::System {
            domain: "vm".into(),
            system: "".into()
        }])
        .is_none()
    );
}
