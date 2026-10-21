//! Boot success requires complete same-instance generation and capability evidence.

use airlock_deployment_core::boot::acceptance::{
    AcceptedBoot, GenerationReady, GuestReady, Instance, Requirement, Scope, Service, ServiceReady,
};

/// A running GUI incarnation on this host boot.
fn instance(xid: u32) -> Instance {
    Instance {
        boot: "boot-a".into(),
        domain: "sys-gui".into(),
        xid,
    }
}

/// Desktop and input are independent requirements even when they share a target.
fn required() -> Requirement {
    Requirement {
        domain: "sys-gui".into(),
        system: "/nix/store/selected".into(),
        desktop: true,
        input: true,
        worker: false,
        network: false,
    }
}

/// Successful external generation observation, checked by the production constructor.
fn generation() -> GenerationReady {
    GenerationReady::check(
        "/nix/store/selected",
        "/nix/store/selected".into(),
        instance(7),
        &instance(7),
    )
    .unwrap()
}

/// Successful fixed capability observation bracketed by a running incarnation.
fn service(kind: Service, xid: u32) -> ServiceReady {
    ServiceReady::check(kind, instance(xid), &instance(xid)).unwrap()
}

/// Every readiness request uses the same checked construction as the production collector.
fn guest() -> GuestReady {
    GuestReady::check(
        required(),
        generation(),
        vec![service(Service::Desktop, 7), service(Service::Input, 7)],
    )
    .unwrap()
}

#[test]
fn effective_requirements_preserve_each_installed_capability() {
    let gui = Requirement::new(
        "sys-gui".into(),
        "/nix/store/selected".into(),
        true,
        Some("sys-gui"),
        None,
        false,
    )
    .unwrap();
    assert!(gui.desktop && gui.input && !gui.worker && !gui.network);
    let worker = Requirement::new(
        "sys-ctrl".into(),
        "/nix/store/selected".into(),
        true,
        Some("sys-gui"),
        Some("sys-ctrl"),
        true,
    )
    .unwrap();
    assert!(!worker.desktop && !worker.input && worker.worker && worker.network);
    assert!(
        Requirement::new(
            "other".into(),
            "/nix/store/selected".into(),
            false,
            None,
            Some("sys-ctrl"),
            false
        )
        .is_none()
    );
    assert!(AcceptedBoot::check("boot-a".into(), Scope::Desktop, &[], &[], &[]).is_none());
}

#[test]
fn incomplete_or_mixed_instance_evidence_cannot_establish_guest_readiness() {
    assert!(
        GenerationReady::check(
            "/nix/store/selected",
            "/nix/store/other".into(),
            instance(7),
            &instance(7)
        )
        .is_none()
    );
    assert!(
        GenerationReady::check(
            "/nix/store/selected",
            "/nix/store/selected".into(),
            instance(7),
            &instance(8)
        )
        .is_none()
    );
    assert!(
        GenerationReady::check(
            "/nix/store/selected",
            "/nix/store/selected".into(),
            instance(0),
            &instance(0)
        )
        .is_none()
    );
    assert!(ServiceReady::check(Service::Desktop, instance(7), &instance(8)).is_none());
    assert!(
        GuestReady::check(required(), generation(), vec![service(Service::Desktop, 7)]).is_none()
    );
    assert!(
        GuestReady::check(
            required(),
            generation(),
            vec![service(Service::Desktop, 7), service(Service::Input, 8)]
        )
        .is_none()
    );
    assert!(
        GuestReady::check(
            required(),
            generation(),
            vec![service(Service::Desktop, 7), service(Service::Worker, 7)]
        )
        .is_none()
    );
    let mut network = required();
    network.network = true;
    assert!(
        GuestReady::check(
            network,
            generation(),
            vec![service(Service::Desktop, 7), service(Service::Input, 7)]
        )
        .is_none()
    );
}

#[test]
fn acceptance_requires_the_complete_original_policy_and_fresh_incarnations() {
    let accepted = AcceptedBoot::check(
        "boot-a".into(),
        Scope::Desktop,
        &[required()],
        &[guest()],
        &[instance(7)],
    )
    .unwrap();
    assert!(accepted.permits("boot-a", Scope::Desktop));
    assert!(!accepted.permits("boot-b", Scope::Desktop));
    assert!(!accepted.permits("boot-a", Scope::Complete));
    assert!(
        AcceptedBoot::check("boot-a".into(), Scope::Complete, &[required()], &[], &[]).is_none()
    );
    assert!(
        AcceptedBoot::check(
            "boot-a".into(),
            Scope::Complete,
            &[required()],
            &[guest()],
            &[instance(8)]
        )
        .is_none()
    );
    assert!(
        AcceptedBoot::check(
            "boot-b".into(),
            Scope::Complete,
            &[required()],
            &[guest()],
            &[instance(7)]
        )
        .is_none()
    );
    let mut changed = required();
    changed.worker = true;
    assert!(
        AcceptedBoot::check(
            "boot-a".into(),
            Scope::Complete,
            &[changed],
            &[guest()],
            &[instance(7)]
        )
        .is_none()
    );
}

#[test]
fn worker_and_network_evidence_are_required_independently() {
    let requirement = || Requirement {
        desktop: false,
        input: false,
        worker: true,
        network: true,
        ..required()
    };
    assert!(
        GuestReady::check(
            requirement(),
            generation(),
            vec![service(Service::Worker, 7)]
        )
        .is_none()
    );
    let ready = GuestReady::check(
        requirement(),
        generation(),
        vec![service(Service::Worker, 7), service(Service::Network, 7)],
    )
    .unwrap();
    assert!(
        AcceptedBoot::check(
            "boot-a".into(),
            Scope::Complete,
            &[requirement()],
            &[ready],
            &[instance(7)]
        )
        .is_some()
    );
}
