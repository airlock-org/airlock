//! Bind a protected review to its candidate, baseline, previous operation and effects.

use vstd::prelude::*;

verus! {

/// Only these two protected statuses carry review authority.
#[derive(Clone, Copy)]
pub enum Status {
    /// Human approval has not yet been consumed.
    Approved,
    /// Publication has associated the approval with a deployment.
    Started,
    /// Every other wire status, including rejection and revocation.
    Denied,
}

/// Borrowed security fields decoded from a protected approval record.
pub struct Review<'a> {
    /// Correlation identity of the protected human decision.
    pub request: &'a str,
    /// Approved source commit.
    pub revision: &'a str,
    /// Approved dependency lock digest.
    pub lock: &'a str,
    /// Guest restriction, absent for whole-machine approval.
    pub guest: Option<&'a str>,
    /// Digest of the machine state against which the review was made.
    pub baseline: &'a str,
    /// Presence of the material shown to the reviewer.
    pub fingerprint: Option<&'a str>,
    /// Protected status, mapped exhaustively from the wire enum.
    pub status: Status,
    /// Previously selected candidate, required for a started approval.
    pub release: Option<&'a str>,
    /// Stable consumed operation, absent in unconsumed or legacy records.
    pub operation: Option<&'a str>,
    /// Maximum set of VM effects, independent of order or duplicates.
    pub allowed: Vec<&'a str>,
    /// Permission for required or unknown reboot effects.
    pub allow_reboot: bool,
}

/// Protected mapping from a candidate's stored location to the source review.
pub struct Binding<'a> {
    /// Review identity selected by the stored candidate binding.
    pub request: &'a str,
    /// Candidate location approved for this binding.
    pub release: &'a str,
    /// Source commit recorded by admission.
    pub revision: &'a str,
    /// Dependency lock digest recorded by admission.
    pub lock: &'a str,
}

/// Candidate and recovery identity, without any caller-supplied effect projection.
pub struct Identity<'a> {
    /// Reviewed release location opened by the coordinator, including during recovery.
    pub release: &'a str,
    /// Source commit in its manifest.
    pub revision: &'a str,
    /// Dependency lock digest in its manifest.
    pub lock: &'a str,
    /// Guest admission target in its manifest.
    pub guest: Option<&'a str>,
    /// Observed baseline for new operations; recovery uses the saved authority.
    pub baseline: Option<&'a str>,
    /// Stable operation read from the previous authoritative intent.
    pub previous_operation: Option<&'a str>,
}

/// Observed reviewed-release fields and the effects proposed for this attempt.
/// During rollback the coordinator separately validates the execution target.
pub struct Candidate<'a> {
    /// Reviewed release location opened by the coordinator, including during recovery.
    pub release: &'a str,
    /// Source commit in its manifest.
    pub revision: &'a str,
    /// Dependency lock digest in its manifest.
    pub lock: &'a str,
    /// Guest admission target in its manifest.
    pub guest: Option<&'a str>,
    /// Observed baseline for new operations; recovery uses the saved authority.
    pub baseline: Option<&'a str>,
    /// Stable operation read from the previous authoritative intent.
    pub previous_operation: Option<&'a str>,
    /// All affected VMs, including both private-copy endpoints.
    pub effects: Vec<&'a str>,
    /// True for both a required reboot and an unknown reboot requirement.
    pub may_reboot: bool,
}

/// Reason the deterministic review gate refused execution.
#[derive(Debug)]
pub enum Denial {
    /// A complete effect plan contains an unclassified extension.
    Unsupported,
    /// The protected binding does not describe the observed candidate.
    Candidate,
    /// Review correlation, status or human-review material is absent or invalid.
    Review,
    /// The source differs from the approved commit or dependency lock.
    Source,
    /// A guest review is being applied to another guest.
    Guest,
    /// A fresh operation was reviewed against another machine state.
    Baseline,
    /// A started decision names another candidate.
    StartedCandidate,
    /// The consumed decision names another operation or has an invalid status.
    Operation,
    /// A VM effect or reboot requirement exceeds the reviewed scope.
    Scope,
}

/// Optional identities compare by string content, including presence.
pub open spec fn same_optional(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => x@ == y@,
        (None, None) => true,
        _ => false,
    }
}

/// Compare optional identities without delegating equality to an unverified adapter.
fn optional_eq(a: Option<&str>, b: Option<&str>) -> (equal: bool)
    ensures equal == same_optional(a, b),
{
    match (a, b) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

/// Every proposed effect must be a member of the protected allow-list.
pub open spec fn covered(allowed: Seq<&str>, effects: Seq<&str>) -> bool {
    forall|i: int| 0 <= i < effects.len() ==>
        #[trigger] member(allowed, effects[i])
}

/// Membership compares identifier contents, without assuming canonical ordering.
pub open spec fn member(allowed: Seq<&str>, name: &str) -> bool {
    exists|j: int| 0 <= j < allowed.len() && name@ == #[trigger] allowed[j]@
}

/// Extract candidate identity independently of the proposed effect list.
pub open spec fn candidate_identity<'a>(c: &Candidate<'a>) -> Identity<'a> {
    Identity { release: c.release, revision: c.revision, lock: c.lock,
        guest: c.guest, baseline: c.baseline, previous_operation: c.previous_operation }
}

/// Identity and status predicate, with baseline freshness required for fresh attempts.
pub open spec fn identity_valid(r: &Review, b: &Binding, c: &Candidate) -> bool {
    identity_authorized(r, b, &candidate_identity(c))
}

/// Exact approved source, candidate, baseline and consumed-operation identity.
pub open spec fn identity_authorized(r: &Review, b: &Binding, c: &Identity) -> bool {
    &&& b.release@ == c.release@
    &&& b.revision@ == c.revision@
    &&& b.lock@ == c.lock@
    &&& r.request@ == b.request@
    &&& (r.status is Approved || r.status is Started)
    &&& r.fingerprint is Some
    &&& r.revision@ == b.revision@
    &&& r.lock@ == b.lock@
    &&& (r.guest is Some ==> same_optional(r.guest, c.guest))
    &&& (c.baseline is Some ==> r.baseline@ == c.baseline.unwrap()@)
    &&& (r.status is Started ==> same_optional(r.release, Some(b.release)))
    &&& (r.operation is Some ==> r.status is Started
        && same_optional(r.operation, c.previous_operation))
}

/// Complete acceptance predicate: valid identity and no effect beyond approved scope.
pub open spec fn authorized(r: &Review, b: &Binding, c: &Candidate) -> bool {
    identity_valid(r, b, c) && covered(r.allowed@, c.effects@)
        && (c.may_reboot ==> r.allow_reboot)
}

/// Find an identity by content; the loop proof covers arbitrary lists and duplicates.
pub(crate) fn contains(allowed: &[&str], name: &str) -> (found: bool)
    ensures found == member(allowed@, name),
{
    let mut j = 0;
    while j < allowed.len()
        invariant
            0 <= j <= allowed.len(),
            forall|k: int| 0 <= k < j ==> name@ != #[trigger] allowed[k]@,
        decreases allowed.len() - j,
    {
        if name == allowed[j] {
            return true;
        }
        j += 1;
    }
    false
}

/// Accept exactly the source, candidate, baseline, identity and scope predicate above.
/// No preconditions are imposed: malformed relationships always return a denial.
pub fn check(r: &Review, b: &Binding, c: &Candidate) -> (result: Result<(), Denial>)
    ensures result.is_ok() == authorized(r, b, c),
{
    if b.release != c.release || b.revision != c.revision || b.lock != c.lock {
        return Err(Denial::Candidate);
    }
    if r.request != b.request || matches!(r.status, Status::Denied) || r.fingerprint.is_none() {
        return Err(Denial::Review);
    }
    if r.revision != b.revision || r.lock != b.lock {
        return Err(Denial::Source);
    }
    if r.guest.is_some() && !optional_eq(r.guest, c.guest) {
        return Err(Denial::Guest);
    }
    if c.baseline.is_some() && !optional_eq(Some(r.baseline), c.baseline) {
        return Err(Denial::Baseline);
    }
    if matches!(r.status, Status::Started) && !optional_eq(r.release, Some(b.release)) {
        return Err(Denial::StartedCandidate);
    }
    if r.operation.is_some()
        && (!matches!(r.status, Status::Started) || !optional_eq(r.operation, c.previous_operation)) {
        return Err(Denial::Operation);
    }
    let mut i = 0;
    while i < c.effects.len()
        invariant
            0 <= i <= c.effects.len(),
            identity_valid(r, b, c),
            forall|k: int| 0 <= k < i ==> #[trigger] member(r.allowed@, c.effects[k]),
        decreases c.effects.len() - i,
    {
        if !contains(r.allowed.as_slice(), c.effects[i]) {
            assert(!covered(r.allowed@, c.effects@));
            return Err(Denial::Scope);
        }
        i += 1;
    }
    if c.may_reboot && !r.allow_reboot {
        return Err(Denial::Scope);
    }
    Ok(())
}

/// Choose a stable operation ID, refusing any change to previously recorded authority.
/// Authority is a canonical encoding supplied by the caller, compared by content here.
pub(crate) fn operation_id<'a>(
    attempt: &'a str,
    legacy: Option<&'a str>,
    previous: Option<(&'a str, &'a str)>,
    authority: &str,
) -> (id: Option<&'a str>)
    ensures
        match previous {
            Some((saved_id, saved_authority)) => {
                &&& id.is_some() == (saved_authority@ == authority@)
                &&& id is Some ==> id.unwrap()@ == saved_id@
            },
            None => same_optional(id, Some(match legacy { Some(old) => old, None => attempt })),
        },
{
    if let Some((id, saved)) = previous {
        if saved != authority {
            return None;
        }
        return Some(id);
    }
    Some(match legacy { Some(old) => old, None => attempt })
}

}
