//! Exact task coverage is independent of helper invocation logs.

use super::{CompletedSetup, Evidence};
use vstd::prelude::*;

verus! {

/// Global preparation and the distinct responsibilities that must finish for each VM.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Global policy, kernels, templates and labels for the requested scope.
    Preparation,
    /// Common VM registration, properties, features and services.
    Configuration,
    /// Storage/backend reconciliation, including an explicit not-applicable result.
    Backend,
    /// Device reconciliation, including an explicit empty assignment result.
    Devices,
}

/// Required work names a VM and responsibility, independently of observed reports.
pub struct Task {
    /// Canonical VM identity; empty only for global preparation.
    pub domain: String,
    /// Responsibility for which evidence is required.
    pub kind: Kind,
}

/// A helper outcome names its task rather than relying on its position or filename.
pub struct Outcome {
    /// Claimed responsibility and VM identity.
    pub task: Task,
    /// Same-invocation outcome, including all completion blockers.
    pub evidence: Evidence,
}

/// Task equality compares both responsibility and VM identity.
pub open spec fn same(a: Task, b: Task) -> bool {
    a.domain@ == b.domain@ && a.kind == b.kind
}

fn equal(a: &Task, b: &Task) -> (yes: bool)
    ensures yes == same(*a, *b),
{
    a.domain == b.domain && matches!((a.kind, b.kind),
        (Kind::Preparation, Kind::Preparation) | (Kind::Configuration, Kind::Configuration) | (Kind::Backend, Kind::Backend)
            | (Kind::Devices, Kind::Devices))
}

/// Each VM requires configuration, backend and device evidence under its own identity.
pub open spec fn triple(domains: Seq<String>, tasks: Seq<Task>, i: int) -> bool {
    tasks[1 + 3 * i].domain@ == domains[i]@ && tasks[1 + 3 * i].kind == Kind::Configuration
        && tasks[2 + 3 * i].domain@ == domains[i]@ && tasks[2 + 3 * i].kind == Kind::Backend
        && tasks[3 + 3 * i].domain@ == domains[i]@ && tasks[3 + 3 * i].kind == Kind::Devices
}

/// Configuration requires preparation and exactly three responsibilities per VM.
/// This specification describes the required work independently of observed invocations.
pub open spec fn requirements(domains: Seq<String>, tasks: Seq<Task>) -> bool {
    tasks.len() == 1 + 3 * domains.len()
        && tasks[0].domain@.len() == 0 && tasks[0].kind == Kind::Preparation
        && forall|i: int| 0 <= i < domains.len() ==> #[trigger] triple(domains, tasks, i)
}

/// Expand the independently selected VM namespace without dropping any responsibility.
pub fn required(domains: &[String]) -> (out: Option<Vec<Task>>)
    ensures out.is_some() ==> requirements(domains@, out.unwrap()@),
{
    if domains.len() > (usize::MAX - 1) / 3 { return None; }
    let mut tasks = Vec::new();
    tasks.push(Task { domain: String::new(), kind: Kind::Preparation });
    let mut i = 0;
    while i < domains.len()
        invariant 0 <= i <= domains.len(), domains.len() <= (usize::MAX - 1) / 3,
            tasks.len() == 1 + 3 * i,
            tasks[0].domain@.len() == 0, tasks[0].kind == Kind::Preparation,
            forall|k: int| 0 <= k < i ==> #[trigger] triple(domains@, tasks@, k),
        decreases domains.len() - i,
    {
        let ghost before = tasks@;
        tasks.push(Task { domain: domains[i].clone(), kind: Kind::Configuration });
        tasks.push(Task { domain: domains[i].clone(), kind: Kind::Backend });
        tasks.push(Task { domain: domains[i].clone(), kind: Kind::Devices });
        assert forall|k: int| 0 <= k <= i implies #[trigger] triple(domains@, tasks@, k) by {
            if k < i { assert(triple(domains@, before, k)); }
        }
        i += 1;
    }
    Some(tasks)
}

/// One requirement has a distinct, matching and successful outcome.
pub open spec fn ready(root: Evidence, required: Seq<Task>, outcomes: Seq<Outcome>, i: int) -> bool {
    (required[i].domain@.len() == 0 <==> required[i].kind == Kind::Preparation)
        && same(required[i], outcomes[i].task)
        && super::same_run(root, outcomes[i].evidence)
        && (required[i].kind == Kind::Preparation ==> outcomes[i].evidence.scope@ == root.scope@)
        && outcomes[i].evidence.passed && outcomes[i].evidence.applied == root.applied
        && !outcomes[i].evidence.pending && outcomes[i].evidence.unsupported == 0
        && forall|j: int| 0 <= j < i ==> !same(required[i], #[trigger] required[j])
}

/// Exact canonical correspondence rules out missing, extra and substituted outcomes.
/// Each distinct requirement has successful evidence from the root's run and topology.
pub open spec fn covered(root: Evidence, required: Seq<Task>, outcomes: Seq<Outcome>) -> bool {
    required.len() == outcomes.len()
        && forall|i: int| 0 <= i < required.len() ==> #[trigger] ready(root, required, outcomes, i)
}

fn contains(tasks: &[Task], task: &Task) -> (found: bool)
    ensures found == (exists|i: int| 0 <= i < tasks.len() && same(#[trigger] tasks[i], *task)),
{
    let mut i = 0;
    while i < tasks.len()
        invariant 0 <= i <= tasks.len(),
            forall|j: int| 0 <= j < i ==> !same(#[trigger] tasks[j], *task),
        decreases tasks.len() - i,
    {
        if equal(&tasks[i], task) { return true; }
        i += 1;
    }
    false
}

/// Check every requirement; a missing invocation log cannot turn missing work into success.
pub fn check(root: &Evidence, required: &[Task], outcomes: &[Outcome]) -> (yes: bool)
    ensures yes ==> covered(*root, required@, outcomes@),
{
    if required.len() != outcomes.len() { return false; }
    let mut i = 0;
    while i < required.len()
        invariant 0 <= i <= required.len(), required.len() == outcomes.len(),
            forall|k: int| 0 <= k < i ==> #[trigger] ready(*root, required@, outcomes@, k),
        decreases required.len() - i,
    {
        let e = &outcomes[i].evidence;
        if (required[i].domain.is_empty() != matches!(required[i].kind, Kind::Preparation)) || !equal(&required[i], &outcomes[i].task)
            || root.run.is_empty() || root.topology.is_empty()
            || e.run != root.run || e.topology != root.topology
            || (matches!(required[i].kind, Kind::Preparation) && e.scope != root.scope)
            || !e.passed || e.applied != root.applied || e.pending || e.unsupported != 0 { return false; }
        if contains(&required[0..i], &required[i]) { return false; }
        assert forall|j: int| 0 <= j < i implies !same(required[i as int], #[trigger] required[j]) by {
            assert(!same(required@.subrange(0, i as int)[j], required[i as int]));
        }
        assert(ready(*root, required@, outcomes@, i as int));
        i += 1;
    }
    true
}

impl CompletedSetup {
    /// Establish completion only with successful evidence for every distinct required task.
    pub fn check_tasks(run: &str, topology: &str, scope: &str, evidence: Evidence,
        domains: &[String], outcomes: &[Outcome]) -> (out: Option<Self>)
        ensures out.is_some() ==> super::complete(run@, topology@, scope@, evidence)
            && (exists|tasks: Seq<Task>| requirements(domains@, tasks) && covered(evidence, tasks, outcomes@))
            && out.unwrap()@ == (run@, topology@, scope@),
    {
        let tasks = required(domains)?;
        if !check(&evidence, &tasks, outcomes) { return None; }
        assert(requirements(domains@, tasks@) && covered(evidence, tasks@, outcomes@));
        Self::check(run, topology, scope, evidence)
    }
}

}
