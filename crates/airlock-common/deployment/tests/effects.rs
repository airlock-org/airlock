//! Exercise the same owned authority chain compiled by Cargo and checked by Verus.

use airlock_deployment_core::{
    effects::{GuiPreservation, ShutdownObservation, Shutdowns, Targets},
    inventory::{DomainObservation, PowerState},
    operation::{
        AttemptId, AuthorityBinding, AuthorizedOperation, DurableOperation, IntentBinding,
        OperationId, PlanBinding, PreviousOperation,
    },
    publication::Event,
    review::{Binding, Candidate, Review, Status},
};

/// Protected review and observed candidate with one approved startup target.
fn inputs() -> (Review<'static>, Binding<'static>, Candidate<'static>) {
    (
        Review {
            request: "review",
            revision: "source",
            lock: "lock",
            guest: None,
            baseline: "baseline",
            fingerprint: Some("shown"),
            status: Status::Approved,
            release: None,
            operation: None,
            allowed: vec!["usr-home"],
            allow_reboot: false,
        },
        Binding {
            request: "review",
            release: "candidate",
            revision: "source",
            lock: "lock",
        },
        Candidate {
            release: "candidate",
            revision: "source",
            lock: "lock",
            guest: None,
            baseline: Some("baseline"),
            previous_operation: None,
            effects: vec!["usr-home"],
            may_reboot: false,
        },
    )
}

/// Build checked scope, rejecting any extra startup target before operation allocation.
fn scope() -> Targets {
    let (r, b, c) = inputs();
    Targets::reviewed(&r, &b, &c, vec!["usr-home".into()]).unwrap()
}

/// Empty shutdown authority checked against the same protected review as startup.
fn shutdowns() -> Shutdowns {
    let (r, b, c) = inputs();
    Shutdowns::reviewed(&r, &b, &c, vec![], vec![]).unwrap()
}

/// Select identity through the only constructor that can yield publishable authority.
fn operation(previous: Option<&PreviousOperation>, authority: &str) -> Option<AuthorizedOperation> {
    AuthorizedOperation::new(
        scope(),
        shutdowns(),
        AttemptId::new("attempt".into()),
        AuthorityBinding::new(authority.into()),
        PlanBinding::new("candidate+baseline+plan".into()),
        previous,
        None,
    )
}

#[test]
fn scope_cannot_expand_or_change_review_identity() {
    let (r, b, mut c) = inputs();
    assert!(Targets::reviewed(&r, &b, &c, vec!["sys-net".into()]).is_err());
    // A dependency added to the full plan still needs original approval.
    c.effects.push("sys-net");
    assert!(Targets::reviewed(&r, &b, &c, vec!["usr-home".into()]).is_err());
    c.effects.pop();
    c.effects.clear();
    // Approval may be wider than the particular plan; that does not add planned effects.
    assert!(Targets::reviewed(&r, &b, &c, vec!["usr-home".into()]).is_err());
    c.effects.push("usr-home");
    c.baseline = Some("other-baseline");
    assert!(Targets::reviewed(&r, &b, &c, vec!["usr-home".into()]).is_err());
    c.baseline = Some("baseline");
    c.release = "other-candidate";
    assert!(Targets::reviewed(&r, &b, &c, vec!["usr-home".into()]).is_err());
}

#[test]
fn recovery_selection_is_mandatory_before_publication() {
    let saved = PreviousOperation {
        id: OperationId::new("stable-operation".into()),
        authority: AuthorityBinding::new("original-decision".into()),
    };
    assert!(operation(Some(&saved), "changed-decision").is_none());
    let recovered = operation(Some(&saved), "original-decision").unwrap();
    assert_eq!(recovered.id().as_str(), "stable-operation");
    // A caller cannot substitute its fresh attempt for the selected stable ID.
    assert!(
        recovered
            .publication(
                &AttemptId::new("attempt".into()),
                &OperationId::new("attempt".into()),
                &AuthorityBinding::new("original-decision".into()),
                &PlanBinding::new("candidate+baseline+plan".into()),
                IntentBinding::new("intent".into()),
            )
            .is_none()
    );

    let legacy = OperationId::new("legacy-attempt".into());
    let adopted = AuthorizedOperation::new(
        scope(),
        shutdowns(),
        AttemptId::new("attempt".into()),
        AuthorityBinding::new("decision".into()),
        PlanBinding::new("plan".into()),
        None,
        Some(&legacy),
    )
    .unwrap();
    assert_eq!(adopted.id().as_str(), "legacy-attempt");
}

#[test]
fn requests_require_exact_executing_intent_and_scope() {
    let attempt = AttemptId::new("attempt".into());
    let id = OperationId::new("attempt".into());
    let authority = AuthorityBinding::new("decision".into());
    let plan = PlanBinding::new("candidate+baseline+plan".into());
    let intent = IntentBinding::new("intent".into());
    let mut durable = operation(None, "decision")
        .unwrap()
        .publication(&attempt, &id, &authority, &plan, intent)
        .unwrap();
    assert!(
        durable
            .request(&attempt, &id, &authority, &plan, "usr-home".into())
            .is_none()
    );
    durable.advance(Event::IntentSaved);
    durable.advance(Event::ApprovalSaved);
    assert!(
        durable
            .request(&attempt, &id, &authority, &plan, "usr-home".into())
            .is_none()
    );
    assert!(durable.begin(&attempt, &IntentBinding::new("intent".into())));

    let request = durable
        .request(&attempt, &id, &authority, &plan, "usr-home".into())
        .unwrap();
    assert_eq!(request.target(), "usr-home");
    assert!(
        durable
            .request(&attempt, &id, &authority, &plan, "sys-net".into())
            .is_none()
    );
    assert!(
        durable
            .request(
                &attempt,
                &OperationId::new("other".into()),
                &authority,
                &plan,
                "usr-home".into()
            )
            .is_none()
    );
    assert!(
        durable
            .request(
                &AttemptId::new("retry".into()),
                &id,
                &authority,
                &plan,
                "usr-home".into()
            )
            .is_none()
    );
    assert!(
        durable
            .request(
                &attempt,
                &id,
                &AuthorityBinding::new("changed".into()),
                &plan,
                "usr-home".into()
            )
            .is_none()
    );
    assert!(
        durable
            .request(
                &attempt,
                &id,
                &authority,
                &PlanBinding::new("different-baseline".into()),
                "usr-home".into()
            )
            .is_none()
    );
    durable.advance(Event::Interrupted);
    assert!(
        durable
            .request(&attempt, &id, &authority, &plan, "usr-home".into())
            .is_none()
    );
    assert!(!durable.begin(&attempt, &IntentBinding::new("intent".into())));

    // Raw state-machine events cannot bypass the exact preflight check at begin.
    let mut bypass = operation(None, "decision")
        .unwrap()
        .publication(
            &attempt,
            &id,
            &authority,
            &plan,
            IntentBinding::new("intent".into()),
        )
        .unwrap();
    bypass.advance(Event::IntentSaved);
    bypass.advance(Event::ApprovalSaved);
    bypass.advance(Event::Begin);
    assert!(
        bypass
            .request(&attempt, &id, &authority, &plan, "usr-home".into())
            .is_none()
    );
}

/// Explicit GUI-preservation evidence with one provider and caller-specified expected clients.
fn gui(clients: &[&str]) -> Vec<GuiPreservation> {
    vec![GuiPreservation {
        provider: "sys-gui".into(),
        clients: clients.iter().map(|name| (*name).to_owned()).collect(),
    }]
}

/// Publish the checked shutdown scope without entering execution, preserving gate tests.
fn shutdown_gate(shutdowns: Shutdowns, saved: Option<&PreviousOperation>) -> DurableOperation {
    let operation = AuthorizedOperation::new(
        Targets::policy_exempt(vec![]),
        shutdowns,
        AttemptId::new("attempt".into()),
        AuthorityBinding::new("decision".into()),
        PlanBinding::new("plan".into()),
        saved,
        None,
    )
    .unwrap();
    let id = OperationId::new(operation.id().as_str().to_owned());
    operation
        .publication(
            &AttemptId::new("attempt".into()),
            &id,
            &AuthorityBinding::new("decision".into()),
            &PlanBinding::new("plan".into()),
            IntentBinding::new("intent".into()),
        )
        .unwrap()
}

/// Build raw inventory evidence without bypassing the verified dependency checks.
fn observed_inventory(target: &str, clients: &[&str], hard: &[&str]) -> ShutdownObservation {
    let names: std::collections::BTreeSet<_> = clients.iter().chain(hard).copied().collect();
    let domains = names
        .into_iter()
        .map(|name| DomainObservation {
            name: name.into(),
            power: PowerState::classify("Running"),
            gui_provider: Some(
                if clients.contains(&name) {
                    target
                } else {
                    "none"
                }
                .into(),
            ),
            hard_dependent: hard.contains(&name),
            declared: true,
        })
        .collect();
    ShutdownObservation::new(target.into(), domains)
}

/// A fresh inventory with no active peers of the normal shutdown target.
fn no_dependents() -> ShutdownObservation {
    observed_inventory("usr-home", &[], &[])
}

/// Acknowledge real-adapter barriers and consume readiness for this test attempt.
fn begin_shutdown(gate: &mut DurableOperation) {
    gate.advance(Event::IntentSaved);
    gate.advance(Event::ApprovalSaved);
    assert!(gate.begin(
        &AttemptId::new("attempt".into()),
        &IntentBinding::new("intent".into())
    ));
}

#[test]
fn normal_shutdown_requires_its_own_plan_and_executing_gate() {
    let (r, b, c) = inputs();
    let targets = Shutdowns::reviewed(&r, &b, &c, vec!["usr-home".into()], vec![]).unwrap();
    let mut gate = shutdown_gate(targets, None);
    let attempt = AttemptId::new("attempt".into());
    let id = OperationId::new("attempt".into());
    let authority = AuthorityBinding::new("decision".into());
    let plan = PlanBinding::new("plan".into());
    let observation = no_dependents();
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observation
        )
        .is_none()
    );
    gate.advance(Event::IntentSaved);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observation
        )
        .is_none()
    );
    gate.advance(Event::ApprovalSaved);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observation
        )
        .is_none()
    );
    assert!(gate.begin(&attempt, &IntentBinding::new("intent".into())));
    let request = gate
        .shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observation,
        )
        .unwrap();
    assert!(!request.forced());
    assert_eq!(
        request.arguments(),
        ["--wait", "--timeout", "120", "usr-home"]
    );
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "sys-net".into(),
            &observation
        )
        .is_none()
    );
    let unexpected = observed_inventory("usr-home", &["usr-other"], &[]);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &unexpected
        )
        .is_none()
    );
    gate.advance(Event::Verified);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observation
        )
        .is_none()
    );
}

#[test]
fn forced_gui_shutdown_needs_complete_approved_clients_and_no_hard_dependents() {
    let (mut r, b, mut c) = inputs();
    r.allowed.push("sys-gui");
    c.effects.push("sys-gui");
    // GUI clients are effects, even when the explicit target is within scope.
    assert!(Shutdowns::reviewed(&r, &b, &c, vec!["sys-gui".into()], gui(&["usr-other"])).is_err());
    c.effects = vec!["sys-gui"];
    assert!(Shutdowns::reviewed(&r, &b, &c, vec!["sys-gui".into()], gui(&["usr-home"])).is_err());
    c.effects.push("usr-home");
    assert!(Shutdowns::reviewed(&r, &b, &c, vec![], gui(&["usr-home"])).is_err());
    assert!(
        Shutdowns::reviewed(
            &r,
            &b,
            &c,
            vec!["sys-gui".into(), "usr-home".into()],
            gui(&["usr-home"])
        )
        .is_err()
    );
    assert!(
        Shutdowns::policy_exempt(
            vec!["sys-net".into()],
            vec![GuiPreservation {
                provider: "sys-net".into(),
                clients: vec!["usr-home".into()],
            }]
        )
        .is_none()
    );

    let targets =
        Shutdowns::reviewed(&r, &b, &c, vec!["sys-gui".into()], gui(&["usr-home"])).unwrap();
    let mut gate = shutdown_gate(targets, None);
    begin_shutdown(&mut gate);
    let attempt = AttemptId::new("attempt".into());
    let id = OperationId::new("attempt".into());
    let authority = AuthorityBinding::new("decision".into());
    let plan = PlanBinding::new("plan".into());
    let observed = observed_inventory("sys-gui", &["usr-home"], &[]);
    let request = gate
        .shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "sys-gui".into(),
            &observed,
        )
        .unwrap();
    assert_eq!(
        request.arguments(),
        ["--force", "--wait", "--timeout", "120", "sys-gui"]
    );
    assert!(request.forced());
    // A preserved client is accounted for without gaining authority to shut that client down.
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &no_dependents()
        )
        .is_none()
    );
    let observed = observed_inventory("sys-gui", &["usr-home", "usr-other"], &[]);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "sys-gui".into(),
            &observed
        )
        .is_none()
    );
    let observed = observed_inventory("sys-gui", &["usr-home"], &["usr-home"]);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "sys-gui".into(),
            &observed
        )
        .is_none()
    );
    // Even policy-exempt callers retain target/mode/client constraints.
    let exempt = Shutdowns::policy_exempt(vec!["sys-gui".into()], gui(&["usr-home"])).unwrap();
    let mut exempt = shutdown_gate(exempt, None);
    begin_shutdown(&mut exempt);
    let observed = observed_inventory("sys-gui", &["usr-home"], &[]);
    assert!(
        exempt
            .shutdown(
                &attempt,
                &id,
                &authority,
                &plan,
                "sys-gui".into(),
                &observed
            )
            .is_some()
    );
}

#[test]
fn shutdown_requires_complete_target_bound_inventory() {
    let (mut r, b, mut c) = inputs();
    r.allowed.push("sys-gui");
    c.effects.push("sys-gui");
    let targets =
        Shutdowns::reviewed(&r, &b, &c, vec!["sys-gui".into()], gui(&["usr-home"])).unwrap();
    let mut gate = shutdown_gate(targets, None);
    begin_shutdown(&mut gate);
    let accepted = |observed: &ShutdownObservation| {
        gate.shutdown(
            &AttemptId::new("attempt".into()),
            &OperationId::new("attempt".into()),
            &AuthorityBinding::new("decision".into()),
            &PlanBinding::new("plan".into()),
            "sys-gui".into(),
            observed,
        )
        .is_some()
    };

    let row = |name: &str, state: &str, provider: Option<&str>| DomainObservation {
        name: name.into(),
        power: PowerState::classify(state),
        gui_provider: provider.map(str::to_owned),
        hard_dependent: false,
        declared: true,
    };
    assert!(accepted(&ShutdownObservation::new(
        "sys-gui".into(),
        vec![
            row("usr-home", "Paused", Some("sys-gui")),
            row("usr-other", "Halted", None),
        ]
    )));
    assert!(!accepted(&ShutdownObservation::new(
        "other-provider".into(),
        vec![]
    )));
    // These rows are outside the approved GUI-client set, whatever their active state.
    for state in [
        "Running",
        "Paused",
        "Transient",
        "Halting",
        "Dying",
        "Crashed",
        "Suspended",
    ] {
        assert!(
            !accepted(&ShutdownObservation::new(
                "sys-gui".into(),
                vec![row("usr-other", state, Some("sys-gui")),]
            )),
            "unapproved client in {state}"
        );
    }
    // Omitting a probe cannot hide a client. An unsupported state blocks even with no GuiVM.
    assert!(!accepted(&ShutdownObservation::new(
        "sys-gui".into(),
        vec![row("usr-home", "Paused", None),]
    )));
    assert!(!accepted(&ShutdownObservation::new(
        "sys-gui".into(),
        vec![row("usr-other", "Unknown", Some("none")),]
    )));
}

#[test]
fn shutdown_recovery_and_retry_preserve_all_identity_bindings() {
    let (r, b, c) = inputs();
    let saved = PreviousOperation {
        id: OperationId::new("stable".into()),
        authority: AuthorityBinding::new("decision".into()),
    };
    let mut gate = shutdown_gate(
        Shutdowns::reviewed(&r, &b, &c, vec!["usr-home".into()], vec![]).unwrap(),
        Some(&saved),
    );
    begin_shutdown(&mut gate);
    let attempt = AttemptId::new("attempt".into());
    let id = OperationId::new("stable".into());
    let authority = AuthorityBinding::new("decision".into());
    let plan = PlanBinding::new("plan".into());
    let observed = no_dependents();
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observed
        )
        .is_some()
    );
    assert!(
        gate.shutdown(
            &AttemptId::new("old-attempt".into()),
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observed
        )
        .is_none()
    );
    assert!(
        gate.shutdown(
            &attempt,
            &OperationId::new("attempt".into()),
            &authority,
            &plan,
            "usr-home".into(),
            &observed
        )
        .is_none()
    );
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &AuthorityBinding::new("conflict".into()),
            &plan,
            "usr-home".into(),
            &observed
        )
        .is_none()
    );
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &PlanBinding::new("other-candidate-or-baseline".into()),
            "usr-home".into(),
            &observed
        )
        .is_none()
    );
    gate.advance(Event::Interrupted);
    assert!(
        gate.shutdown(
            &attempt,
            &id,
            &authority,
            &plan,
            "usr-home".into(),
            &observed
        )
        .is_none()
    );
    let mut retry = shutdown_gate(
        Shutdowns::reviewed(&r, &b, &c, vec!["usr-home".into()], vec![]).unwrap(),
        Some(&saved),
    );
    assert!(
        retry
            .shutdown(
                &attempt,
                &id,
                &authority,
                &plan,
                "usr-home".into(),
                &observed
            )
            .is_none()
    );
    begin_shutdown(&mut retry);
    assert!(
        retry
            .shutdown(
                &attempt,
                &id,
                &authority,
                &plan,
                "usr-home".into(),
                &observed
            )
            .is_some()
    );
}
