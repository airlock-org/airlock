//! Validated authority is an owned value required to create a publication gate.
//! Decoded journal identities are evidence, never executable authority by themselves.
//!
//! ## Production execution paths
//!
//! The production [planner adapter](../../machine/src/planning.rs) uses
//! `airlock-machine-planning` to interpret topology, derive deployment seeds and order
//! VM effects. That crate has no dependency on deployment approval or publication.
//! This crate depends on its public contracts to compose planning with authorization.
//!
//! [`name_targets`](../../machine-planning/src/graph.rs) verifies the exact string contents copied from computed vertex
//! indices. [`approved_schedule`](crate::planning) composes that mapping and exact selection with the
//! existing checked scope: every selected operation needs original approval.
//! The coordinator checks the complete proposed effect set before creating owned
//! lifecycle authority; graph reachability cannot grant execution permission.
//! Recovery and rollback use the same seed selector and kernel with reconstructed
//! inputs and then reauthorize. Recovery adds saved startup targets to its running
//! input and filters stops against a fresh live snapshot; those reconstruction and
//! filtering steps remain unverified coordinator code. Existing operation, attempt
//! and plan bindings prevent substitution
//! within an executing attempt. Recovery endpoint selection and its handoff to the
//! owned operation are verified; observation reconstruction and external progress
//! remain outside the proof. Different attempts may replan within the original scope.
//!
//! | Path | Enforcement and proof boundary |
//! | --- | --- |
//! | `relationships.rs` → `machine-planning/src/topology.rs`: reference interpretation and canonical indexing | Verified effective defaults/overrides, reference grammar, exact internal edge relation, canonical universe, unique identities, name/mask correspondence and named planner calls. JSON/typed-field decoding and conversion to/from ordinary `GuestName` collections remain tested adapters. External relationships remain visible to admission. |
//! | `machine-planning/src/graph.rs`: closure and lifecycle operation lists | Verified least forward/reverse closures, exact startup effects, affected/running shutdown selection, dependency/reverse ordering and cycle rejection. Startup uses candidate edges; impact uses baseline edges with eligible soft GUI edges removed. Change/removal/autostart roots, baseline GUI eligibility and their named graph handoffs are verified in `machine-planning/src/deployment.rs`. The adapter still chooses baseline/candidate inputs and decodes configuration and observations. |
//! | `machine-planning/src/deployment.rs`: seed selection and GUI preservation | Verified exact change/removal/startup roots, configuration identity coverage, GUI opt-in/provider/hard-edge conditions, exact reconnect selection, cached impact closure and input-order-independent root sets. Production deployment planning rejects unsupported power states; inventory parsing/completeness and canonical configuration encoding remain tested adapters. |
//! | Complete plan effect projection and authorization | Verified exact VM-impact union, unknown-field rejection, required/unknown reboot permission, full scope and identity check, exact lifecycle-list derivation and retained plan binding through fresh execution/recovery. Wire decoding/classification, staging/retirement/copy/PCI selection and complete-plan serialization remain adapter obligations; no whole-plan correctness/determinism theorem. |
//! | `recovery.rs`: durable evidence → `RecoveryDecision` → `AuthorizedOperation` | Verified evidence compatibility, exact direction/endpoint selection, unchanged retained intent and V2 authority, stable operation identity, fresh checked scopes and new publication. JSON/record authentication, snapshot projection, runtime reconstruction and actual effect completion remain adapter obligations. |
//! | Pre-retirement/configuration power check | Verified disjointness decision for required targets and fresh active observations. Hazard-set extraction, inventory completeness and absence of changes after observation remain trusted. |
//! | `activation.rs`: explicit starts after configuration, including resume and rollback | Verified target selection and identity/scope checks; opaque request passed to `Runner::start_deployment`. The narrow process adapter remains trusted. |
//! | `activation.rs`: explicit shutdowns, including resume and rollback | Verified target, GUI mode, expected-client scope, observed-client bounds, identity and gate checks; verified argv passed to `Runner::shutdown_deployment`. |
//! | Forced GUI shutdown | Only a checked GUI-preservation entry supplies force. Verified power-state classification and complete row checking require all active GUI clients to fit the approved expected set. Missing probes, uncertain states and hard/undeclared active dependents block execution. Inventory decoding, truthful Qubes observations and baseline reference extraction remain trusted. |
//! | Direct kill | Deployment issues no separate kill request. Raw `qvm-kill` is rejected; Qubes' shutdown helper can kill the same checked target on timeout. |
//! | Generic `Runner::run`, `capture`, `text`, `stream` during deployment | Reject direct `qvm-start`, `qvm-shutdown`, `qvm-kill` and `qvm-create`, including absolute paths. This tested adapter guard prevents accidental raw-call bypass; it does not confine arbitrary helpers, aliases or shell commands. |
//! | VM creation and declared properties/features/services | Verified `ConfigurationPlan` namespace, retained definitions, durable context, exact target/key/value/tool and argv; typed requests reach thin Qubes adapters. JSON decoding, reconciliation orchestration, observations and external CLI semantics are trusted. |
//! | `activation.rs`: host roots/controller, boot preparation and selection | Verified `HostPlan`/`HostRequest` preserve exact admitted host/transport/controller, prepared-view policy and fixed native/Nix argv under the executing gate. Local retention paths, controller symlink publication and external filesystem/storage/boot effects remain trusted. Selection/status files are transaction metadata. |
//! | Retained setup sections | Checked helper modes/paths and complete configuration namespace; global setup, backend-derived properties, storage and device dispatch remain trusted adapters. |
//! | `migration.rs`: private copies | Verified original ordered endpoints, preflight configuration/resource bindings, monotonic receipt progress, stopped/capacity checks, durable context and exact clone request names. Completed and uncertain receipts never request an overwrite. Qubes and copy-publication acknowledgments remain trusted. |
//! | Reversible retirement/restoration | Verified checked namespaces/configuration, canonical trash identity, named UUID/storage/power observations, retire-only selected generation/system/manifest checks, no purge states and fixed tag/autostart/retention argv. Baseline pin requests are target/attempt-bound. Observation decoding, storage identity hashing, persistence and faithful execution remain trusted. |
//! | PCI handoff and explicit purge | Handoff retains runtime scope/ownership checks; purge has independent maintenance authority and runtime reference/UUID/volume/expiry safeguards. Neither has fine-grained verified request dispatch. |
//! | `admission.rs` and storage adapters: artifact admission and generation publication | Existing storage contracts and checks, outside this effect proof. |
//! | `boot.rs::boot_start`, trash enforcement, development startup | Separate boot/maintenance/development policy; no deployment-review theorem. |
//! | Qubes Manager Admin API | Existing `sys-gui` policy separately permits `admin.vm.Shutdown` and `admin.vm.Kill`. Those user administration requests do not pass through deployment authority or its machine lock. |
//! | RPC prepared apply and CLI recovery | Route into the coordinator activation path. Routing and prepared-record interpretation are tested code, not Verus proofs. |
//!
//! Qubes can start dependencies implicitly. The planner expands declared startup
//! dependencies; regression coverage checks that such expansion still needs approval.
//! References outside the declared topology, live dependency drift and Qubes/Xen's
//! actual effects are outside the explicit-request theorem. No claim covers all
//! privileged operations in dom0 or all effects of starting a VM.
//!
//! The pinned [Qubes client v4.3.30 shutdown helper](https://github.com/QubesOS/qubes-core-admin-client/blob/v4.3.30/qubesadmin/tools/qvm_shutdown.py)
//! uses `--force` to bypass dependent-VM protection. With the existing
//! `--wait --timeout 120` arguments it can kill the same explicit target after timeout.
//! The [Qubes GUI handler v4.3.44](https://github.com/QubesOS/qubes-core-admin/blob/v4.3.44/qubes/ext/gui.py)
//! normally prevents stopping a provider with running GUI clients; force bypasses
//! that protection without explicitly shutting those clients down. Airlock checks
//! their expected GUI interruption and subsequently verifies preserved boot IDs and
//! GUI reconnection. Verus does not prove client survival or faithful Xen/Qubes effects.

use crate::{
    effects::{
        self, Execution, ShutdownObservation, ShutdownRequest, Shutdowns, StartRequest, Targets,
    },
    publication::{Event, Publication},
    review,
};
use vstd::prelude::*;

// These wrappers distinguish identity roles without duplicating wire-format parsing.
// Their constructors classify authenticated adapter inputs; they do not authenticate them.
macro_rules! identity {
    ($name:ident, $description:literal) => {
        verus! {
            #[doc = $description]
            pub struct $name {
                // Canonical contents supplied by the record adapter for this identity role.
                value: String,
            }

            impl View for $name {
                type V = Seq<char>;

                closed spec fn view(&self) -> Seq<char> { self.value@ }
            }

            impl $name {
                /// Classify a decoded identity; this does not grant execution authority.
                pub fn new(value: String) -> (id: Self)
                    ensures id@ == value@,
                { Self { value } }

                /// Borrow the canonical contents at a serialization or comparison boundary.
                pub fn as_str(&self) -> (value: &str)
                    ensures value@ == self@,
                { self.value.as_str() }
            }
        }
    };
}

identity!(
    AttemptId,
    "One execution attempt; recovery allocates a new attempt."
);

identity!(
    OperationId,
    "The authorised operation identity, preserved across recovery attempts."
);

identity!(
    AuthorityBinding,
    "Canonical protected authority, including the decision and candidate fingerprint."
);

identity!(
    PlanBinding,
    "Digest binding the candidate, baseline and complete proposed effect plan."
);

identity!(
    IntentBinding,
    "Digest of the complete preflight journal whose publication is acknowledged."
);

verus! {

/// Immutable identity evidence re-extracted from the journal at a privileged request.
pub struct ExecutionContext {
    /// Current attempt, rather than a previous recovery attempt.
    pub attempt: AttemptId,
    /// Stable operation retained by recovery.
    pub operation: OperationId,
    /// Original protected authority encoding.
    pub authority: AuthorityBinding,
    /// Exact candidate, baseline, revision and complete plan digest.
    pub plan: PlanBinding,
}

/// Evidence decoded from a previous journal. Constructing it grants no permission.
pub struct PreviousOperation {
    /// Stable identity recorded by the previous attempt.
    pub id: OperationId,
    /// Exact authority to which that identity was bound.
    pub authority: AuthorityBinding,
}

/// Proof view shared by the owned authority and its durable successor.
pub ghost struct AuthorizedState {
    /// Current attempt, distinct from the stable operation.
    pub attempt: Seq<char>,
    /// Stable identity selected by the checked recovery rule.
    pub operation: Seq<char>,
    /// Authority whose equality was checked on recovery.
    pub authority: Seq<char>,
    /// Original candidate/baseline/plan binding.
    pub plan: Seq<char>,
    /// Checked startup subset and original human scope.
    pub scope: effects::Scope,
    /// Checked shutdown targets and exact GUI-preservation modes.
    pub shutdown: effects::ShutdownScope,
}

/// Opaque, non-cloneable authority. Only checked identity selection can construct it.
/// Publication consumes this value; a deserialized journal cannot replace it.
///
/// ```compile_fail
/// use airlock_deployment_core::operation::{AuthorizedOperation, PreviousOperation};
/// // Publication adapters require validated authority, not decoded evidence.
/// fn publish(_: AuthorizedOperation) {}
/// // A journal cannot bypass the checked constructor through a type conversion.
/// fn bypass(saved: PreviousOperation) { publish(saved); }
/// ```
///
/// ```compile_fail
/// use airlock_deployment_core::operation::{AttemptId, PreviousOperation, AuthorityBinding};
/// // An attempt cannot accidentally stand in for the stable operation identity.
/// fn evidence(attempt: AttemptId, authority: AuthorityBinding) -> PreviousOperation {
///     PreviousOperation { id: attempt, authority }
/// }
/// ```
pub struct AuthorizedOperation {
    // Checked subset of the reviewed plan, or an explicitly exempt plan.
    starts: Targets,
    // Independently checked shutdown targets and affected GUI clients.
    shutdowns: Shutdowns,
    // New attempt identity, allocated before publication.
    attempt: AttemptId,
    // Stable identity selected by the mandatory verified recovery check.
    operation: OperationId,
    // Original protected authority used for the recovery equality check.
    authority: AuthorityBinding,
    // Candidate and baseline plan validated before intent publication.
    plan: PlanBinding,
}

impl View for AuthorizedOperation {
    type V = AuthorizedState;

    closed spec fn view(&self) -> AuthorizedState {
        AuthorizedState { attempt: self.attempt@, operation: self.operation@,
            authority: self.authority@, plan: self.plan@, scope: self.starts@, shutdown: self.shutdowns@ }
    }
}

impl AuthorizedOperation {
    /// Consume checked lifecycle scopes and select their stable operation identity.
    /// Conflicting retained authority yields no value, so publication cannot proceed.
    pub fn new(
        starts: Targets, shutdowns: Shutdowns, attempt: AttemptId, authority: AuthorityBinding, plan: PlanBinding,
        previous: Option<&PreviousOperation>, legacy: Option<&OperationId>,
    ) -> (result: Option<Self>)
        ensures
            result.is_some() == match previous {
                Some(saved) => saved.authority@ == authority@,
                None => true,
            },
            result.is_some() ==> result.unwrap()@.operation == match previous {
                Some(saved) => saved.id@,
                None => match legacy { Some(id) => id@, None => attempt@ },
            },
            result.is_some() ==> result.unwrap()@.attempt == attempt@
                && result.unwrap()@.authority == authority@ && result.unwrap()@.plan == plan@
                && result.unwrap()@.scope == starts@ && result.unwrap()@.shutdown == shutdowns@,
    {
        let saved = previous.map(|saved: &PreviousOperation| -> (pair: (&str, &str))
            ensures pair.0@ == saved.id@ && pair.1@ == saved.authority@,
        { (saved.id.as_str(), saved.authority.as_str()) });
        let legacy_id = legacy.map(|id: &OperationId| -> (value: &str)
            ensures value@ == id@,
        { id.as_str() });
        let selected = review::operation_id(attempt.as_str(), legacy_id, saved, authority.as_str())?;
        let id = OperationId::new(selected.to_string());

        Some(Self { starts, shutdowns, attempt, authority, plan, operation: id })
    }

    /// The only operation identity the coordinator may put in this intent's wire record.
    pub fn id(&self) -> (id: &OperationId)
        ensures id@ == self@.operation,
    { &self.operation }

    /// Consume authority only if the proposed journal still carries its exact bindings.
    /// No publication gate is returned for an altered attempt, operation, authority or plan.
    pub fn publication(
        self, attempt: &AttemptId, operation: &OperationId, authority: &AuthorityBinding,
        plan: &PlanBinding, intent: IntentBinding,
    ) -> (result: Option<DurableOperation>)
        ensures
            result.is_some() == (self@.attempt == attempt@ && self@.operation == operation@
                && self@.authority == authority@ && self@.plan == plan@),
            result.is_some() ==> result.unwrap()@.authority == self@,
            result.is_some() ==> result.unwrap()@.gate.stage is Unpublished,
            result.is_some() ==> result.unwrap()@.gate.binding == intent@,
            result.is_some() ==> consistent(result.unwrap()@),
    {
        if self.attempt.as_str() != attempt.as_str() || self.operation.as_str() != operation.as_str()
            || self.authority.as_str() != authority.as_str() || self.plan.as_str() != plan.as_str() {
            return None;
        }

        let gate = Publication::new(self.attempt.as_str().to_string(), intent.as_str().to_string());
        let execution = self.starts.bind(self.shutdowns, self.attempt.as_str().to_string(),
            intent.as_str().to_string(), self.plan.as_str().to_string());

        Some(DurableOperation { gate, execution, operation: self.operation, authority: self.authority })
    }
}

/// Publication state plus the unchanged operation, plan and checked scope.
pub ghost struct DurableState {
    /// Immutable authority carried from the checked constructor.
    pub authority: AuthorizedState,
    /// Volatile acknowledgments and phase for this attempt.
    pub gate: crate::publication::State,
    /// Internal execution binding used to construct exact requests.
    pub execution: effects::ExecutionState,
}

/// Exact conditions shared by all privileged request families.
pub open spec fn executing(durable: &DurableOperation, context: &ExecutionContext) -> bool {
    &&& durable@.gate.stage is Executing && durable@.gate.intent && durable@.gate.approval
    &&& context.attempt@ == durable@.authority.attempt
    &&& context.operation@ == durable@.authority.operation
    &&& context.authority@ == durable@.authority.authority
    &&& context.plan@ == durable@.authority.plan
    &&& durable@.gate.attempt == durable@.authority.attempt
    &&& durable@.gate.binding == durable@.execution.intent
}

/// Internal gate and execution selector describe the same attempt and intent.
pub open spec fn consistent(s: DurableState) -> bool {
    &&& crate::publication::valid(s.gate)
    &&& s.gate.attempt == s.authority.attempt
    &&& s.execution.attempt == s.authority.attempt
    &&& s.execution.intent == s.gate.binding
    &&& s.execution.context == s.authority.plan
    &&& s.execution.scope == s.authority.scope
    &&& s.execution.shutdown == s.authority.shutdown
}

/// A gate constructible only by consuming validated authority, never from a wire record.
///
/// ```compile_fail
/// use airlock_deployment_core::{operation::*, publication::Event};
/// // A request keeps its gate borrowed until the adapter has consumed the target.
/// fn interrupted_request(gate: &mut DurableOperation, attempt: &AttemptId,
///     operation: &OperationId, authority: &AuthorityBinding, plan: &PlanBinding) {
///     let request = gate.request(attempt, operation, authority, plan, "usr-home".into()).unwrap();
///     gate.advance(Event::Interrupted);
///     println!("{target}", target = request.target());
/// }
/// ```
///
/// ```compile_fail
/// use airlock_deployment_core::{effects::ShutdownObservation, operation::*, publication::Event};
/// // Shutdown requests keep the same gate borrowed until their arguments are dispatched.
/// fn interrupted_shutdown(gate: &mut DurableOperation, attempt: &AttemptId,
///     operation: &OperationId, authority: &AuthorityBinding, plan: &PlanBinding,
///     observed: &ShutdownObservation) {
///     let request = gate.shutdown(attempt, operation, authority, plan, "usr-home".into(), observed).unwrap();
///     gate.advance(Event::Interrupted);
///     let _ = request.arguments();
/// }
/// ```
pub struct DurableOperation {
    // Truthful I/O acknowledgments drive the existing verified publication machine.
    gate: Publication,
    // Bound startup list; callers cannot replace its plan, identity or scope.
    execution: Execution,
    // Stable identity that each request must still match.
    operation: OperationId,
    // Exact protected authority that each request must still match.
    authority: AuthorityBinding,
}

impl View for DurableOperation {
    type V = DurableState;

    closed spec fn view(&self) -> DurableState {
        DurableState { authority: AuthorizedState { attempt: self.execution@.attempt,
            operation: self.operation@, authority: self.authority@,
            plan: self.execution@.context, scope: self.execution@.scope, shutdown: self.execution@.shutdown },
            gate: self.gate@, execution: self.execution@ }
    }
}

impl DurableOperation {
    /// Admit a new request family under the same exact executing context as lifecycle requests.
    pub fn allows_effect(&self, context: &ExecutionContext) -> (accepted: bool)
        ensures accepted == executing(self, context),
    {
        self.operation.as_str() == context.operation.as_str()
            && self.authority.as_str() == context.authority.as_str()
            && self.execution.permits(&self.gate, context.attempt.as_str(), context.plan.as_str())
    }

    /// Record durable I/O or completion observations without changing authority or scope.
    /// Raw Begin events fail closed; only the identity-checking begin method enters execution.
    pub fn advance(&mut self, event: Event)
        ensures
            final(self)@.authority == old(self)@.authority,
            final(self)@.gate == crate::publication::next(old(self)@.gate,
                if event is Begin { Event::Interrupted } else { event }),
            event is Begin ==> final(self)@.gate.stage is Blocked,
            consistent(old(self)@) ==> consistent(final(self)@),
    {
        self.gate.advance(match event { Event::Begin => Event::Interrupted, other => other });
    }

    /// Test readiness against the exact attempt and complete preflight record.
    pub fn permits(&self, attempt: &AttemptId, intent: &IntentBinding) -> (allowed: bool)
        ensures allowed == (self@.gate.stage is Ready && self@.gate.intent && self@.gate.approval
            && self@.gate.attempt == attempt@ && self@.gate.binding == intent@),
    { self.gate.permits(attempt.as_str(), intent.as_str()) }

    /// Consume readiness; failure or a duplicate begin permanently blocks this gate.
    pub fn begin(&mut self, attempt: &AttemptId, intent: &IntentBinding) -> (allowed: bool)
        ensures
            allowed == (old(self)@.gate.stage is Ready && old(self)@.gate.intent && old(self)@.gate.approval
                && old(self)@.gate.attempt == attempt@ && old(self)@.gate.binding == intent@),
            final(self)@.authority == old(self)@.authority,
            allowed ==> final(self)@.gate.stage is Executing,
            !allowed ==> final(self)@.gate.stage is Blocked,
            consistent(old(self)@) ==> consistent(final(self)@),
    { self.gate.begin(attempt.as_str(), intent.as_str()) }

    /// Admit a request only for this unchanged operation, executing intent and planned VM.
    pub fn request<'a>(
        &'a self, attempt: &AttemptId, operation: &OperationId, authority: &AuthorityBinding,
        plan: &PlanBinding, target: String,
    ) -> (result: Option<StartRequest<'a>>)
        ensures
            result.is_some() ==> operation@ == self@.authority.operation
                && authority@ == self@.authority.authority
                && attempt@ == self@.authority.attempt && plan@ == self@.authority.plan,
            result.is_some() ==> self@.gate.stage is Executing
                && self@.gate.intent && self@.gate.approval,
            result.is_some() ==> effects::planned(self@.authority.scope, target@),
            result.is_some() && self@.authority.scope.reviewed && effects::valid(self@.authority.scope)
                ==> self@.authority.scope.allowed.contains(target@),
            result.is_some() ==> result.unwrap()@.target == target@,
    {
        if self.operation.as_str() != operation.as_str() || self.authority.as_str() != authority.as_str() {
            return None;
        }

        self.execution.request(&self.gate, attempt.as_str(), plan.as_str(), target)
    }

    /// Select the exact shutdown target and mode under the same authority used for starts.
    pub fn shutdown<'a>(
        &'a self, attempt: &AttemptId, operation: &OperationId, authority: &AuthorityBinding,
        plan: &PlanBinding, target: String, observation: &ShutdownObservation,
    ) -> (result: Option<ShutdownRequest<'a>>)
        ensures
            result.is_some() ==> operation@ == self@.authority.operation && authority@ == self@.authority.authority
                && attempt@ == self@.authority.attempt && plan@ == self@.authority.plan,
            result.is_some() ==> self@.gate.stage is Executing && self@.gate.intent && self@.gate.approval,
            result.is_some() ==> effects::planned(self@.authority.shutdown.targets, target@),
            result.is_some() ==> result.unwrap()@.target == target@,
            result.is_some() ==> result.unwrap()@.execution == self@.execution
                && result.unwrap()@.publication == self@.gate,
            result.is_some() ==> effects::shutdown_mode(self@.authority.shutdown, target@,
                result.unwrap()@.forced, result.unwrap()@.clients),
            result.is_some() ==> crate::inventory::permitted(observation@, target@,
                result.unwrap()@.clients, result.unwrap()@.forced),
            result.is_some() && self@.authority.shutdown.targets.reviewed && effects::shutdown_valid(self@.authority.shutdown) ==>
                crate::inventory::clients_within(observation@, target@, self@.authority.shutdown.targets.allowed),
            result.is_some() && self@.authority.shutdown.targets.reviewed && effects::shutdown_valid(self@.authority.shutdown) ==>
                self@.authority.shutdown.targets.allowed.contains(target@)
                && forall|j: int| 0 <= j < result.unwrap()@.clients.len() ==>
                    self@.authority.shutdown.targets.allowed.contains(#[trigger] result.unwrap()@.clients[j]@),
    {
        if self.operation.as_str() != operation.as_str() || self.authority.as_str() != authority.as_str() { return None; }

        self.execution.shutdown(&self.gate, attempt.as_str(), plan.as_str(), target, observation)
    }
}

}
