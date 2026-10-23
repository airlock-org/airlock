//! Verify exact caller, target, source, approval binding and output ceilings.
//! Authentication, installed-contract integrity, query encoding and faithful execution
//! remain trusted Rust/Qubes boundaries. These proofs do not establish secret-free logs.

use vstd::prelude::*;

verus! {

/// Trusted installed authorization scope, decoded outside the verified core.
pub struct Policy {
    /// Exact VM or dom0 identity served by this endpoint.
    pub target: String,
    /// Authenticated Qubes sources allowed to request evidence.
    pub callers: Vec<String>,
    /// Canonical source descriptors, including their complete reader configuration.
    pub sources: Vec<String>,
    /// Maximum combined stdout/stderr bytes permitted for one request.
    pub ceiling: u64,
}

/// Complete immutable request identity, assembled after strict wire validation.
pub struct Request {
    /// Identity supplied by Qubes, never taken from request JSON.
    pub caller: String,
    /// Requested endpoint, independently checked against the installed target.
    pub target: String,
    /// Complete descriptor selected from the installed source inventory.
    pub source: String,
    /// Canonical full query and installed contract, excluding no query parameters.
    pub binding: String,
    /// Combined output budget requested by the client.
    pub bytes: u64,
}

/// Approval-provider result; an automatic decision is not a human approval.
pub struct Decision {
    /// Exact complete query to which this decision applies.
    pub binding: String,
    /// Provider accepted this query under its installed approval mode.
    pub approved: bool,
}

/// Membership compares full contents, without assuming canonical list order.
pub open spec fn member(values: Seq<String>, value: Seq<char>) -> bool {
    exists|i: int| 0 <= i < values.len() && #[trigger] values[i]@ == value
}

/// Exact authorization predicate; possession grants no deployment authority.
pub open spec fn authorized(policy: &Policy, request: &Request, decision: &Decision) -> bool {
    &&& request.target@ == policy.target@
    &&& member(policy.callers@, request.caller@)
    &&& member(policy.sources@, request.source@)
    &&& request.bytes > 0
    &&& request.bytes <= policy.ceiling
    &&& decision.approved
    &&& decision.binding@ == request.binding@
}

/// Accepted immutable query; callers cannot construct or mutate its request.
pub struct Permit {
    /// Exact identity retained from the successful authorization check.
    request: Request,
}

/// Compare complete source/caller identities using the executable Rust equality.
fn contains(values: &[String], value: &str) -> (found: bool)
    ensures found == member(values@, value@),
{
    let mut index = 0;
    while index < values.len()
        invariant
            index <= values.len(),
            forall|i: int| 0 <= i < index ==> #[trigger] values[i]@ != value@,
        decreases values.len() - index,
    {
        if values[index].as_str() == value {
            return true;
        }
        index += 1;
    }
    false
}

impl Permit {
    /// Abstract immutable request identity, exposing no executable constructor or mutation.
    pub closed spec fn identity(&self) -> Request { self.request }

    /// Return a permit exactly when all identity, scope and approval checks hold.
    pub fn check(policy: &Policy, request: Request, decision: &Decision) -> (result: Option<Self>)
        ensures
            result is Some <==> authorized(policy, &request, decision),
            result is Some ==> result.unwrap().identity() == request,
    {
        if request.target != policy.target
            || !contains(&policy.callers, request.caller.as_str())
            || !contains(&policy.sources, request.source.as_str())
            || request.bytes == 0 || request.bytes > policy.ceiling
            || !decision.approved || decision.binding != request.binding
        {
            return None;
        }
        Some(Self { request })
    }

    /// Borrow the checked request without permitting replacement after authorization.
    pub fn request(&self) -> (request: &Request)
        ensures *request == self.identity(),
    {
        &self.request
    }
}

/// Permission to relay only back to the authenticated observer, never to another VM.
pub struct SelfRelay {
    /// Destination retained from the successful three-way identity comparison.
    target: String,
}

impl SelfRelay {
    /// Abstract destination retained by the relay permission.
    pub closed spec fn destination(&self) -> Seq<char> { self.target@ }

    /// Authorize exactly an installed observer requesting its own target.
    pub fn check(observer: &str, caller: &str, target: String) -> (result: Option<Self>)
        ensures
            result is Some <==> observer@ == caller@ && target@ == caller@,
            result is Some ==> result.unwrap().destination() == caller@,
    {
        if observer != caller || target.as_str() != caller { return None; }
        Some(Self { target })
    }

    /// Borrow the checked destination used as the relay's sole VM argument.
    pub fn target(&self) -> (target: &str)
        ensures target@ == self.destination(),
    {
        &self.target
    }
}

/// Shared byte allowance for both evidence streams; consuming it can never widen access.
pub struct Budget {
    /// Unconsumed bytes from the original approved allowance.
    remaining: u64,
}

impl Budget {
    /// Abstract unconsumed allowance used in the accounting contracts.
    pub closed spec fn remaining(&self) -> u64 { self.remaining }

    /// Start accounting from the exact approved allowance.
    pub fn new(bytes: u64) -> (budget: Self)
        ensures budget.remaining() == bytes,
    {
        Self { remaining: bytes }
    }

    /// Keep only bytes within the remaining allowance, accounting across both streams.
    pub fn consume(&mut self, offered: u64) -> (kept: u64)
        ensures
            kept <= offered,
            kept <= old(self).remaining(),
            final(self).remaining() == old(self).remaining() - kept,
            kept == offered || final(self).remaining() == 0,
    {
        let kept = if offered <= self.remaining { offered } else { self.remaining };
        self.remaining -= kept;
        kept
    }
}

}
