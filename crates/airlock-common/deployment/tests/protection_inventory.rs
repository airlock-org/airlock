//! Complete source groups determine protection independently of traversal and aggregation order.

use airlock_deployment_core::{
    boot::Selection,
    retention::{
        Candidate, Generation, Pin, Protection,
        inventory::{
            EffectiveSystem, GuestPin, JournalSource, PoolSources, ProtectionInventory,
            ReleaseSource, ReleaseState, StoredGeneration,
        },
    },
    storage::staging::persistence::RecordedResolution,
};

/// Exact identity in either guest-name or pool-namespace coordinates, as required by its source.
fn generation(domain: &str, number: u64) -> Generation {
    Generation {
        domain: domain.into(),
        number,
    }
}

/// Independently observed candidate; source requirements cannot be replaced with success flags.
fn candidate(domain: &str, number: u64, system: &str) -> Candidate {
    Candidate {
        generation: generation(domain, number),
        system: system.into(),
    }
}

/// One full original journal selection; only identity is projected into retention authority.
fn saved(domain: &str, number: u64) -> RecordedResolution {
    RecordedResolution::Retained(Box::new(Selection {
        domain: domain.into(),
        generation: number,
        system: "/system".into(),
        root: "/root".into(),
        source: "/closure".into(),
        kernel: "kernel".into(),
        mode: "shared-closure".into(),
        device: "/dev/xvde".into(),
        blob_device: "/dev/xvdf".into(),
        mountpoint: "/nix/store".into(),
        backend: "erofs-objectmap".into(),
    }))
}

/// Define a release whose manifest may pin one guest and leave another on its effective source.
fn release(state: ReleaseState) -> ReleaseSource {
    ReleaseSource {
        state,
        systems: vec![
            EffectiveSystem {
                domain: "pinned".into(),
                system: "/pinned-system".into(),
            },
            EffectiveSystem {
                domain: "unpinned".into(),
                system: "/shared-system".into(),
            },
        ],
        pins: vec![GuestPin {
            guest: "pinned".into(),
            number: 2,
        }],
    }
}

#[test]
fn release_and_journal_requirements_survive_complete_group_interpretation() {
    let releases = vec![release(ReleaseState::Present)];
    let journals = vec![JournalSource {
        state: ReleaseState::Present,
        entries: vec![
            saved("pinned", 7),
            RecordedResolution::Existing("unpinned".into()),
            RecordedResolution::NonNative("foreign".into()),
        ],
    }];
    let inventory = ProtectionInventory::machine(&releases, &journals).unwrap();
    for number in [2, 7] {
        assert!(
            inventory
                .reclaim(candidate("pinned-runtime-store", number, "/anything"))
                .is_none()
        );
    }
    assert!(
        inventory
            .release(
                candidate("unpinned-runtime-store", 9, "/shared-system"),
                Pin::Automatic
            )
            .is_none()
    );
    assert!(
        inventory
            .reclaim(candidate("pinned-runtime-store", 9, "/pinned-system"))
            .is_some()
    );
    assert!(
        inventory
            .reclaim(candidate("different-runtime-store", 2, "/shared-system"))
            .is_some()
    );
    assert!(
        inventory
            .reclaim(candidate("foreign-runtime-store", 1, "/system"))
            .is_some()
    );

    let pruned = ProtectionInventory::machine(
        &[release(ReleaseState::Pruned)],
        &[JournalSource {
            state: ReleaseState::Pruned,
            entries: vec![saved("pinned", 7)],
        }],
    )
    .unwrap();
    assert!(pruned.references().is_empty());
    assert!(
        pruned
            .reclaim(candidate("pinned-runtime-store", 7, "/system"))
            .is_some()
    );
}

/// Complete pool observations before any selected/running/retained filtering.
fn pool() -> PoolSources {
    PoolSources {
        committed: (1..=5)
            .map(|number| StoredGeneration {
                generation: generation("vm", number),
                retained: number == 1,
            })
            .collect(),
        selected: vec![generation("vm", 2)],
        running: vec![generation("vm", 3), generation("missing", 9)],
        recent: vec![generation("vm", 4)],
    }
}

#[test]
fn pool_categories_and_external_references_cannot_replace_one_another() {
    let sources = pool();
    let inventory = ProtectionInventory::pool(&sources).unwrap();
    for number in 1..=4 {
        assert!(
            inventory
                .reclaim(candidate("vm", number, "/system"))
                .is_none()
        );
    }
    assert!(
        inventory
            .reclaim(candidate("other", 2, "/system"))
            .is_some()
    );
    assert!(inventory.reclaim(candidate("vm", 5, "/system")).is_some());
    // A missing running logical record retains its separate physical-pin policy.
    assert!(
        inventory
            .reclaim(candidate("missing", 9, "/system"))
            .is_some()
    );
    let inventory = inventory
        .with_references(vec![Protection::System {
            domain: "vm".into(),
            system: "/system".into(),
        }])
        .unwrap();
    for number in 1..=5 {
        assert!(
            inventory
                .reclaim(candidate("vm", number, "/system"))
                .is_none()
        );
    }
    let inventory = inventory.with_references(vec![]).unwrap();
    assert!(
        inventory
            .reclaim(candidate("vm", 2, "/different"))
            .is_none()
    );
}

#[test]
fn missing_selected_or_recent_records_refuse_inventory_construction() {
    let mut sources = pool();
    sources.selected.push(generation("missing", 9));
    assert!(ProtectionInventory::pool(&sources).is_none());
    let mut sources = pool();
    sources.recent.push(generation("missing", 9));
    assert!(ProtectionInventory::pool(&sources).is_none());
    let mut release = release(ReleaseState::Present);
    release.pins[0].number = 0;
    assert!(ProtectionInventory::machine(&[release], &[]).is_none());
}

#[test]
fn equivalent_input_orders_protect_the_same_candidate_set() {
    let first = pool();
    let mut second = pool();
    second.committed.reverse();
    second.running.reverse();
    let a = ProtectionInventory::pool(&first).unwrap();
    let b = ProtectionInventory::pool(&second).unwrap();
    for number in 1..=7 {
        assert_eq!(
            a.reclaim(candidate("vm", number, "/system")).is_some(),
            b.reclaim(candidate("vm", number, "/system")).is_some()
        );
    }
}

/// Retry histories must not duplicate transported authority or discard distinct coordinates.
#[test]
fn repeated_sources_are_deduplicated_without_losing_distinct_protections() {
    let releases = vec![
        release(ReleaseState::Present),
        release(ReleaseState::Present),
    ];
    let journals = vec![JournalSource {
        state: ReleaseState::Present,
        entries: vec![saved("pinned", 7), saved("pinned", 7)],
    }];
    let inventory = ProtectionInventory::machine(&releases, &journals).unwrap();
    assert_eq!(inventory.references().len(), 3);
    for number in [2, 7] {
        assert!(
            inventory
                .reclaim(candidate("pinned-runtime-store", number, "/system"))
                .is_none()
        );
    }
    assert!(
        inventory
            .reclaim(candidate("unpinned-runtime-store", 9, "/shared-system"))
            .is_none()
    );
    let pool = ProtectionInventory::pool(&pool()).unwrap();
    let combined = pool
        .with_references(vec![
            Protection::Exact(generation("vm", 2)),
            Protection::Exact(generation("vm", 5)),
        ])
        .unwrap();
    assert_eq!(combined.references().len(), 5);
    for number in 1..=5 {
        assert!(
            combined
                .reclaim(candidate("vm", number, "/system"))
                .is_none()
        );
    }
}
