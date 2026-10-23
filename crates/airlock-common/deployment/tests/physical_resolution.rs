//! Physical reclamation preserves every original logical resolution and every retained consumer pin.

use airlock_deployment_core::retention::{
    physical::{Consumer, Inputs, PhysicalProtection, Resolution},
    runtime::{Observation, Phase},
    segments::{Candidate, State},
    tracing::{Extent, Manifest, Resource, Trace},
};

/// Construct a named range with a positive size independent of its containing source.
fn extent(segment: &str) -> Extent {
    Extent {
        segment: segment.into(),
        offset: 512,
        length: 1024,
    }
}

/// Resolve a complete manifest including its shared terminator and separately managed boot disk.
fn fixture() -> (Trace, Inputs) {
    let mut logical = Trace::new();
    assert!(logical.include_manifest(&Manifest {
        objects: vec!["object".into()],
        view: "/pool/views/by-closure/view".into(),
        root: Some("root".into()),
    }));
    let resolutions = vec![
        Resolution {
            resource: Resource::Object("object".into()),
            extents: vec![extent("000001")],
        },
        Resolution {
            resource: Resource::Object(".tar-eof".into()),
            extents: vec![extent("000002")],
        },
        Resolution {
            resource: Resource::View("/pool/views/by-closure/view".into()),
            // A retained view may pin older backing bytes than canonical current metadata.
            extents: vec![extent("000003"), extent("000002")],
        },
        Resolution {
            resource: Resource::Root("root".into()),
            extents: vec![],
        },
    ];
    (
        logical,
        Inputs {
            resolutions,
            consumers: vec![],
        },
    )
}

/// Check the final deletion contract rather than a separately computed diagnostic count.
fn deletable(protection: &PhysicalProtection<'_>, segment: &str) -> bool {
    protection
        .index()
        .unwrap()
        .reclaim(
            Candidate {
                id: segment.into(),
                state: State::Sealed,
            },
            &None,
        )
        .is_some()
}

#[test]
fn complete_resolution_preserves_current_and_old_view_locations_without_invented_pins() {
    let (logical, mut input) = fixture();
    input.consumers.push(Consumer::Running {
        identity: "/pool/running/vm/1/store-data.json".into(),
        extents: vec![extent("000004"), extent("000001")],
    });
    let protection = PhysicalProtection::resolve("/pool".into(), &logical, input).unwrap();
    assert_eq!(protection.pool(), "/pool");
    for segment in ["000001", "000002", "000003", "000004"] {
        assert!(!deletable(&protection, segment));
    }
    assert!(deletable(&protection, "000005"));
    assert!(deletable(&protection, "00001"));
}

#[test]
fn omitted_added_reordered_and_mismatched_resolutions_cannot_create_protection() {
    for defect in [
        "omit",
        "extra",
        "reorder",
        "identity",
        "category",
        "empty",
        "overflow",
        "root-pins",
        "multi-object",
    ] {
        let (logical, mut input) = fixture();
        match defect {
            "omit" => {
                input.resolutions.remove(0);
            }
            "extra" => input.resolutions.push(Resolution {
                resource: Resource::Object("unrequired".into()),
                extents: vec![extent("000005")],
            }),
            "reorder" => input.resolutions.swap(0, 1),
            "identity" => input.resolutions[0].resource = Resource::Object("other".into()),
            "category" => input.resolutions[0].resource = Resource::Root("object".into()),
            "empty" => input.resolutions[2].extents.clear(),
            "overflow" => input.resolutions[0].extents[0].offset = u64::MAX,
            "root-pins" => input.resolutions[3].extents.push(extent("000005")),
            "multi-object" => input.resolutions[0].extents.push(extent("000005")),
            _ => unreachable!(),
        }
        assert!(
            PhysicalProtection::resolve("/pool".into(), &logical, input).is_none(),
            "accepted {defect}"
        );
    }
    let (logical, input) = fixture();
    assert!(PhysicalProtection::resolve(String::new(), &logical, input).is_none());
}

#[test]
fn consumers_keep_unknown_present_and_preparing_pins_and_release_only_exact_absence() {
    for (phase, inventory, retained) in [
        (Phase::Active, None, true),
        (Phase::Retiring, Some(vec!["airlock-mapper".into()]), true),
        (Phase::Preparing, Some(vec![]), true),
        (Phase::Active, Some(vec![]), false),
        (
            Phase::Retiring,
            Some(vec!["airlock-mapper-other".into()]),
            false,
        ),
    ] {
        let (logical, mut input) = fixture();
        input.consumers.push(Consumer::Runtime {
            identity: "airlock-mapper".into(),
            phase,
            observation: Observation::from_inventory("airlock-mapper".into(), inventory),
            extents: vec![extent("000005"), extent("000006")],
        });
        let protection = PhysicalProtection::resolve("/pool".into(), &logical, input).unwrap();
        for segment in ["000005", "000006"] {
            assert_eq!(!deletable(&protection, segment), retained);
        }
    }
}

#[test]
fn consumer_identity_substitution_and_invalid_retained_ranges_refuse_cleanup() {
    for defect in ["identity", "empty", "range"] {
        let (logical, mut input) = fixture();
        input.consumers.push(Consumer::Runtime {
            identity: "airlock-mapper".into(),
            phase: Phase::Active,
            observation: Observation::from_inventory(
                if defect == "identity" {
                    "other"
                } else {
                    "airlock-mapper"
                }
                .into(),
                None,
            ),
            extents: if defect == "empty" {
                vec![]
            } else {
                vec![Extent {
                    segment: "000005".into(),
                    offset: 0,
                    length: if defect == "range" { 0 } else { 512 },
                }]
            },
        });
        assert!(PhysicalProtection::resolve("/pool".into(), &logical, input).is_none());
    }
}

#[test]
fn duplicate_original_requirements_must_each_resolve_and_physical_inputs_are_rejected() {
    let mut logical = Trace::new();
    assert!(logical.include_object("object".into()));
    assert!(logical.include_object("object".into()));
    let input = Inputs {
        resolutions: (0..2)
            .map(|_| Resolution {
                resource: Resource::Object("object".into()),
                extents: vec![extent("000001")],
            })
            .collect(),
        consumers: vec![],
    };
    let protection = PhysicalProtection::resolve("/pool".into(), &logical, input).unwrap();
    assert!(!deletable(&protection, "000001"));

    let mut logical = Trace::new();
    assert!(logical.include_extents(&[extent("000001")]));
    let input = Inputs {
        resolutions: vec![Resolution {
            resource: Resource::Segment("000001".into()),
            extents: vec![extent("000001")],
        }],
        consumers: vec![],
    };
    assert!(PhysicalProtection::resolve("/pool".into(), &logical, input).is_none());
}
