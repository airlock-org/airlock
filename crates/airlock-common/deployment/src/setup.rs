//! Setup completion preserves blockers and requires evidence for independently required work.
//! Decoding, topology fingerprints and truthful helper outcomes remain adapter obligations.

/// Required setup tasks and exact outcome coverage.
pub mod tasks;

use vstd::prelude::*;

verus! {

/// Security-relevant projection of one typed setup report.
pub struct Evidence {
    /// Invocation or deployment attempt owning this work.
    pub run: String,
    /// Fingerprint of the effective configuration actually read by setup.
    pub topology: String,
    /// Requested scope of the root invocation.
    pub scope: String,
    /// The helper completed successfully rather than reporting failure.
    pub passed: bool,
    /// Effects were applied rather than merely planned.
    pub applied: bool,
    /// The root invocation covers a final phase, not preparation or a single helper.
    pub final_phase: bool,
    /// Work remains deferred, including explicit restart actions.
    pub pending: bool,
    /// Count of unsupported domains, including list evidence supplied by the adapter.
    pub unsupported: u64,
}

/// Reports refer to the same nonempty run and configuration.
pub open spec fn same_run(a: Evidence, b: Evidence) -> bool {
    a.run@.len() > 0 && a.topology@.len() > 0
        && a.run@ == b.run@ && a.topology@ == b.topology@
}

/// Complete aggregation preserves root phase and every contributing blocker.
pub open spec fn merged(a: Evidence, b: Evidence, out: Evidence) -> bool {
    same_run(a, b) && out.run@ == a.run@ && out.topology@ == a.topology@
        && out.scope@ == a.scope@ && out.passed == (a.passed && b.passed)
        && out.applied == (a.applied && b.applied)
        && out.final_phase == a.final_phase
        && out.pending == (a.pending || b.pending)
        && out.unsupported == a.unsupported + b.unsupported
}

impl Evidence {
    /// Combine helper outcomes without allowing stale evidence, overflow or erased blockers.
    pub fn merge(self, child: &Self) -> (out: Option<Self>)
        ensures out.is_some() == (same_run(self, *child)
            && self.unsupported + child.unsupported <= u64::MAX),
            out.is_some() ==> merged(self, *child, out.unwrap()),
    {
        if self.run.is_empty() || self.topology.is_empty()
            || self.run != child.run || self.topology != child.topology
            || u64::MAX - self.unsupported < child.unsupported { return None; }
        Some(Self { run: self.run, topology: self.topology, scope: self.scope,
            passed: self.passed && child.passed, applied: self.applied && child.applied,
            final_phase: self.final_phase, pending: self.pending || child.pending,
            unsupported: self.unsupported + child.unsupported })
    }

    /// Retain event-driven deferred work without erasing any report identity or outcome.
    pub fn with_pending(self, pending: bool) -> (out: Self)
        ensures out.run@ == self.run@ && out.topology@ == self.topology@
            && out.scope@ == self.scope@ && out.passed == self.passed && out.applied == self.applied
            && out.final_phase == self.final_phase && out.unsupported == self.unsupported
            && out.pending == (self.pending || pending),
    {
        Self { run: self.run, topology: self.topology, scope: self.scope, passed: self.passed, applied: self.applied,
            final_phase: self.final_phase, pending: self.pending || pending, unsupported: self.unsupported }
    }

}

/// Success requires the expected invocation, applied final work and no remaining blockers.
pub open spec fn complete(run: Seq<char>, topology: Seq<char>, scope: Seq<char>, e: Evidence) -> bool {
    run.len() > 0 && topology.len() > 0 && e.run@ == run && e.topology@ == topology
        && scope.len() > 0 && e.scope@ == scope && e.passed && e.applied && e.final_phase && !e.pending && e.unsupported == 0
}

/// Opaque evidence that setup finished for one exact run and configuration.
pub struct CompletedSetup {
    // Invocation for which the completion predicate was established.
    run: String,
    // Configuration for which the completion predicate was established.
    topology: String,
    // Requested configuration scope retained with completion.
    scope: String,
}

impl View for CompletedSetup {
    type V = (Seq<char>, Seq<char>, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.run@, self.topology@, self.scope@) }
}

impl CompletedSetup {
    /// Construct completion only from a successful, applied and fully finished matching report.
    pub fn check(run: &str, topology: &str, scope: &str, evidence: Evidence) -> (out: Option<Self>)
        ensures out.is_some() == complete(run@, topology@, scope@, evidence),
            out.is_some() ==> out.unwrap()@ == (run@, topology@, scope@),
    {
        if run.is_empty() || topology.is_empty() || scope.is_empty() || evidence.scope.as_str() != scope || evidence.run.as_str() != run
            || evidence.topology.as_str() != topology || !evidence.passed || !evidence.applied
            || !evidence.final_phase || evidence.pending || evidence.unsupported != 0 { return None; }
        Some(Self { run: evidence.run, topology: evidence.topology, scope: evidence.scope })
    }

    /// Prevent a checked completion from being reused for another attempt or configuration.
    pub fn matches(&self, run: &str, topology: &str, scope: &str) -> (yes: bool)
        ensures yes == (self@ == (run@, topology@, scope@)),
    { self.run.as_str() == run && self.topology.as_str() == topology && self.scope.as_str() == scope }
}

}
