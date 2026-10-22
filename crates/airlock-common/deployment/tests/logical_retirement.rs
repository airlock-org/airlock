//! Logical permissions preserve full identities and categories from the original resource trace.

use airlock_deployment_core::retention::{
    logical::{ResourceIndex, Target},
    tracing::{Extent, Manifest, Sources, Trace},
};

#[test]
fn every_required_object_view_root_and_pending_object_refuses_deletion() {
    let sources = Sources {
        manifests: vec![Manifest {
            objects: vec!["shared".into(), "shared".into()],
            view: "/pool/views/by-closure/view".into(),
            root: Some("root".into()),
        }],
        pending_objects: vec!["pending".into()],
    };
    let trace = Trace::from_sources(&sources).unwrap();
    let index = ResourceIndex::new("/pool".into(), &trace).unwrap();
    for target in [
        Target::ObjectMetadata("shared".into()),
        Target::FragmentMetadata("shared".into()),
        Target::ObjectMetadata(".tar-eof".into()),
        Target::FragmentMetadata(".tar-eof".into()),
        Target::ObjectMetadata("pending".into()),
        Target::FragmentMetadata("pending".into()),
        Target::View("/pool/views/by-closure/view".into()),
        Target::Root("root".into()),
    ] {
        assert!(index.reclaim(target).is_none());
    }
    assert!(ResourceIndex::new(String::new(), &trace).is_none());
    for target in [
        Target::ObjectMetadata(String::new()),
        Target::FragmentMetadata(String::new()),
        Target::View(String::new()),
        Target::Root(String::new()),
    ] {
        assert!(index.reclaim(target).is_none());
    }
}

#[test]
fn namespaces_and_full_spellings_remain_distinct_without_invented_protections() {
    let mut trace = Trace::new();
    assert!(trace.include_object("only-object".into()));
    assert!(trace.include_root("only-root".into()));
    assert!(trace.include_extents(&[Extent {
        segment: "only-segment".into(),
        offset: 0,
        length: 512
    }]));
    let index = ResourceIndex::new("/original-pool".into(), &trace).unwrap();
    for target in [
        Target::ObjectMetadata("only-root".into()),
        Target::FragmentMetadata("only-segment".into()),
        Target::Root("only-object".into()),
        Target::Root("only-roo".into()),
        Target::View("only-object".into()),
    ] {
        let request = index.reclaim(target).unwrap();
        assert_eq!(request.pool(), "/original-pool");
    }
    let request = index
        .reclaim(Target::FragmentMetadata("exact-target".into()))
        .unwrap();
    assert!(matches!(request.target(), Target::FragmentMetadata(name) if name == "exact-target"));
}

#[test]
fn colliding_root_digests_cannot_hide_or_invent_references() {
    // These 64-character hex names collide under the numeric bucket recurrence.
    // Exact full-name comparisons must still distinguish them in either insertion order.
    let first = format!("{prefix}ae0ea", prefix = "a".repeat(59));
    let second = format!("{prefix}ba6ab", prefix = "a".repeat(59));
    for (protected, unprotected) in [(&first, &second), (&second, &first)] {
        let mut trace = Trace::new();
        assert!(trace.include_root(protected.clone()));
        let index = ResourceIndex::new("/pool".into(), &trace).unwrap();
        assert!(index.reclaim(Target::Root(protected.clone())).is_none());
        assert!(index.reclaim(Target::Root(unprotected.clone())).is_some());
    }
    let mut trace = Trace::new();
    for digest in [&first, &second, &first] {
        assert!(trace.include_root(digest.clone()));
    }
    let index = ResourceIndex::new("/pool".into(), &trace).unwrap();
    assert!(index.reclaim(Target::Root(first)).is_none());
    assert!(index.reclaim(Target::Root(second)).is_none());
}
