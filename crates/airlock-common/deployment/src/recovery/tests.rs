//! Exercise erased production decisions with contradictory durable evidence and fresh observations.

use super::*;

/// Give each release path an independently checked content identity.
fn release(path: &str) -> Release {
    Release {
        path: path.into(),
        fingerprint: format!("contents-of-{path}"),
    }
}

/// Describe an interrupted V2 attempt; the original operation and authority remain stable.
fn journal() -> Journal {
    Journal {
        attempt: "101".into(),
        operation: Some(PreviousOperation {
            id: OperationId::new("100".into()),
            authority: AuthorityBinding::new("original-review".into()),
        }),
        candidate: Some(release("candidate")),
        release: release("candidate"),
        previous: release("baseline"),
        plan: "original-plan".into(),
        progress: Progress::Interrupted,
    }
}

/// Copy decoded evidence without copying any executable authority or publication gate.
fn evidence(j: &Journal) -> Journal {
    Journal {
        attempt: j.attempt.clone(),
        operation: j.operation.as_ref().map(|o| PreviousOperation {
            id: OperationId::new(o.id.as_str().into()),
            authority: AuthorityBinding::new(o.authority.as_str().into()),
        }),
        candidate: j.candidate.as_ref().map(|c| Release {
            path: c.path.clone(),
            fingerprint: c.fingerprint.clone(),
        }),
        release: Release {
            path: j.release.path.clone(),
            fingerprint: j.release.fingerprint.clone(),
        },
        previous: Release {
            path: j.previous.path.clone(),
            fingerprint: j.previous.fingerprint.clone(),
        },
        plan: j.plan.clone(),
        progress: j.progress,
    }
}

/// Supply selection fields independently from the journal they claim to reference.
fn selection(journal: Journal) -> Selection {
    Selection {
        attempt: journal.attempt.clone(),
        release: journal.release.path.clone(),
        previous: journal.previous.path.clone(),
        journal,
    }
}

/// Try a production reconciliation with the image baseline as the only absent-selection exception.
fn decision(
    journal: Journal,
    selected: Option<&Selection>,
    mode: Mode,
) -> Option<RecoveryDecision> {
    RecoveryDecision::reconcile(journal, selected, "baseline", mode)
}

/// Consume checked evidence into the same operation type that publication and dispatch require.
fn authorize(
    decision: RecoveryDecision,
    prior: &Journal,
    target: &Release,
    baseline: &Release,
    authority: &str,
) -> Option<AuthorizedOperation> {
    decision.authorize(
        prior,
        target,
        baseline,
        Targets::policy_exempt(vec!["usr-home".into()]),
        Shutdowns::policy_exempt(vec!["usr-home".into()], vec![]).unwrap(),
        Attempt {
            id: AttemptId::new("102".into()),
            authority: AuthorityBinding::new(authority.into()),
            plan: PlanBinding::new("rechecked-plan".into()),
        },
    )
}

/// A saved phase never skips replay of possibly incomplete external effects.
#[test]
fn interrupted_selection_and_rollback_preserve_both_endpoints() {
    for mode in [Mode::Resume, Mode::Rollback] {
        for selected in [None, Some(selection(journal()))] {
            let accepted = decision(journal(), selected.as_ref(), mode).unwrap();
            assert!(matches!(accepted.action(), Action::Reconcile));
            let (target, baseline) = accepted.releases();
            let expected = match mode {
                Mode::Resume => ("candidate", "baseline"),
                Mode::Rollback => ("baseline", "candidate"),
            };
            assert_eq!((target.path(), baseline.path()), expected);
        }
    }
    let mut prior = journal();
    prior.attempt = "100".into();
    let lagging = selection(prior);
    assert!(decision(journal(), Some(&lagging), Mode::Resume).is_some());

    // Selection may still refer to the completed generation preceding this operation.
    let mut previous = journal();
    previous.attempt = "90".into();
    previous.operation = None;
    previous.candidate = None;
    previous.release = release("baseline");
    previous.previous = release("older");
    previous.progress = Progress::Complete;
    assert!(decision(journal(), Some(&selection(previous)), Mode::Rollback).is_some());
}

/// Conflicting selection identity, contents, scope or current plan cannot mint recovery authority.
#[test]
fn contradictory_evidence_fails_closed() {
    let mut wrong = selection(journal());
    wrong.attempt = "another-attempt".into();
    assert!(decision(journal(), Some(&wrong), Mode::Resume).is_none());
    let mut wrong = selection(journal());
    wrong.previous = "different-predecessor".into();
    assert!(decision(journal(), Some(&wrong), Mode::Resume).is_none());
    let mut wrong = journal();
    wrong.release = release("third-release");
    assert!(decision(journal(), Some(&selection(wrong)), Mode::Resume).is_none());
    let mut wrong = journal();
    wrong.release.fingerprint = "changed-contents-at-same-path".into();
    assert!(decision(journal(), Some(&selection(wrong)), Mode::Resume).is_none());
    let mut wrong = journal();
    wrong.plan = "different-plan".into();
    assert!(decision(journal(), Some(&selection(wrong)), Mode::Resume).is_none());
    let mut wrong = journal();
    wrong.attempt = "100".into();
    wrong.operation.as_mut().unwrap().authority = AuthorityBinding::new("changed-scope".into());
    assert!(decision(journal(), Some(&selection(wrong)), Mode::Resume).is_none());
    let mut wrong = journal();
    wrong.attempt = "90".into();
    wrong.operation = None;
    wrong.candidate = None;
    assert!(decision(journal(), Some(&selection(wrong)), Mode::Resume).is_none());
    let mut nonbootstrap = journal();
    nonbootstrap.previous = release("non-image-generation");
    assert!(decision(nonbootstrap, None, Mode::Resume).is_none());
    let mut collapsed = journal();
    collapsed.release = release("baseline");
    assert!(decision(collapsed, None, Mode::Resume).is_none());
    let mut unbound = journal();
    unbound.candidate = Some(release("different-approved-candidate"));
    assert!(decision(unbound, None, Mode::Rollback).is_none());
}

/// Verified outcomes require the current selection; reboot waiting never authorizes a new resume attempt.
#[test]
fn verified_outcomes_require_consistent_selection() {
    for progress in [Progress::Complete, Progress::RebootRequired] {
        let completed = || {
            let mut j = journal();
            j.progress = progress;
            j
        };
        let current = selection(completed());
        let accepted = decision(completed(), Some(&current), Mode::Resume).unwrap();
        assert!(matches!(accepted.action(), Action::Verify));
        assert!(
            authorize(
                accepted,
                &completed(),
                &release("candidate"),
                &release("baseline"),
                "original-review"
            )
            .is_none()
        );
        assert!(decision(completed(), None, Mode::Resume).is_none());
        let mut prior = completed();
        prior.attempt = "100".into();
        assert!(decision(completed(), Some(&selection(prior)), Mode::Resume).is_none());
        assert_eq!(
            decision(completed(), Some(&current), Mode::Rollback).is_some(),
            matches!(progress, Progress::RebootRequired)
        );
    }
}

/// Reconciliation evidence cannot be reused with an altered journal, release pair or protected decision.
#[test]
fn execution_consumes_exact_recovery_bindings() {
    let accepted = || decision(journal(), None, Mode::Rollback).unwrap();
    let correct = authorize(
        accepted(),
        &journal(),
        &release("baseline"),
        &release("candidate"),
        "original-review",
    )
    .unwrap();
    assert_eq!(correct.id().as_str(), "100");
    assert!(
        authorize(
            accepted(),
            &journal(),
            &release("candidate"),
            &release("baseline"),
            "original-review"
        )
        .is_none()
    );
    let mut stale = journal();
    stale.plan = "substituted-plan".into();
    assert!(
        authorize(
            accepted(),
            &stale,
            &release("baseline"),
            &release("candidate"),
            "original-review"
        )
        .is_none()
    );
    assert!(
        authorize(
            accepted(),
            &journal(),
            &release("baseline"),
            &release("candidate"),
            "expanded-review"
        )
        .is_none()
    );
    let mut changed = release("baseline");
    changed.fingerprint = "substituted-target".into();
    assert!(
        authorize(
            accepted(),
            &journal(),
            &changed,
            &release("candidate"),
            "original-review"
        )
        .is_none()
    );

    // A successful reconciliation still cannot request an effect before both durable barriers.
    let mut gate = correct
        .publication(
            &AttemptId::new("102".into()),
            &OperationId::new("100".into()),
            &AuthorityBinding::new("original-review".into()),
            &PlanBinding::new("rechecked-plan".into()),
            crate::operation::IntentBinding::new("saved-intent".into()),
        )
        .unwrap();
    assert!(
        gate.request(
            &AttemptId::new("102".into()),
            &OperationId::new("100".into()),
            &AuthorityBinding::new("original-review".into()),
            &PlanBinding::new("rechecked-plan".into()),
            "usr-home".into()
        )
        .is_none()
    );
    gate.advance(crate::publication::Event::Interrupted);
    assert!(!gate.begin(
        &AttemptId::new("102".into()),
        &crate::operation::IntentBinding::new("saved-intent".into())
    ));
}

/// Native V1 adopts its attempt identity once; later attempts retain the same V2 operation.
#[test]
fn legacy_adoption_and_repeated_direction_changes_keep_operation() {
    let mut prior = journal();
    prior.operation = None;
    prior.candidate = None;
    for attempt in ["102", "103", "104"] {
        let accepted = decision(evidence(&prior), None, Mode::Rollback).unwrap();
        let target = release(accepted.releases().0.path());
        let baseline = release(accepted.releases().1.path());
        let op = accepted
            .authorize(
                &prior,
                &target,
                &baseline,
                Targets::policy_exempt(vec![]),
                Shutdowns::policy_exempt(vec![], vec![]).unwrap(),
                Attempt {
                    id: AttemptId::new(attempt.into()),
                    authority: AuthorityBinding::new("original-review".into()),
                    plan: PlanBinding::new("original-plan".into()),
                },
            )
            .unwrap();
        assert_eq!(op.id().as_str(), "101");
        prior = Journal {
            attempt: attempt.into(),
            operation: Some(PreviousOperation {
                id: OperationId::new(op.id().as_str().into()),
                authority: AuthorityBinding::new("original-review".into()),
            }),
            candidate: Some(release("candidate")),
            release: target,
            previous: baseline,
            plan: "original-plan".into(),
            progress: Progress::Interrupted,
        };
    }
}

/// Observed shutdown success is rechecked; an unrelated running VM needs no shutdown.
#[test]
fn uncertain_shutdown_and_external_restart_block_configuration() {
    let required = vec!["usr-home".into(), "sys-net".into()];
    assert!(quiescent(&required, &["unaffected".into()]));
    assert!(!quiescent(&required, &["sys-net".into()]));
    assert!(quiescent(&required, &[]));
    assert!(!quiescent(&required, &["usr-home".into()]));
}
