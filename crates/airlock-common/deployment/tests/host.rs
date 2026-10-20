//! Host commands retain the admitted artifacts and exact durable execution context.

mod support;

use airlock_deployment_core::{host::*, operation::*, publication::Event};

/// Artifact identities captured from one admitted release and the native controller policy.
fn definition() -> Definition {
    Definition {
        prepared: false,
        host: "/nix/store/host".into(),
        transport: "/nix/store/transport".into(),
        controller: "/nix/store/controller/bin/airlock-dom0-service".into(),
        controller_root: "/nix/store/controller".into(),
        service: "/native/storage-adapter".into(),
    }
}

/// Every host operation selects its own exact artifact and fixed upstream argv.
#[test]
fn host_requests_fix_targets_programs_and_operations() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let host = p.host(definition());
    let gate = support::durable(p, true);
    for (action, program, args) in [
        (
            Action::PinHost,
            "nix-store",
            vec!["--add-root", "/root", "--realise", "/nix/store/host"],
        ),
        (
            Action::PinTransport,
            "nix-store",
            vec!["--add-root", "/root", "--realise", "/nix/store/transport"],
        ),
        (
            Action::PinController,
            "nix-store",
            vec!["--add-root", "/root", "--realise", "/nix/store/controller"],
        ),
        (
            Action::Profile,
            "nix-env",
            vec![
                "-p",
                "/nix/var/nix/profiles/system",
                "--set",
                "/nix/store/host",
            ],
        ),
        (
            Action::Publish,
            "/native/storage-adapter",
            vec!["storage", "host", "publish", "/nix/store/host"],
        ),
        (
            Action::InstallBoot,
            "/native/storage-adapter",
            vec!["storage", "host", "install-boot", "/nix/store/host"],
        ),
        (
            Action::Bind,
            "/native/storage-adapter",
            vec!["storage", "host", "bind-machine", "/nix/store/host"],
        ),
        (
            Action::Select,
            "/native/storage-adapter",
            vec!["storage", "host", "select", "/nix/store/host"],
        ),
        (
            Action::ProtectBoot,
            "/native/storage-adapter",
            vec!["storage", "host", "protect-boot"],
        ),
        (
            Action::SealBoot,
            "/native/storage-adapter",
            vec!["storage", "host", "seal-boot", "--keep-default"],
        ),
        (
            Action::Checkpoint,
            "/native/storage-adapter",
            vec!["storage", "host", "checkpoint-machine"],
        ),
    ] {
        let request = host.request(&gate, &support::context(), action).unwrap();
        assert_eq!(request.program(), program);
        assert_eq!(request.arguments("/root"), args);
        assert_eq!(request.attempt(), "attempt");
    }
    let controller = host
        .request(&gate, &support::context(), Action::InstallController)
        .unwrap();
    assert_eq!(
        controller.target(),
        "/nix/store/controller/bin/airlock-dom0-service"
    );
    assert!(controller.arguments("/root").is_empty());
    assert!(
        host.request(&gate, &support::context(), Action::Validate)
            .is_none()
    );
}

/// An already admitted view cannot acquire publishing authority through a different action selector.
#[test]
fn prepared_host_uses_validation_without_publication() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let mut d = definition();
    d.prepared = true;
    let host = p.host(d);
    let gate = support::durable(p, true);
    assert!(
        host.request(&gate, &support::context(), Action::Publish)
            .is_none()
    );
    let request = host
        .request(&gate, &support::context(), Action::Validate)
        .unwrap();
    assert_eq!(
        request.arguments(""),
        ["storage", "host", "validate", "/nix/store/host"]
    );
}

/// The same request is unavailable before publication, with any substituted identity, or after interruption.
#[test]
fn host_requests_require_unchanged_durable_authority() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let host = p.host(definition());
    let gate = support::durable(p, false);
    assert!(
        host.request(&gate, &support::context(), Action::Select)
            .is_none()
    );
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let mut gate = support::durable(p, true);
    for field in ["attempt", "operation", "authority", "plan"] {
        let mut c = support::context();
        match field {
            "attempt" => c.attempt = AttemptId::new("stale".into()),
            "operation" => c.operation = OperationId::new("other".into()),
            "authority" => c.authority = AuthorityBinding::new("other".into()),
            "plan" => c.plan = PlanBinding::new("other".into()),
            _ => unreachable!(),
        }
        assert!(host.request(&gate, &c, Action::Select).is_none(), "{field}");
    }
    gate.advance(Event::Interrupted);
    assert!(
        host.request(&gate, &support::context(), Action::Select)
            .is_none()
    );
}
