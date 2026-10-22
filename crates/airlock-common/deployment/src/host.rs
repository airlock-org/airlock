//! Fix host and controller execution parameters before durable publication.
//! The admitted release decoder supplies artifacts; requests cannot substitute them afterward.
//!
//! ## Host preparation and selection
//!
//! The [host boundary](crate::host) retains the admitted candidate artifacts and native
//! storage adapter before durable publication. The approved complete plan supplies its
//! binding; the release adapter is trusted to decode those artifacts faithfully. Host
//! scope comes from the approved source and reboot permission, rather than a fabricated
//! `dom0` VM entry. This does not prove Rusix evaluation, build provenance or admission.
//!
//! Each request checks the original operation, authority, attempt and complete plan against
//! the executing durable gate. Host and transport pinning, controller pin/link publication,
//! profile selection and boot/storage operations have distinct fixed actions. Exhaustive
//! matches require new actions to be classified. Prepared host views allow validation;
//! transport-supplied views allow publication. CLI arguments and the retained executable
//! are selected by verified Rust, not reconstructed from current environment or raw journal
//! fields at dispatch. The recovery path retains the existing running-controller policy.
//!
//! Publication/validation returns a typed host record. `HostRequest::checked` verifies its
//! immutable system, positive canonical generation and complete record digest against the
//! retained prepared record, when present. The opaque `CheckedGeneration` also retains the
//! attempt, operation, original approval and complete plan. Profile, installation, sealing,
//! machine binding and selection requests require that result and the current executing gate;
//! ordinary `request` cannot construct those actions. Prepared recovery revalidates the exact
//! original record; an old attempt's token cannot be reused by a new attempt. Publication-only
//! releases without a prepared record capture the validated generation anew for each attempt.
//!
//! Verified argv carries the checked system, generation and record digest to storage actions.
//! The storage adapter revalidates that identity under its pool lock before mutation. Isolated
//! installation uses verified argument construction to retain identity and pool/ESP paths across the mount child; a
//! missing child expectation requires an explicit administrative mode. Profile changes target
//! the immutable host system rather than a pool generation. Installation rejects a different
//! copied system profile and pins its private profile directly to the requested host, avoiding
//! mutable global profile indirection. Protect/checkpoint intentionally
//! inspect the running baseline, while sealing may also preserve other retained boot entries.
//!
//! Wire decoding, canonical positive generation/string projection, complete-record SHA-256,
//! cooperative pool/machine locks and faithful file/process execution remain trusted. Hashes
//! exclude diagnostic timing and retain unknown record extensions. This proves authorised
//! requests and checked identity comparisons, not physical storage/firmware behavior, global
//! atomicity, durable receipt publication or exactly-once effects. Host machine bindings still
//! read the protected live selection and guest generation records in ordinary Rust. Independent
//! recovery and maintenance boot repair retain their explicit administrative policies.

mod receipt;

pub use receipt::{CheckedGeneration, Observation, equal as same_observation, isolated_arguments};

use crate::operation::{DurableOperation, ExecutionContext, PlanBinding};
use vstd::prelude::*;

verus! {

/// Exact artifacts and native storage adapter captured from the admitted execution context.
pub struct Definition {
    /// Prepared host record retained from the immutable release; absent before publication.
    pub expected: Option<Observation>,
    /// Approved candidate's complete host system.
    pub host: String,
    /// Retained transport closure, which may differ from the host system.
    pub transport: String,
    /// Selected native controller; recovery may retain the running coordinator.
    pub controller: String,
    /// Controller's containing store object, or empty for an explicit non-store runtime.
    pub controller_root: String,
    /// Native storage service selected before publication, never supplied at dispatch.
    pub service: String,
}

/// Host preparation and selection are distinct operations with fixed command semantics.
#[derive(Clone, Copy)]
pub enum Action {
    /// Keep the exact host closure reachable from the transaction root.
    PinHost,
    /// Keep the exact transport closure reachable.
    PinTransport,
    /// Keep the selected controller's immutable store object reachable.
    PinController,
    /// Install the selected controller link; the filesystem adapter handles atomic publication.
    InstallController,
    /// Publish the admitted host view.
    Publish,
    /// Validate a host view already admitted by the transport.
    Validate,
    /// Preserve the running host's boot artifacts before preparing its successor.
    ProtectBoot,
    /// Set the system profile to the retained candidate host.
    Profile,
    /// Install boot artifacts for the exact candidate host.
    InstallBoot,
    /// Seal boot artifacts while retaining the default boot entry.
    SealBoot,
    /// Save recovery state of the currently running machine.
    Checkpoint,
    /// Bind machine recovery state to the exact candidate host.
    Bind,
    /// Select the exact installed host without rebooting.
    Select,
}

/// An admitted host is validated; a transport-supplied view is published exactly once per request.
pub open spec fn allowed(d: Definition, action: Action) -> bool {
    match action { Action::Publish => d.expected.is_none(), Action::Validate => d.expected.is_some(),
        Action::PinHost | Action::PinTransport | Action::PinController | Action::InstallController
        | Action::ProtectBoot | Action::Profile | Action::InstallBoot | Action::SealBoot
        | Action::Checkpoint | Action::Bind | Action::Select => true }
}

/// Original host definition bound to the complete approved candidate, baseline and plan.
pub struct HostPlan {
    // Artifact names are immutable after the checked plan captures them.
    definition: Definition,
    // Complete source/candidate/baseline/plan digest inherited from AuthorizedPlan.
    binding: PlanBinding,
}

impl View for HostPlan {
    type V = (Definition, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.definition, self.binding@) }
}

impl HostPlan {
    /// Only the checked deployment plan can retain host artifacts for execution.
    pub(crate) fn new(definition: Definition, binding: PlanBinding) -> (plan: Self)
        ensures plan@ == (definition, binding@),
    { Self { definition, binding } }

    /// No action is executable with a replaced candidate/baseline/plan or a nonexecuting gate.
    pub fn request<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext, action: Action)
        -> (out: Option<HostRequest<'a>>)
        ensures out.is_some() == (self@.1 == context.plan@ && crate::operation::executing(durable, context) && allowed(self@.0, action) && !receipt::required(action)),
            out.is_some() ==> out.unwrap()@ == (self@.0, action, context.attempt@, context.operation@, context.authority@, context.plan@, None),
    {
        if self.binding.as_str() != context.plan.as_str() || !durable.allows_effect(context)
            || receipt::needs_receipt(action) { return None; }
        if matches!(action, Action::Publish) && self.definition.expected.is_some() { return None; }
        if matches!(action, Action::Validate) && self.definition.expected.is_none() { return None; }
        Some(HostRequest { definition: &self.definition, action, attempt: context.attempt.as_str().to_string(),
            operation: context.operation.as_str().to_string(), authority: context.authority.as_str().to_string(),
            plan: context.plan.as_str().to_string(), checked: None, _durable: durable })
    }
}

/// Host adapters accept this value instead of separate operation and artifact parameters.
pub struct HostRequest<'a> {
    // Immutable admitted host/controller/transport definition.
    definition: &'a Definition,
    // Fixed privileged operation selected under the executing gate.
    action: Action,
    // Verified attempt used to choose transaction-local retention roots.
    attempt: String,
    // Stable operation retained by this request's original execution context.
    operation: String,
    // Original source approval and candidate identity.
    authority: String,
    // Full candidate/baseline/plan binding.
    plan: String,
    // Required for candidate profile, boot preparation, binding and selection.
    checked: Option<&'a CheckedGeneration>,
    // Keep the authorised durable context borrowed throughout execution.
    _durable: &'a DurableOperation,
}

impl<'a> View for HostRequest<'a> {
    type V = (Definition, Action, Seq<char>, Seq<char>, Seq<char>, Seq<char>, Option<Observation>);

    closed spec fn view(&self) -> Self::V { (*self.definition, self.action, self.attempt@, self.operation@, self.authority@, self.plan@,
        match self.checked { Some(receipt) => Some(receipt@.0), None => None }) }
}

/// Exact artifact for each operation; host selection cannot become transport/controller selection.
pub open spec fn target(d: Definition, action: Action) -> Seq<char> {
    match action {
        Action::PinTransport => d.transport@,
        Action::PinController => d.controller_root@,
        Action::InstallController => d.controller@,
        Action::PinHost | Action::Publish | Action::Validate | Action::ProtectBoot | Action::Profile
        | Action::InstallBoot | Action::SealBoot | Action::Checkpoint | Action::Bind | Action::Select => d.host@,
    }
}

impl<'a> HostRequest<'a> {
    /// Borrow the exact executing attempt for transaction-local retention paths.
    pub fn attempt(&self) -> (attempt: &str)
        ensures attempt@ == self@.2,
    { self.attempt.as_str() }

    /// Determine the sole permitted artifact for this action.
    pub fn target(&self) -> (value: &str)
        ensures value@ == target(self@.0, self@.1),
    {
        match self.action { Action::PinTransport => self.definition.transport.as_str(),
            Action::PinController => self.definition.controller_root.as_str(),
            Action::InstallController => self.definition.controller.as_str(),
            Action::PinHost | Action::Publish | Action::Validate | Action::ProtectBoot | Action::Profile
            | Action::InstallBoot | Action::SealBoot | Action::Checkpoint | Action::Bind | Action::Select => self.definition.host.as_str() }
    }

    /// Borrow the fixed action for an external filesystem adapter.
    pub fn action(&self) -> (action: Action)
        ensures action == self@.1,
    { self.action }

    /// Select the native adapter or upstream Nix tool without a dispatch-time program override.
    pub fn program(&self) -> (program: &str)
        ensures program@ == match self@.1 {
            Action::PinHost | Action::PinTransport | Action::PinController => "nix-store"@,
            Action::Profile => "nix-env"@,
            Action::InstallController | Action::Publish | Action::Validate | Action::ProtectBoot
            | Action::InstallBoot | Action::SealBoot | Action::Checkpoint | Action::Bind | Action::Select => self@.0.service@,
        },
    {
        match self.action {
            Action::PinHost | Action::PinTransport | Action::PinController => "nix-store",
            Action::Profile => "nix-env",
            Action::InstallController | Action::Publish | Action::Validate | Action::ProtectBoot
            | Action::InstallBoot | Action::SealBoot | Action::Checkpoint | Action::Bind | Action::Select => self.definition.service.as_str(),
        }
    }

    /// Construct the exact privileged command; root is an adapter-owned local retention path.
    fn base_arguments(&self, root: &str) -> (args: Vec<String>)
        ensures
            (self@.1 is PinHost || self@.1 is PinTransport || self@.1 is PinController) ==>
                args.len() == 4 && args[0]@ == "--add-root"@ && args[1]@ == root@
                    && args[2]@ == "--realise"@ && args[3]@ == target(self@.0, self@.1),
            self@.1 is Profile ==> args.len() == 4 && args[0]@ == "-p"@
                && args[1]@ == "/nix/var/nix/profiles/system"@ && args[2]@ == "--set"@ && args[3]@ == self@.0.host@,
            self@.1 is InstallController ==> args.len() == 0,
            (self@.1 is Publish || self@.1 is Validate || self@.1 is InstallBoot || self@.1 is Bind || self@.1 is Select) ==>
                args.len() == 4 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[3]@ == self@.0.host@,
            self@.1 is Publish ==> args[2]@ == "publish"@,
            self@.1 is Validate ==> args[2]@ == "validate"@,
            self@.1 is InstallBoot ==> args[2]@ == "install-boot"@,
            self@.1 is Bind ==> args[2]@ == "bind-machine"@,
            self@.1 is Select ==> args[2]@ == "select"@,
            self@.1 is ProtectBoot ==> args.len() == 3 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "protect-boot"@,
            self@.1 is SealBoot ==> args.len() == 4 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "seal-boot"@ && args[3]@ == "--keep-default"@,
            self@.1 is Checkpoint ==> args.len() == 3 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "checkpoint-machine"@,
    {
        match self.action {
            Action::PinHost | Action::PinTransport | Action::PinController => vec!["--add-root".to_string(), root.to_string(), "--realise".to_string(), self.target().to_string()],
            Action::Profile => vec!["-p".to_string(), "/nix/var/nix/profiles/system".to_string(), "--set".to_string(), self.definition.host.as_str().to_string()],
            Action::InstallController => Vec::new(),
            Action::Publish => vec!["storage".to_string(), "host".to_string(), "publish".to_string(), self.definition.host.as_str().to_string()],
            Action::Validate => vec!["storage".to_string(), "host".to_string(), "validate".to_string(), self.definition.host.as_str().to_string()],
            Action::InstallBoot => vec!["storage".to_string(), "host".to_string(), "install-boot".to_string(), self.definition.host.as_str().to_string()],
            Action::Bind => vec!["storage".to_string(), "host".to_string(), "bind-machine".to_string(), self.definition.host.as_str().to_string()],
            Action::Select => vec!["storage".to_string(), "host".to_string(), "select".to_string(), self.definition.host.as_str().to_string()],
            Action::ProtectBoot => vec!["storage".to_string(), "host".to_string(), "protect-boot".to_string()],
            Action::SealBoot => vec!["storage".to_string(), "host".to_string(), "seal-boot".to_string(), "--keep-default".to_string()],
            Action::Checkpoint => vec!["storage".to_string(), "host".to_string(), "checkpoint-machine".to_string()],
        }
    }

    /// Carry the exact checked generation through the storage process boundary.
    /// Profile selection uses only the already checked immutable system target.
    pub fn arguments(&self, root: &str) -> (args: Vec<String>)
        ensures
            (self@.1 is PinHost || self@.1 is PinTransport || self@.1 is PinController) ==>
                args.len() == 4 && args[0]@ == "--add-root"@ && args[1]@ == root@
                    && args[2]@ == "--realise"@ && args[3]@ == target(self@.0, self@.1),
            self@.1 is Profile ==> args.len() == 4 && args[0]@ == "-p"@
                && args[1]@ == "/nix/var/nix/profiles/system"@ && args[2]@ == "--set"@ && args[3]@ == self@.0.host@,
            self@.1 is InstallController ==> args.len() == 0,
            (self@.1 is Publish || self@.1 is Validate || self@.1 is InstallBoot || self@.1 is Bind || self@.1 is Select) ==>
                args.len() == (if receipt::required(self@.1) && self@.6.is_some() { 10usize } else { 4usize }) && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[3]@ == self@.0.host@,
            self@.1 is Publish ==> args[2]@ == "publish"@,
            self@.1 is Validate ==> args[2]@ == "validate"@,
            self@.1 is InstallBoot ==> args[2]@ == "install-boot"@,
            self@.1 is Bind ==> args[2]@ == "bind-machine"@,
            self@.1 is Select ==> args[2]@ == "select"@,
            self@.1 is ProtectBoot ==> args.len() == 3 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "protect-boot"@,
            self@.1 is SealBoot ==> args.len() == (if receipt::required(self@.1) && self@.6.is_some() { 10usize } else { 4usize }) && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "seal-boot"@ && args[3]@ == "--keep-default"@,
            self@.1 is Checkpoint ==> args.len() == 3 && args[0]@ == "storage"@ && args[1]@ == "host"@ && args[2]@ == "checkpoint-machine"@,
            (receipt::required(self@.1) && !(self@.1 is Profile) && self@.6.is_some()) ==>
                args.len() == 10 && args[4]@ == "--expected-system"@ && args[5]@ == self@.6.unwrap().system@
                && args[6]@ == "--expected-generation"@ && args[7]@ == self@.6.unwrap().generation@
                && args[8]@ == "--expected-record"@ && args[9]@ == self@.6.unwrap().record@,
            (self@.1 is InstallBoot || self@.1 is Bind || self@.1 is Select) ==>
                args[0]@ == "storage"@ && args[1]@ == "host"@ && args[3]@ == self@.0.host@,
            self@.1 is InstallBoot ==> args[2]@ == "install-boot"@,
            self@.1 is Bind ==> args[2]@ == "bind-machine"@,
            self@.1 is Select ==> args[2]@ == "select"@,
            self@.1 is Profile ==> args.len() == 4 && args[0]@ == "-p"@
                && args[1]@ == "/nix/var/nix/profiles/system"@ && args[2]@ == "--set"@ && args[3]@ == self@.0.host@,
    {
        let mut args = self.base_arguments(root);
        let selected = if receipt::needs_receipt(self.action) && !matches!(self.action, Action::Profile) {
            self.checked
        } else { None };
        if let Some(receipt) = selected {
            let observation = receipt.observation();
            args.push("--expected-system".to_string()); args.push(observation.system.clone());
            args.push("--expected-generation".to_string()); args.push(observation.generation.clone());
            args.push("--expected-record".to_string()); args.push(observation.record.clone());
        }
        args
    }

}

}
