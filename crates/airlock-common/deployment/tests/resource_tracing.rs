//! Storage tracing preserves distinct resource identities across repeated source extensions.

use airlock_deployment_core::retention::tracing::{Extent, Manifest, Resource, Trace};
use std::collections::BTreeSet;

/// Compare semantic membership without relying on traversal order or duplicate counts.
fn references(trace: &Trace) -> BTreeSet<(u8, String)> {
    trace
        .references()
        .iter()
        .map(|r| match r {
            Resource::Object(n) => (0, n.clone()),
            Resource::View(n) => (1, n.clone()),
            Resource::Root(n) => (2, n.clone()),
            Resource::Segment(n) => (3, n.clone()),
        })
        .collect()
}

/// A valid manifest deliberately reuses names across otherwise distinct namespaces.
fn manifest() -> Manifest {
    Manifest {
        objects: vec!["shared".into(), "package".into()],
        view: "shared".into(),
        root: Some("shared".into()),
    }
}

#[test]
fn complete_manifest_and_pins_preserve_categories_and_existing_requirements() {
    let mut trace = Trace::new();
    assert!(trace.include_object("pending".into()));
    assert!(trace.include_manifest(&manifest()));
    assert!(trace.include_root("recovery-root".into()));
    assert!(trace.include_extents(&[Extent {
        segment: "shared".into(),
        offset: 512,
        length: 1024
    }]));
    assert_eq!(
        references(&trace),
        BTreeSet::from([
            (0, "pending".into()),
            (0, "shared".into()),
            (0, "package".into()),
            (0, ".tar-eof".into()),
            (1, "shared".into()),
            (2, "shared".into()),
            (2, "recovery-root".into()),
            (3, "shared".into()),
        ])
    );
}

#[test]
fn malformed_sources_refuse_without_partially_extending_the_trace() {
    let mut trace = Trace::new();
    assert!(trace.include_manifest(&manifest()));
    let before = references(&trace);
    let mut bad = manifest();
    bad.objects.push(String::new());
    assert!(!trace.include_manifest(&bad));
    bad.objects.clear();
    assert!(!trace.include_manifest(&bad));
    bad = manifest();
    bad.view.clear();
    assert!(!trace.include_manifest(&bad));
    bad = manifest();
    bad.root = Some(String::new());
    assert!(!trace.include_manifest(&bad));
    assert!(!trace.include_object(String::new()));
    assert!(!trace.include_root(String::new()));
    assert_eq!(references(&trace), before);
}

#[test]
fn invalid_ranges_refuse_all_pins_but_the_maximum_valid_end_is_supported() {
    let mut trace = Trace::new();
    for (segment, offset, length) in [("", 0, 1), ("old", 0, 0), ("old", u64::MAX, 1)] {
        assert!(!trace.include_extents(&[
            Extent {
                segment: "valid-prefix".into(),
                offset: 0,
                length: 512
            },
            Extent {
                segment: segment.into(),
                offset,
                length
            },
        ]));
        assert!(trace.references().is_empty());
    }
    assert!(trace.include_extents(&[Extent {
        segment: "end".into(),
        offset: u64::MAX - 512,
        length: 512
    }]));
    assert_eq!(references(&trace), BTreeSet::from([(3, "end".into())]));
}

#[test]
fn reordering_and_repetition_preserve_the_same_required_set() {
    let mut a = Trace::new();
    let mut b = Trace::new();
    assert!(a.include_manifest(&manifest()));
    assert!(a.include_object("pending".into()));
    assert!(b.include_object("pending".into()));
    let mut reordered = manifest();
    reordered.objects.reverse();
    assert!(b.include_manifest(&reordered));
    assert!(b.include_manifest(&manifest()));
    assert_eq!(references(&a), references(&b));
    let mut store_only = manifest();
    store_only.root = None;
    let mut c = Trace::new();
    assert!(c.include_manifest(&store_only));
    assert!(!references(&c).contains(&(2, "shared".into())));
}

#[test]
fn complete_source_groups_are_traced_together_or_refused() {
    use airlock_deployment_core::retention::tracing::Sources;

    let mut other = manifest();
    other.view = "other-view".into();
    other.root = None;
    let mut sources = Sources {
        manifests: vec![manifest(), other],
        pending_objects: vec!["pending-one".into(), "pending-two".into()],
    };
    let trace = Trace::from_sources(&sources).unwrap();
    assert_eq!(
        references(&trace),
        BTreeSet::from([
            (0, ".tar-eof".into()),
            (0, "shared".into()),
            (0, "package".into()),
            (0, "pending-one".into()),
            (0, "pending-two".into()),
            (1, "shared".into()),
            (1, "other-view".into()),
            (2, "shared".into()),
        ])
    );
    sources.manifests.reverse();
    sources.pending_objects.reverse();
    assert_eq!(
        references(&Trace::from_sources(&sources).unwrap()),
        references(&trace)
    );
    sources.pending_objects.push(String::new());
    assert!(Trace::from_sources(&sources).is_none());
    sources.pending_objects.pop();
    sources.manifests[1].objects.clear();
    assert!(Trace::from_sources(&sources).is_none());
}
