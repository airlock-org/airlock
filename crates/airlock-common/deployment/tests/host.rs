//! Host commands retain the admitted artifacts and exact durable execution context.

mod support;

use airlock_deployment_core::{host::*, operation::*, publication::Event};

/// Artifact identities captured from one admitted release and the native controller policy.
fn definition() -> Definition {
    Definition {
        expected: None,
        host: "/nix/store/host".into(),
        transport: "/nix/store/transport".into(),
        controller: "/nix/store/controller/bin/airlock-dom0-service".into(),
        controller_root: "/nix/store/controller".into(),
        service: "/native/storage-adapter".into(),
    }
}

/// A complete storage response names the same immutable system and its admitted generation.
fn observation() -> Observation {
    Observation {
        system: "/nix/store/host".into(),
        generation: "7".into(),
        record: "host-record".into(),
    }
}

/// Every host operation selects its own exact artifact and fixed upstream argv.
#[test]
fn host_requests_fix_targets_programs_and_operations() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let host = p.host(definition());
    let gate = support::durable(p, true);
    let receipt = host
        .request(&gate, &support::context(), Action::Publish)
        .unwrap()
        .checked(observation())
        .unwrap();
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
        let checked = matches!(
            action,
            Action::Profile
                | Action::InstallBoot
                | Action::SealBoot
                | Action::Bind
                | Action::Select
        );
        let request = if checked {
            assert!(host.request(&gate, &support::context(), action).is_none());
            host.request_checked(&gate, &support::context(), action, &receipt)
                .unwrap()
        } else {
            host.request(&gate, &support::context(), action).unwrap()
        };
        assert_eq!(request.program(), program);
        let mut expected = args;
        if checked && !matches!(action, Action::Profile) {
            expected.extend([
                "--expected-system",
                "/nix/store/host",
                "--expected-generation",
                "7",
                "--expected-record",
                "host-record",
            ]);
        }
        assert_eq!(request.arguments("/root"), expected);
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
    d.expected = Some(observation());
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

/// A response cannot introduce another system, generation or record into an admitted candidate.
#[test]
fn checked_host_rejects_invalid_and_substituted_publication_results() {
    for field in ["system", "generation", "record", "zero"] {
        let p = support::approved(&support::effects(), vec![]).unwrap();
        let mut d = definition();
        d.expected = Some(observation());
        let host = p.host(d);
        let gate = support::durable(p, true);
        let mut observed = observation();
        match field {
            "system" => observed.system = "/nix/store/other".into(),
            "generation" => observed.generation = "8".into(),
            "record" => observed.record = "other-record".into(),
            "zero" => observed.generation = "0".into(),
            _ => unreachable!(),
        }
        assert!(
            host.request(&gate, &support::context(), Action::Validate)
                .unwrap()
                .checked(observed)
                .is_none()
        );
    }
}

/// Retrying under substituted context or an interrupted durable operation cannot reuse checked evidence.
#[test]
fn checked_host_generation_preserves_all_execution_identities() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let host = p.host(definition());
    let mut gate = support::durable(p, true);
    let receipt = host
        .request(&gate, &support::context(), Action::Publish)
        .unwrap()
        .checked(observation())
        .unwrap();
    for field in ["attempt", "operation", "authority", "plan"] {
        let mut context = support::context();
        match field {
            "attempt" => context.attempt = AttemptId::new("new-attempt".into()),
            "operation" => context.operation = OperationId::new("other".into()),
            "authority" => context.authority = AuthorityBinding::new("other".into()),
            "plan" => context.plan = PlanBinding::new("other".into()),
            _ => unreachable!(),
        }
        assert!(
            host.request_checked(&gate, &context, Action::Select, &receipt)
                .is_none()
        );
    }
    assert!(
        host.request_checked(&gate, &support::context(), Action::Select, &receipt)
            .is_some()
    );
    gate.advance(Event::Interrupted);
    assert!(
        host.request_checked(&gate, &support::context(), Action::Select, &receipt)
            .is_none()
    );
}

/// Even a valid newly published attempt must revalidate its own host result.
#[test]
fn checked_host_receipt_cannot_cross_valid_execution_attempts() {
    let p = support::approved(&support::effects(), vec![]).unwrap();
    let host = p.host(definition());
    let gate = support::durable(p, true);
    let receipt = host
        .request(&gate, &support::context(), Action::Publish)
        .unwrap()
        .checked(observation())
        .unwrap();

    let p = support::approved(&support::effects(), vec![]).unwrap();
    let retry_host = p.host(definition());
    let c = ExecutionContext {
        attempt: AttemptId::new("retry".into()),
        operation: OperationId::new("retry".into()),
        authority: AuthorityBinding::new("authority".into()),
        plan: PlanBinding::new("complete-plan".into()),
    };
    let operation = p
        .operation(
            AttemptId::new("retry".into()),
            AuthorityBinding::new("authority".into()),
        )
        .unwrap();
    let mut retry = operation
        .publication(
            &c.attempt,
            &c.operation,
            &c.authority,
            &c.plan,
            IntentBinding::new("intent".into()),
        )
        .unwrap();
    retry.advance(Event::IntentSaved);
    retry.advance(Event::ApprovalSaved);
    assert!(retry.begin(&c.attempt, &IntentBinding::new("intent".into())));
    assert!(retry.allows_effect(&c));
    assert!(
        retry_host
            .request_checked(&retry, &c, Action::Select, &receipt)
            .is_none()
    );
    let current = retry_host
        .request(&retry, &c, Action::Publish)
        .unwrap()
        .checked(observation())
        .unwrap();
    assert!(
        retry_host
            .request_checked(&retry, &c, Action::Select, &current)
            .is_some()
    );
}

/// Namespace entry retains every checked field and path; absence remains explicitly administrative.
#[test]
fn isolated_installation_preserves_checked_identity_and_paths() {
    let observed = observation();
    for offline in [true, false] {
        let args = isolated_arguments(
            "/nix/store/host",
            "/service",
            "/pool",
            "/efi",
            offline,
            Some(&observed),
        )
        .unwrap();
        assert_eq!(
            &args[..10],
            [
                "--mount",
                "--propagation",
                "private",
                "/service",
                "storage",
                "host",
                "--pool-root",
                "/pool",
                "--efi-directory",
                "/efi"
            ]
        );
        assert_eq!(
            &args[args.len() - 6..],
            [
                "--expected-system",
                "/nix/store/host",
                "--expected-generation",
                "7",
                "--expected-record",
                "host-record"
            ]
        );
        assert!(!args.iter().any(|arg| arg == "--administrative"));
        let administrative = isolated_arguments(
            "/nix/store/host",
            "/service",
            "/pool",
            "/efi",
            offline,
            None,
        )
        .unwrap();
        assert_eq!(administrative.last().unwrap(), "--administrative");
    }
    assert!(
        isolated_arguments(
            "/nix/store/other",
            "/service",
            "/pool",
            "/efi",
            false,
            Some(&observed)
        )
        .is_none()
    );
}
