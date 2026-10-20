//! Restrict deployment lifecycle requests to a checked plan and an executing intent.
//! The Qubes adapter consumes an opaque request with an immutable target.

pub use crate::inventory::ShutdownObservation;
use crate::{publication::Publication, review};
use airlock_machine_planning::names::owned_contains;
use vstd::prelude::*;

verus! {

/// Mathematical contents of a lifecycle target set and its original review scope.
pub ghost struct Scope {
    /// Exact planned targets for this lifecycle operation, compared by string content.
    pub targets: Seq<String>,
    /// VM names in the original approval; empty for an explicit policy exemption.
    pub allowed: Seq<Seq<char>>,
    /// Whether the policy was constructed by checking a human review.
    pub reviewed: bool,
}

/// An immutable target subset admitted by a review or an explicit policy exemption.
pub struct Targets {
    // Only the checked constructors can populate or change this list.
    targets: Vec<String>,
    // Proof-only snapshot of the original approved scope, not a caller-supplied flag.
    _scope: Ghost<Seq<Seq<char>>>,
    // Distinguishes checked review authority from the coordinator's policy exceptions.
    _reviewed: Ghost<bool>,
}

impl View for Targets {
    type V = Scope;

    closed spec fn view(&self) -> Scope {
        Scope { targets: self.targets@, allowed: self._scope@, reviewed: self._reviewed@ }
    }
}

/// A reviewed target list never contains a VM outside its original approval.
pub open spec fn valid(s: Scope) -> bool {
    s.reviewed ==> forall|i: int| 0 <= i < s.targets.len() ==>
        s.allowed.contains(#[trigger] s.targets[i]@)
}

/// Membership in a stored target list compares names, not their allocation identities.
pub open spec fn planned(s: Scope, target: Seq<char>) -> bool {
    airlock_machine_planning::names::owned_member(s.targets, target)
}

/// Every stored lifecycle target also occurs in the candidate's checked effect set.
pub open spec fn included(targets: Seq<String>, effects: Seq<&str>) -> bool {
    forall|i: int| 0 <= i < targets.len() ==>
        #[trigger] named(effects, targets[i]@)
}

/// Match an owned target's mathematical contents against borrowed review fields.
pub open spec fn named(effects: Seq<&str>, target: Seq<char>) -> bool {
    exists|j: int| 0 <= j < effects.len() && target == #[trigger] effects[j]@
}

impl Targets {
    /// Decide target membership using the same contents as the scope specification.
    pub(crate) fn contains(&self, target: &str) -> (found: bool)
        ensures found == planned(self@, target@),
    { owned_contains(self.targets.as_slice(), target) }

    /// Check the full review and ensure each target belongs to that checked plan.
    /// No preconditions: an expanded list, mismatched identity or denied review fails closed.
    pub fn reviewed(
        r: &review::Review, b: &review::Binding, c: &review::Candidate,
        targets: Vec<String>,
    ) -> (result: Result<Self, review::Denial>)
        ensures
            result.is_ok() ==> review::authorized(r, b, c),
            result.is_ok() ==> valid(result.unwrap()@),
            result.is_ok() ==> result.unwrap()@.reviewed,
            result.is_ok() ==> result.unwrap()@.targets == targets@,
            result.is_ok() ==> included(targets@, c.effects@),
            result.is_ok() ==> result.unwrap()@.allowed == r.allowed@.map(|i, n: &str| n@),
    {
        review::check(r, b, c)?;

        let mut i = 0;
        while i < targets.len()
            invariant
                0 <= i <= targets.len(),
                review::authorized(r, b, c),
                forall|k: int| 0 <= k < i ==>
                    #[trigger] named(r.allowed@, targets[k]@),
                forall|k: int| 0 <= k < i ==>
                    #[trigger] named(c.effects@, targets[k]@),
            decreases targets.len() - i,
        {
            if !review::contains(c.effects.as_slice(), targets[i].as_str())
                || !review::contains(r.allowed.as_slice(), targets[i].as_str()) {
                return Err(review::Denial::Scope);
            }
            i += 1;
        }

        let ghost allowed = r.allowed@.map(|i, n: &str| n@);
        proof {
            assert forall|k: int| 0 <= k < targets.len() implies
                allowed.contains(#[trigger] targets[k]@) by {
                assert(named(r.allowed@, targets[k]@));
                let j = choose|j: int| 0 <= j < r.allowed@.len()
                    && targets[k]@ == #[trigger] r.allowed@[j]@;
                assert(allowed[j] == targets[k]@);
            }
        }

        Ok(Self { targets, _scope: Ghost(allowed), _reviewed: Ghost(true) })
    }

    /// Bound an explicitly exempt deployment to its plan; this grants no review theorem.
    /// Only the coordinator's existing administrative/review-disabled policy uses this.
    pub fn policy_exempt(targets: Vec<String>) -> (s: Self)
        ensures valid(s@), !s@.reviewed, s@.targets == targets@,
    {
        Self { targets, _scope: Ghost(Seq::empty()), _reviewed: Ghost(false) }
    }

    /// Borrow immutable target names for complete-definition validation.
    pub(crate) fn names(&self) -> (names: &[String])
        ensures names@ == self@.targets,
    { self.targets.as_slice() }

    /// Retain an immutable copy for another request family without changing approval.
    pub(crate) fn duplicate(&self) -> (out: Self)
        ensures out@.allowed == self@.allowed, out@.reviewed == self@.reviewed,
            out@.targets.len() == self@.targets.len(),
            forall|i: int| 0 <= i < self@.targets.len() ==>
                #[trigger] out@.targets[i]@ == self@.targets[i]@,
            valid(self@) ==> valid(out@),
    {
        let mut targets: Vec<String> = Vec::new();
        let mut i = 0;
        while i < self.targets.len()
            invariant
                0 <= i <= self.targets.len(), targets.len() == i,
                forall|j: int| 0 <= j < i ==> #[trigger] targets[j]@ == self.targets[j]@,
            decreases self.targets.len() - i,
        {
            targets.push(self.targets[i].as_str().to_string());
            i += 1;
        }
        Self { targets, _scope: self._scope, _reviewed: self._reviewed }
    }

    /// Bind lifecycle targets to the persisted attempt and immutable execution context.
    /// The adapter computes these digests from the same authenticated records and plan.
    pub(crate) fn bind(self, shutdowns: Shutdowns, attempt: String, intent: String, context: String) -> (e: Execution)
        ensures e@.scope == self@, e@.attempt == attempt@,
            e@.intent == intent@, e@.context == context@,
            e@.shutdown == shutdowns@,
    {
        Execution { starts: self, shutdowns, attempt, intent, context }
    }
}

/// Untrusted plan evidence: clients whose GUI connections may be interrupted while they stay running.
/// This record alone cannot construct a forced shutdown request.
pub struct GuiPreservation {
    /// Provider explicitly scheduled for shutdown; only sys-gui preservation is supported.
    pub provider: String,
    /// Expected surviving clients, all of which must fit the original approved effect scope.
    pub clients: Vec<String>,
}

/// GUI preservation applies only to a stopped sys-gui and excludes explicitly stopped clients.
pub open spec fn gui_entry_shape(targets: Scope, gui: GuiPreservation) -> bool {
    &&& gui.provider@ == "sys-gui"@
    &&& planned(targets, gui.provider@)
    &&& gui.clients.len() > 0
    &&& forall|j: int| 0 <= j < gui.clients.len() ==>
        !#[trigger] planned(targets, gui.clients[j]@)
}

/// Every GUI entry describes a valid preservation mode for this shutdown plan.
pub open spec fn gui_shape(targets: Scope, gui: Seq<GuiPreservation>) -> bool {
    forall|i: int| 0 <= i < gui.len() ==> #[trigger] gui_entry_shape(targets, gui[i])
}

/// Every expected GUI client is present in a borrowed candidate or review effect set.
pub open spec fn gui_covered(gui: Seq<GuiPreservation>, names: Seq<&str>) -> bool {
    forall|i: int, j: int| 0 <= i < gui.len() && 0 <= j < gui[i].clients.len() ==>
        #[trigger] named(names, gui[i].clients[j]@)
}

/// Expected GUI clients are effects covered by the original human scope.
pub open spec fn gui_allowed(gui: Seq<GuiPreservation>, allowed: Seq<Seq<char>>) -> bool {
    forall|i: int, j: int| 0 <= i < gui.len() && 0 <= j < gui[i].clients.len() ==>
        allowed.contains(#[trigger] gui[i].clients[j]@)
}

/// Validate preservation structure even for administrative and review-disabled deployments.
fn check_gui_shape(targets: &Targets, gui: &[GuiPreservation]) -> (accepted: bool)
    ensures accepted ==> gui_shape(targets@, gui@),
{
    let mut i = 0;
    while i < gui.len()
        invariant
            0 <= i <= gui.len(),
            forall|k: int| 0 <= k < i ==> #[trigger] gui_entry_shape(targets@, gui[k]),
        decreases gui.len() - i,
    {
        let entry = &gui[i];
        if entry.provider.as_str() != "sys-gui" || !targets.contains(entry.provider.as_str())
            || entry.clients.is_empty() { return false; }

        let mut j = 0;
        while j < entry.clients.len()
            invariant
                0 <= j <= entry.clients.len(),
                entry.provider@ == "sys-gui"@ && planned(targets@, entry.provider@),
                entry.clients.len() > 0,
                forall|k: int| 0 <= k < j ==> !#[trigger] planned(targets@, entry.clients[k]@),
            decreases entry.clients.len() - j,
        {
            if targets.contains(entry.clients[j].as_str()) { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Check expected clients against actual scope data, without an authority flag from the caller.
fn check_gui_coverage(gui: &[GuiPreservation], names: &[&str]) -> (accepted: bool)
    ensures accepted ==> gui_covered(gui@, names@),
{
    let mut i = 0;
    while i < gui.len()
        invariant
            0 <= i <= gui.len(),
            forall|k: int, j: int| 0 <= k < i && 0 <= j < gui[k].clients.len() ==>
                #[trigger] named(names@, gui[k].clients[j]@),
        decreases gui.len() - i,
    {
        let clients = &gui[i].clients;
        let mut j = 0;
        while j < clients.len()
            invariant
                0 <= j <= clients.len(),
                forall|k: int| 0 <= k < j ==> #[trigger] named(names@, clients[k]@),
            decreases clients.len() - j,
        {
            if !review::contains(names, clients[j].as_str()) { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Shutdown targets and the exact GUI-preservation modes retained from the checked plan.
pub ghost struct ShutdownScope {
    /// Explicit shutdown targets and original approval scope.
    pub targets: Scope,
    /// Provider-to-client sets, unchanged after authorization.
    pub gui: Seq<GuiPreservation>,
}

/// A checked shutdown plan accounts for both explicit targets and preserved GUI clients.
pub open spec fn shutdown_valid(s: ShutdownScope) -> bool {
    &&& valid(s.targets)
    &&& gui_shape(s.targets, s.gui)
    &&& (s.targets.reviewed ==> gui_allowed(s.gui, s.targets.allowed))
}

/// Owned, immutable shutdown authority, including the allowed GUI force modes.
pub struct Shutdowns {
    // Explicit targets admitted by the same target checker used for startup.
    targets: Targets,
    // Plan evidence validated for shape, complete client effects and approval scope.
    gui: Vec<GuiPreservation>,
}

impl View for Shutdowns {
    type V = ShutdownScope;

    closed spec fn view(&self) -> ShutdownScope {
        ShutdownScope { targets: self.targets@, gui: self.gui@ }
    }
}

impl Shutdowns {
    /// Check explicit shutdowns and all expected GUI clients against the original review.
    pub fn reviewed(r: &review::Review, b: &review::Binding, c: &review::Candidate,
        targets: Vec<String>, gui: Vec<GuiPreservation>) -> (result: Result<Self, review::Denial>)
        ensures
            result.is_ok() ==> review::authorized(r, b, c),
            result.is_ok() ==> shutdown_valid(result.unwrap()@),
            result.is_ok() ==> result.unwrap()@.targets.reviewed,
            result.is_ok() ==> result.unwrap()@.targets.targets == targets@,
            result.is_ok() ==> result.unwrap()@.gui == gui@,
            result.is_ok() ==> included(targets@, c.effects@) && gui_covered(gui@, c.effects@),
            result.is_ok() ==> result.unwrap()@.targets.allowed == r.allowed@.map(|i, n: &str| n@),
    {
        let targets = Targets::reviewed(r, b, c, targets)?;
        if !check_gui_shape(&targets, gui.as_slice())
            || !check_gui_coverage(gui.as_slice(), r.allowed.as_slice())
            || !check_gui_coverage(gui.as_slice(), c.effects.as_slice()) {
            return Err(review::Denial::Scope);
        }

        proof {
            assert forall|i: int, j: int| 0 <= i < gui.len() && 0 <= j < gui[i].clients.len()
                implies targets@.allowed.contains(#[trigger] gui[i].clients[j]@) by {
                assert(named(r.allowed@, gui[i].clients[j]@));
                let k = choose|k: int| 0 <= k < r.allowed.len()
                    && gui[i].clients[j]@ == #[trigger] r.allowed@[k]@;
                assert(targets@.allowed[k] == gui[i].clients[j]@);
            }
        }
        Ok(Self { targets, gui })
    }

    /// Retain existing policy exemptions while requiring a coherent shutdown/GUI plan.
    pub fn policy_exempt(targets: Vec<String>, gui: Vec<GuiPreservation>) -> (result: Option<Self>)
        ensures
            result.is_some() ==> shutdown_valid(result.unwrap()@),
            result.is_some() ==> !result.unwrap()@.targets.reviewed,
            result.is_some() ==> result.unwrap()@.targets.targets == targets@,
            result.is_some() ==> result.unwrap()@.gui == gui@,
    {
        let targets = Targets::policy_exempt(targets);
        if !check_gui_shape(&targets, gui.as_slice()) { return None; }
        Some(Self { targets, gui })
    }
}

/// Identity and scope retained across all phases of an execution attempt.
pub ghost struct ExecutionState {
    /// Approved startup subset, unchanged after binding.
    pub scope: Scope,
    /// Checked shutdown targets, modes and GUI clients.
    pub shutdown: ShutdownScope,
    /// Attempt whose durable gate must be executing.
    pub attempt: Seq<char>,
    /// Digest of the complete preflight journal acknowledged by the gate.
    pub intent: Seq<char>,
    /// Digest of immutable journal fields, including operation, candidate, baseline and plan.
    pub context: Seq<char>,
}

/// Private immutable bindings prevent a later plan or target substitution.
pub(crate) struct Execution {
    // Checked startup subset and original scope.
    starts: Targets,
    // Checked shutdown targets and GUI preservation modes from the same plan.
    shutdowns: Shutdowns,
    // Exact attempt whose publication gate must be used.
    attempt: String,
    // Full preflight digest, retained after the journal phase changes.
    intent: String,
    // Immutable execution fields independently re-extracted at dispatch.
    context: String,
}

impl View for Execution {
    type V = ExecutionState;

    closed spec fn view(&self) -> ExecutionState {
        ExecutionState { scope: self.starts@, attempt: self.attempt@,
            shutdown: self.shutdowns@, intent: self.intent@, context: self.context@ }
    }
}

/// A single checked Qubes start request. Its lifetime keeps the gate borrowed until dispatch.
/// There is no public constructor, target setter or clone implementation.
pub struct StartRequest<'a> {
    // The exact string admitted by request(), moved intact into this capability.
    target: String,
    // Prevents changing the gate to complete/interrupted while a request is outstanding.
    _publication: &'a Publication,
    // Keeps the request attached to its immutable identity and scope.
    _execution: &'a Execution,
}

/// The exact request and the gate and scope against which it was checked.
pub ghost struct RequestState {
    /// The sole VM target handed to Qubes.
    pub target: Seq<char>,
    /// Immutable deployment identity and admitted scope.
    pub execution: ExecutionState,
    /// Executing gate borrowed for this request's lifetime.
    pub publication: crate::publication::State,
}

impl<'a> View for StartRequest<'a> {
    type V = RequestState;

    closed spec fn view(&self) -> RequestState {
        RequestState { target: self.target@, execution: self._execution@,
            publication: self._publication@ }
    }
}

impl Execution {
    /// Both lifecycle request kinds require this exact executing, acknowledged intent.
    pub(crate) fn permits(&self, publication: &Publication, attempt: &str, context: &str) -> (allowed: bool)
        ensures allowed == (publication@.stage is Executing && publication@.intent && publication@.approval
            && publication@.attempt == self@.attempt && attempt@ == self@.attempt
            && publication@.binding == self@.intent && context@ == self@.context),
    {
        self.attempt.as_str() == attempt && self.context.as_str() == context
            && publication.executing(attempt, self.intent.as_str())
    }

    /// Select an exact target only while this intent is executing with unchanged context.
    pub(crate) fn request<'a>(
        &'a self, publication: &'a Publication, attempt: &str, context: &str, target: String,
    ) -> (request: Option<StartRequest<'a>>)
        ensures
            request.is_some() ==> publication@.stage is Executing
                && publication@.intent && publication@.approval,
            request.is_some() ==> publication@.attempt == self@.attempt
                && attempt@ == self@.attempt && publication@.binding == self@.intent,
            request.is_some() ==> context@ == self@.context,
            request.is_some() ==> planned(self@.scope, target@),
            request.is_some() && self@.scope.reviewed && valid(self@.scope) ==>
                self@.scope.allowed.contains(target@),
            request.is_some() ==> request.unwrap()@.target == target@,
            request.is_some() ==> request.unwrap()@.execution == self@
                && request.unwrap()@.publication == publication@,
    {
        if !self.permits(publication, attempt, context) || !self.starts.contains(target.as_str()) {
            return None;
        }
        Some(StartRequest { target, _publication: publication, _execution: self })
    }
}

/// A mode and its full GUI effect set occur together in the immutable shutdown plan.
pub open spec fn shutdown_mode(s: ShutdownScope, target: Seq<char>, forced: bool, clients: Seq<String>) -> bool {
    if forced {
        exists|i: int| 0 <= i < s.gui.len() && #[trigger] s.gui[i].provider@ == target
            && s.gui[i].clients@ == clients
    } else {
        clients.len() == 0 && forall|i: int| 0 <= i < s.gui.len() ==>
            #[trigger] s.gui[i].provider@ != target
    }
}

/// Immutable shutdown request: target and force mode can only come from the checked plan.
/// The Qubes helper may kill this same target after its existing wait timeout.
pub struct ShutdownRequest<'a> {
    // Exact explicit target selected from the shutdown list.
    target: String,
    // Presence fixes --force and borrows the exact expected GUI-client set from the plan.
    gui: Option<&'a GuiPreservation>,
    // Keeps the gate executing until the adapter consumes this request.
    _publication: &'a Publication,
    // Keeps both immutable identity and scope bindings alive during dispatch.
    _execution: &'a Execution,
}

/// Specification of the exact target, mode, clients and gate carried to the adapter.
pub ghost struct ShutdownRequestState {
    /// The only explicit shutdown/timeout-kill target passed to Qubes.
    pub target: Seq<char>,
    /// Bypass dependent-VM protection only for a planned GUI-preservation operation.
    pub forced: bool,
    /// Complete expected set of affected but preserved GUI clients.
    pub clients: Seq<String>,
    /// Attempt, candidate/baseline binding and checked lifecycle scopes.
    pub execution: ExecutionState,
    /// Executing publication gate borrowed by this request.
    pub publication: crate::publication::State,
}

impl<'a> View for ShutdownRequest<'a> {
    type V = ShutdownRequestState;

    closed spec fn view(&self) -> ShutdownRequestState {
        ShutdownRequestState { target: self.target@, forced: self.gui is Some,
            clients: match self.gui { Some(gui) => gui.clients@, None => Seq::empty() },
            execution: self._execution@, publication: self._publication@ }
    }
}

impl Execution {
    /// Select shutdown target and force mode from the saved plan, never from a caller flag.
    pub(crate) fn shutdown<'a>(&'a self, publication: &'a Publication, attempt: &str, context: &str,
        target: String, observation: &ShutdownObservation) -> (result: Option<ShutdownRequest<'a>>)
        ensures
            result.is_some() ==> publication@.stage is Executing && publication@.intent && publication@.approval,
            result.is_some() ==> publication@.attempt == self@.attempt && attempt@ == self@.attempt
                && publication@.binding == self@.intent && context@ == self@.context,
            result.is_some() ==> planned(self@.shutdown.targets, target@),
            result.is_some() ==> result.unwrap()@.target == target@
                && result.unwrap()@.execution == self@ && result.unwrap()@.publication == publication@,
            result.is_some() ==> shutdown_mode(self@.shutdown, target@,
                result.unwrap()@.forced, result.unwrap()@.clients),
            result.is_some() ==> crate::inventory::permitted(observation@, target@,
                result.unwrap()@.clients, result.unwrap()@.forced),
            result.is_some() && self@.shutdown.targets.reviewed && shutdown_valid(self@.shutdown) ==>
                crate::inventory::clients_within(observation@, target@, self@.shutdown.targets.allowed),
            result.is_some() && self@.shutdown.targets.reviewed && shutdown_valid(self@.shutdown) ==>
                self@.shutdown.targets.allowed.contains(target@)
                && forall|j: int| 0 <= j < result.unwrap()@.clients.len() ==>
                    self@.shutdown.targets.allowed.contains(#[trigger] result.unwrap()@.clients[j]@),
    {
        if !self.permits(publication, attempt, context)
            || !self.shutdowns.targets.contains(target.as_str()) {
            return None;
        }

        let mut i = 0;
        while i < self.shutdowns.gui.len()
            invariant
                0 <= i <= self.shutdowns.gui.len(),
                publication@.stage is Executing && publication@.intent && publication@.approval,
                publication@.attempt == self@.attempt && attempt@ == self@.attempt,
                publication@.binding == self@.intent && context@ == self@.context,
                planned(self@.shutdown.targets, target@),
                forall|k: int| 0 <= k < i ==> #[trigger] self.shutdowns.gui[k].provider@ != target@,
            decreases self.shutdowns.gui.len() - i,
        {
            let gui = &self.shutdowns.gui[i];
            if gui.provider.as_str() == target.as_str() {
                if !observation.permits(target.as_str(), gui.clients.as_slice(), true) { return None; }
                assert(self@.shutdown.gui[i as int].provider@ == target@);
                assert(shutdown_mode(self@.shutdown, target@, true, gui.clients@));
                proof {
                    if self@.shutdown.targets.reviewed && shutdown_valid(self@.shutdown) {
                        crate::inventory::approved_clients(observation@, target@, gui.clients@, true,
                            self@.shutdown.targets.allowed);
                    }
                }
                return Some(ShutdownRequest { target, gui: Some(gui), _publication: publication, _execution: self });
            }
            i += 1;
        }

        let clients = Vec::new();
        if !observation.permits(target.as_str(), clients.as_slice(), false) { return None; }
        proof {
            crate::inventory::approved_clients(observation@, target@, clients@, false,
                self@.shutdown.targets.allowed);
        }
        Some(ShutdownRequest { target, gui: None, _publication: publication, _execution: self })
    }
}

impl<'a> ShutdownRequest<'a> {
    /// Construct the complete fixed Qubes argv, including the checked force mode and sole target.
    pub fn arguments(&self) -> (args: Vec<String>)
        ensures
            args.len() == (if self@.forced { 5usize } else { 4usize }),
            self@.forced ==> args[0]@ == "--force"@ && args[1]@ == "--wait"@
                && args[2]@ == "--timeout"@ && args[3]@ == "120"@ && args[4]@ == self@.target,
            !self@.forced ==> args[0]@ == "--wait"@ && args[1]@ == "--timeout"@
                && args[2]@ == "120"@ && args[3]@ == self@.target,
    {
        let mut args = Vec::new();
        if self.forced() { args.push("--force".to_string()); }
        args.push("--wait".to_string());
        args.push("--timeout".to_string());
        args.push("120".to_string());
        args.push(self.target.as_str().to_string());
        args
    }

    /// Borrow the exact checked name as the sole positional VM argument.
    pub fn target(&self) -> (target: &str)
        ensures target@ == self@.target,
    { self.target.as_str() }

    /// Return the checked GUI-preservation mode used to select the fixed --force argument.
    pub fn forced(&self) -> (forced: bool)
        ensures forced == self@.forced,
    { self.gui.is_some() }
}

impl<'a> StartRequest<'a> {
    /// The Qubes adapter uses this exact checked name as its sole positional VM argument.
    pub fn target(&self) -> (target: &str)
        ensures target@ == self@.target,
    {
        self.target.as_str()
    }
}

}
