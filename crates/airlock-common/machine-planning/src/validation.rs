//! Preserve submitted source and evaluated content through administrative configuration handoffs.
//! These executable checks establish evidence correspondence, never permission to apply a configuration.

use vstd::prelude::*;

verus! {

/// Original submitted bundle and manifest, independently captured before validation.
pub struct Source {
    /// Submitted immutable Git commit.
    pub commit: String,
    /// Digest of the submitted Git bundle bytes.
    pub bundle: String,
    /// Digest of the submitted manifest bytes.
    pub manifest: String,
}

/// Complete content identity of one evaluated candidate.
pub struct Binding {
    /// Original submitted source material.
    pub source: Source,
    /// Digest of the evaluated topology bytes.
    pub topology: String,
    /// Digest of the evaluated host configuration bytes.
    pub system: String,
    /// Generated Rust/Rusix source archive, when present.
    pub prepared: Option<String>,
}

/// Closed interpretation of one validator check outcome.
#[derive(Clone, Copy)]
pub enum Status {
    /// The check completed successfully.
    Passed,
    /// The check failed.
    Failed,
    /// A policy-optional full build was deliberately skipped.
    Skipped,
}

/// One named check after exhaustive wire-name decoding.
pub struct Check {
    /// Mandatory checks occupy 0..11; 11 is the optional full flake build.
    pub kind: u8,
    /// Actual recorded outcome.
    pub status: Status,
}

/// Equality preserves every source coordinate together.
pub open spec fn same_source(a: Source, b: Source) -> bool {
    a.commit@ == b.commit@ && a.bundle@ == b.bundle@ && a.manifest@ == b.manifest@
}

/// Complete candidate equality includes generated-source presence and identity.
pub open spec fn same_binding(a: Binding, b: Binding) -> bool {
    same_source(a.source, b.source) && a.topology@ == b.topology@ && a.system@ == b.system@
        && match (a.prepared, b.prepared) {
            (None, None) => true,
            (Some(x), Some(y)) => x@ == y@,
            _ => false,
        }
}

/// Every required independent check must have successful evidence exactly once.
pub open spec fn present(checks: Seq<Check>, kind: u8) -> bool {
    exists|i: int| 0 <= i < checks.len() && #[trigger] checks[i].kind == kind
}

/// Required checks are successful and unique; only the full build can be skipped.
pub open spec fn complete(checks: Seq<Check>) -> bool {
    &&& forall|i: int| 0 <= i < checks.len() ==> #[trigger] checks[i].kind <= 11
        && (checks[i].status is Passed || checks[i].kind == 11 && checks[i].status is Skipped)
    &&& forall|i: int, j: int| 0 <= j < i < checks.len() ==> #[trigger] checks[i].kind != #[trigger] checks[j].kind
    &&& forall|kind: u8| kind < 11 ==> #[trigger] present(checks, kind)
}

fn source_eq(a: &Source, b: &Source) -> (out: bool) ensures out == same_source(*a, *b),
{ a.commit == b.commit && a.bundle == b.bundle && a.manifest == b.manifest }

fn binding_eq(a: &Binding, b: &Binding) -> (out: bool) ensures out == same_binding(*a, *b),
{
    source_eq(&a.source, &b.source) && a.topology == b.topology && a.system == b.system
        && match (&a.prepared, &b.prepared) {
            (None, None) => true,
            (Some(x), Some(y)) => x.as_str() == y.as_str(),
            _ => false,
        }
}

fn contains_kind(checks: &[Check], kind: u8) -> (out: bool)
    ensures out == present(checks@, kind),
{
    let mut i = 0;
    while i < checks.len()
        invariant 0 <= i <= checks.len(), forall|j: int| 0 <= j < i ==> #[trigger] checks[j].kind != kind,
        decreases checks.len() - i,
    {
        if checks[i].kind == kind { return true; }
        i += 1;
    }
    false
}

/// Refuse omissions, duplicates, failed/skipped mandatory checks and unsupported check kinds.
pub fn check_complete(checks: &[Check]) -> (out: bool) ensures out ==> complete(checks@),
{
    let mut i = 0;
    while i < checks.len()
        invariant 0 <= i <= checks.len(),
            forall|k: int| 0 <= k < i ==> #[trigger] checks[k].kind <= 11
                && (checks[k].status is Passed || checks[k].kind == 11 && checks[k].status is Skipped),
            forall|k: int, j: int| 0 <= j < k < i ==> #[trigger] checks[k].kind != #[trigger] checks[j].kind,
        decreases checks.len() - i,
    {
        if checks[i].kind > 11 || !(matches!(checks[i].status, Status::Passed)
            || checks[i].kind == 11 && matches!(checks[i].status, Status::Skipped)) { return false; }
        let mut j = 0;
        while j < i
            invariant 0 <= j <= i < checks.len(), forall|k: int| 0 <= k < j ==> #[trigger] checks[i as int].kind != #[trigger] checks[k].kind,
            decreases i - j,
        {
            if checks[i].kind == checks[j].kind { return false; }
            j += 1;
        }
        i += 1;
    }
    let mut kind: u8 = 0;
    while kind < 11
        invariant kind <= 11, forall|k: u8| k < kind ==> #[trigger] present(checks@, k),
        decreases 11 - kind,
    {
        if !contains_kind(checks, kind) { return false; }
        kind += 1;
    }
    true
}

/// Successful evidence, sealed against the original submission and independently read artifacts.
pub struct Validated {
    // Identity actually claimed by the validator and matched against the received artifacts.
    binding: Binding,
    // Original submitted source, captured before asking the validator.
    _submitted: Ghost<Source>,
    // Complete original validation evidence.
    _checks: Ghost<Seq<Check>>,
    // Erased independent received-artifact identity used by the constructor.
    _actual: Ghost<Binding>,
    // Erased independently decoded manifest commit.
    _manifest_commit: Ghost<Seq<char>>,
    // Erased summary outcome; passed summaries still require all individual evidence.
    _status: Ghost<Status>,
}

impl Validated {
    /// Exact accepted evaluated-content identity.
    pub closed spec fn identity(&self) -> Binding { self.binding }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        same_source(self.binding.source, self._submitted@) && complete(self._checks@)
            && same_binding(self.binding, self._actual@) && self.binding.source.commit@ == self._manifest_commit@
            && self._status@ is Passed
    }

    /// Check real outcomes and content identities; a passed summary alone cannot create evidence.
    pub fn check(submitted: Source, claimed: Binding, actual: &Binding, manifest_commit: &str, status: Status, checks: Vec<Check>) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().identity() == claimed && same_binding(claimed, *actual)
            && same_source(claimed.source, submitted) && claimed.source.commit@ == manifest_commit@,
    {
        if !matches!(status, Status::Passed) || !check_complete(&checks) || !source_eq(&submitted, &claimed.source)
            || !binding_eq(&claimed, actual) || claimed.source.commit.as_str() != manifest_commit { return None; }
        Some(Self { binding: claimed, _submitted: Ghost(submitted), _checks: Ghost(checks@), _actual: Ghost(*actual), _manifest_commit: Ghost(manifest_commit@), _status: Ghost(status) })
    }

    /// Bind the resolver's output to this original validated candidate, including on a staged restart.
    /// Correct source resolution and actual hashing remain adapter obligations.
    pub fn resolve(self, original: &Binding, topology: String, system: String) -> (out: Option<Resolved>)
        ensures out.is_some() ==> out.unwrap().origin() == self && same_binding(self.identity(), *original)
            && out.unwrap().topology() == topology@ && out.unwrap().system() == system@,
    {
        if !binding_eq(&self.binding, original) || topology.is_empty() || system.is_empty() { return None; }
        Some(Resolved { original: self, topology, system })
    }
}

/// Resolved artifact identity that retains its original validated candidate.
pub struct Resolved {
    // Original source and complete validation evidence, never reconstructed from a success flag.
    original: Validated,
    // Captured resolved topology content.
    topology: String,
    // Captured resolved host contract content.
    system: String,
}

impl Resolved {
    /// Original independently checked candidate.
    pub closed spec fn origin(&self) -> Validated { self.original }

    /// Exact resolver-produced topology identity.
    pub closed spec fn topology(&self) -> Seq<char> { self.topology@ }

    /// Exact resolver-produced host contract identity.
    pub closed spec fn system(&self) -> Seq<char> { self.system@ }

    /// Refuse substituted output content before an administrative apply adapter receives it.
    pub fn select(self, topology: &str, system: &str) -> (out: Option<ApplyInput>)
        ensures out.is_some() ==> out.unwrap().resolved() == self && self.topology() == topology@ && self.system() == system@,
    {
        if self.topology.as_str() != topology || self.system.as_str() != system { return None; }
        Some(ApplyInput { resolved: self })
    }
}

/// Checked apply inputs; this supplies content correspondence, not administrative permission.
pub struct ApplyInput {
    // Original resolved inputs consumed through their exact content gate.
    resolved: Resolved,
}

impl ApplyInput {
    /// Original resolution and its independently validated source.
    pub closed spec fn resolved(&self) -> Resolved { self.resolved }

    /// Forward only the originally validated commit to the administrative coordinator.
    pub fn commit(&self) -> (out: &str)
        ensures out@ == self.resolved().origin().identity().source.commit@,
    { self.resolved.original.binding.source.commit.as_str() }
}

}
