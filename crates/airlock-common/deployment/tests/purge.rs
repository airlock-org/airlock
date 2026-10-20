//! Explicit trash confirmation, publication and resource identity remain distinct from deployment authority.

use airlock_deployment_core::{
    inventory::PowerState,
    purge::*,
    retirement::{Observation, Receipt, Status},
};

/// A protected original retirement record, not an independently supplied VM name.
fn receipt(status: Status) -> Receipt {
    Receipt {
        id: "42-vm".into(),
        transaction: "42".into(),
        name: "vm".into(),
        uuid: "original-uuid".into(),
        baseline: "/baseline".into(),
        configuration: "configuration-digest".into(),
        resources: "complete-volume-digest".into(),
        system: "/system".into(),
        manifest: "/manifest".into(),
        generation: "7".into(),
        status,
    }
}

/// Full named observations are compared against the protected original identity.
fn observation() -> Observation {
    Observation {
        name: "vm".into(),
        uuid: "original-uuid".into(),
        resources: "complete-volume-digest".into(),
        system: "/system".into(),
        manifest: "/manifest".into(),
        generation: "7".into(),
        power: PowerState::classify("Halted"),
    }
}

#[test]
fn deletion_and_cleanup_keep_the_same_explicitly_confirmed_identity() {
    let confirmed = ConfirmedPurge::new("42-vm", "42-vm", receipt(Status::Trashed)).unwrap();
    let intent = confirmed
        .prepare(&observation(), &["airlock-retired".into()])
        .unwrap();
    let request = intent.published();
    assert_eq!(request.arguments(), ["--force", "vm"]);
    let cleanup = request.removed();
    assert_eq!(cleanup.storage(), ("vm", "7"));
    assert_eq!(cleanup.identity(), "42-vm");
    assert!(confirmed.absent(&[]).is_none());
    let interrupted = ConfirmedPurge::new("42-vm", "42-vm", receipt(Status::Purging)).unwrap();
    assert!(interrupted.absent(&["vm".into()]).is_none());
    let cleanup = interrupted.absent(&["other".into()]).unwrap();
    assert_eq!(cleanup.storage(), ("vm", "7"));
}

#[test]
fn confirmation_canonical_identity_and_lifecycle_cannot_be_substituted() {
    assert!(ConfirmedPurge::new("42-vm", "different", receipt(Status::Trashed)).is_none());
    let mut other = receipt(Status::Trashed);
    other.name = "other".into();
    assert!(ConfirmedPurge::new("42-vm", "42-vm", other).is_none());
    let mut other = receipt(Status::Trashed);
    other.transaction = "43".into();
    assert!(ConfirmedPurge::new("42-vm", "42-vm", other).is_none());
    for status in [
        Status::Planned,
        Status::Retiring,
        Status::Restored,
        Status::Purged,
    ] {
        assert!(ConfirmedPurge::new("42-vm", "42-vm", receipt(status)).is_none());
    }
}

#[test]
fn registered_resource_replacement_active_or_unknown_state_and_missing_tag_refuse_deletion() {
    let confirmed = ConfirmedPurge::new("42-vm", "42-vm", receipt(Status::Purging)).unwrap();
    let tags = ["airlock-retired".into()];
    assert!(confirmed.prepare(&observation(), &[]).is_none());
    for substituted in 0..6 {
        let mut o = observation();
        match substituted {
            0 => o.name = "other".into(),
            1 => o.uuid = "replacement-uuid".into(),
            2 => o.resources = "replacement-volumes".into(),
            3 => o.generation = "8".into(),
            4 => o.system = "/other-system".into(),
            _ => o.manifest = "/other-manifest".into(),
        }
        assert!(confirmed.prepare(&o, &tags).is_none());
    }
    for state in ["Running", "Paused", "Transient", "", "unknown"] {
        let mut o = observation();
        o.power = PowerState::classify(state);
        assert!(confirmed.prepare(&o, &tags).is_none());
    }
}
