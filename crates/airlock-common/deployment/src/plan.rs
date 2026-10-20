//! Check complete plan impact before retaining lifecycle scopes and their binding.
//! Projection comes from classified plan fields rather than an independently supplied list.

use crate::{
    effects::{self, GuiPreservation, Shutdowns, Targets},
    operation::{AttemptId, AuthorityBinding, AuthorizedOperation, PlanBinding},
    recovery::{self, RecoveryDecision},
    review,
};
use airlock_machine_planning::effects::PlanEffects;
use vstd::prelude::*;

verus! {

/// Checked lifecycle scopes and the immutable binding of the complete effect plan.
pub ghost struct CheckedPlan {
    /// Startup targets covered by the complete review.
    pub starts: effects::Scope,
    /// Shutdown targets and GUI clients covered by that same review.
    pub shutdowns: effects::ShutdownScope,
    /// Explicit configuration reconciliation targets under the original review.
    pub configuration: effects::Scope,
    /// Original retirement namespace.
    pub retire: effects::Scope,
    /// Original restoration namespace.
    pub restore: effects::Scope,
    /// Copy sources in the exact classified pair order.
    pub copy_sources: effects::Scope,
    /// Copy destinations in the same classified pair order.
    pub copy_targets: effects::Scope,
    /// Original PCI handoff source namespace.
    pub pci_sources: effects::Scope,
    /// Original PCI handoff destination namespace.
    pub pci_targets: effects::Scope,
    /// Candidate, baseline, revision and complete serialized plan digest.
    pub binding: Seq<char>,
}

/// Opaque plan authority consumed by fresh execution or checked recovery.
/// The binding cannot be independently supplied again when creating an operation.
pub struct AuthorizedPlan {
    // Checked startup subset of this exact full plan.
    starts: Targets,
    // Checked shutdown targets and GUI modes from the same plan.
    shutdowns: Shutdowns,
    // Configuration targets admitted by the complete projected plan.
    configuration: Targets,
    // Retirement targets captured from the classified plan.
    retire: Targets,
    // Restoration targets captured from the classified plan.
    restore: Targets,
    // Approved copy sources, retaining their pairing order.
    copy_sources: Targets,
    // Approved copy destinations, retaining the same pairing order.
    copy_targets: Targets,
    // Checked handoff sources, never supplied at dispatch.
    pci_sources: Targets,
    // Checked handoff destinations.
    pci_targets: Targets,
    // Immutable complete-plan digest supplied by the authenticated adapter.
    binding: PlanBinding,
}

impl View for AuthorizedPlan {
    type V = CheckedPlan;

    closed spec fn view(&self) -> CheckedPlan {
        CheckedPlan { starts: self.starts@, shutdowns: self.shutdowns@, configuration: self.configuration@, retire: self.retire@, restore: self.restore@, copy_sources: self.copy_sources@, copy_targets: self.copy_targets@, pci_sources: self.pci_sources@, pci_targets: self.pci_targets@, binding: self.binding@ }
    }
}

/// Copy a classified lifecycle field without substituting, omitting or adding names.
fn targets(names: &[&str]) -> (out: Vec<String>)
    ensures out.len() == names.len(),
        forall|i: int| 0 <= i < names.len() ==> #[trigger] out[i]@ == names[i]@,
{
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < names.len()
        invariant
            0 <= i <= names.len(), out.len() == i,
            forall|j: int| 0 <= j < i ==> #[trigger] out[j]@ == names[j]@,
        decreases names.len() - i,
    {
        out.push(names[i].to_string());
        i += 1;
    }

    out
}

impl AuthorizedPlan {
    /// Derive impact and reboot permission from the full classified plan, then check approval.
    /// The identity input carries no proposed effect list or reboot flag.
    /// This prevents a caller from omitting a category by supplying a smaller effect list.
    pub fn reviewed(
        r: &review::Review, b: &review::Binding, identity: &review::Identity,
        plan: &PlanEffects, gui: Vec<GuiPreservation>,
        binding: PlanBinding,
    ) -> (result: Result<Self, review::Denial>)
        ensures
            result.is_ok() ==> plan.unknown.len() == 0,
            result.is_ok() ==> effects::valid(result.unwrap()@.pci_sources) && effects::valid(result.unwrap()@.pci_targets),
            result.is_ok() ==> result.unwrap()@.pci_sources.targets.len() == plan.pci_sources.len() && result.unwrap()@.pci_targets.targets.len() == plan.pci_targets.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.pci_sources.len() ==> #[trigger] result.unwrap()@.pci_sources.targets[i]@ == plan.pci_sources[i]@,
            result.is_ok() ==> forall|i: int| 0 <= i < plan.pci_targets.len() ==> #[trigger] result.unwrap()@.pci_targets.targets[i]@ == plan.pci_targets[i]@,
            result.is_ok() ==> result.unwrap()@.pci_sources.reviewed && result.unwrap()@.pci_targets.reviewed,
            result.is_ok() ==> effects::valid(result.unwrap()@.retire) && effects::valid(result.unwrap()@.restore),
            result.is_ok() ==> result.unwrap()@.retire.reviewed && result.unwrap()@.restore.reviewed,
            result.is_ok() ==> result.unwrap()@.retire.targets.len() == plan.retire.len() && result.unwrap()@.restore.targets.len() == plan.restore.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.retire.len() ==> #[trigger] result.unwrap()@.retire.targets[i]@ == plan.retire[i]@,
            result.is_ok() ==> forall|i: int| 0 <= i < plan.restore.len() ==> #[trigger] result.unwrap()@.restore.targets[i]@ == plan.restore[i]@,
            result.is_ok() ==> effects::valid(result.unwrap()@.copy_sources) && effects::valid(result.unwrap()@.copy_targets),
            result.is_ok() ==> result.unwrap()@.copy_sources.reviewed && result.unwrap()@.copy_targets.reviewed,
            result.is_ok() ==> result.unwrap()@.copy_sources.targets.len() == plan.copy_sources.len(),
            result.is_ok() ==> result.unwrap()@.copy_targets.targets.len() == plan.copy_targets.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.copy_sources.len() ==> #[trigger] result.unwrap()@.copy_sources.targets[i]@ == plan.copy_sources[i]@,
            result.is_ok() ==> forall|i: int| 0 <= i < plan.copy_targets.len() ==> #[trigger] result.unwrap()@.copy_targets.targets[i]@ == plan.copy_targets[i]@,
            result.is_ok() ==> review::identity_authorized(r, b, identity),
            result.is_ok() ==> forall|name: Seq<char>| #[trigger] airlock_machine_planning::effects::affects(plan, name)
                ==> effects::named(r.allowed@, name),
            result.is_ok() ==> (airlock_machine_planning::effects::may_reboot(plan) ==> r.allow_reboot),
            result.is_ok() ==> result.unwrap()@.binding == binding@,
            result.is_ok() ==> effects::valid(result.unwrap()@.configuration),
            result.is_ok() ==> result.unwrap()@.configuration.targets.len() == plan.configure.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.configure.len() ==>
                #[trigger] result.unwrap()@.configuration.targets[i]@ == plan.configure[i]@,
            result.is_ok() ==> result.unwrap()@.starts.targets.len() == plan.start.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.start.len() ==>
                #[trigger] result.unwrap()@.starts.targets[i]@ == plan.start[i]@,
            result.is_ok() ==> result.unwrap()@.shutdowns.targets.targets.len() == plan.stop.len(),
            result.is_ok() ==> forall|i: int| 0 <= i < plan.stop.len() ==>
                #[trigger] result.unwrap()@.shutdowns.targets.targets[i]@ == plan.stop[i]@,
            result.is_ok() ==> effects::valid(result.unwrap()@.starts)
                && effects::shutdown_valid(result.unwrap()@.shutdowns),
            result.is_ok() ==> result.unwrap()@.starts.reviewed
                && result.unwrap()@.shutdowns.targets.reviewed,
    {
        let projected = match plan.project() {
            Some(names) => names,
            None => return Err(review::Denial::Unsupported),
        };
        let candidate = review::Candidate {
            release: identity.release, revision: identity.revision, lock: identity.lock,
            guest: identity.guest, baseline: identity.baseline,
            previous_operation: identity.previous_operation,
            effects: projected, may_reboot: plan.requires_reboot_permission(),
        };

        let retire = Targets::reviewed(r, b, &candidate, targets(plan.retire.as_slice()))?;
        let restore = Targets::reviewed(r, b, &candidate, targets(plan.restore.as_slice()))?;
        let copy_sources = Targets::reviewed(r, b, &candidate, targets(plan.copy_sources.as_slice()))?;
        let copy_targets = Targets::reviewed(r, b, &candidate, targets(plan.copy_targets.as_slice()))?;
        let pci_sources = Targets::reviewed(r, b, &candidate, targets(plan.pci_sources.as_slice()))?;
        let pci_targets = Targets::reviewed(r, b, &candidate, targets(plan.pci_targets.as_slice()))?;
        let configuration = Targets::reviewed(r, b, &candidate, targets(plan.configure.as_slice()))?;
        let starts = Targets::reviewed(r, b, &candidate, targets(plan.start.as_slice()))?;
        let shutdowns = Shutdowns::reviewed(r, b, &candidate, targets(plan.stop.as_slice()), gui)?;

        proof {
            assert forall|name: Seq<char>| #[trigger] airlock_machine_planning::effects::affects(plan, name)
                implies effects::named(r.allowed@, name) by {
                assert(airlock_machine_planning::effects::member(candidate.effects@, name));
                let i = choose|i: int| 0 <= i < candidate.effects.len()
                    && #[trigger] candidate.effects[i]@ == name;
                assert(review::member(r.allowed@, candidate.effects[i]));
            }
        }

        Ok(Self { starts, shutdowns, configuration, retire, restore, copy_sources, copy_targets, pci_sources, pci_targets, binding })
    }

    /// Preserve explicit policy exceptions while still refusing unclassified effects.
    pub fn policy_exempt(plan: &PlanEffects,
        gui: Vec<GuiPreservation>, binding: PlanBinding) -> (result: Option<Self>)
        ensures
            result.is_some() ==> plan.unknown.len() == 0,
            result.is_some() ==> effects::valid(result.unwrap()@.pci_sources) && effects::valid(result.unwrap()@.pci_targets),
            result.is_some() ==> result.unwrap()@.pci_sources.targets.len() == plan.pci_sources.len() && result.unwrap()@.pci_targets.targets.len() == plan.pci_targets.len(),
            result.is_some() ==> forall|i: int| 0 <= i < plan.pci_sources.len() ==> #[trigger] result.unwrap()@.pci_sources.targets[i]@ == plan.pci_sources[i]@,
            result.is_some() ==> forall|i: int| 0 <= i < plan.pci_targets.len() ==> #[trigger] result.unwrap()@.pci_targets.targets[i]@ == plan.pci_targets[i]@,
            result.is_some() ==> effects::valid(result.unwrap()@.retire) && effects::valid(result.unwrap()@.restore),
            result.is_some() ==> result.unwrap()@.retire.targets.len() == plan.retire.len() && result.unwrap()@.restore.targets.len() == plan.restore.len(),
            result.is_some() ==> forall|i: int| 0 <= i < plan.retire.len() ==> #[trigger] result.unwrap()@.retire.targets[i]@ == plan.retire[i]@,
            result.is_some() ==> forall|i: int| 0 <= i < plan.restore.len() ==> #[trigger] result.unwrap()@.restore.targets[i]@ == plan.restore[i]@,
            result.is_some() ==> result.unwrap()@.copy_sources.targets.len() == plan.copy_sources.len(),
            result.is_some() ==> result.unwrap()@.copy_targets.targets.len() == plan.copy_targets.len(),
            result.is_some() ==> forall|i: int| 0 <= i < plan.copy_sources.len() ==> #[trigger] result.unwrap()@.copy_sources.targets[i]@ == plan.copy_sources[i]@,
            result.is_some() ==> forall|i: int| 0 <= i < plan.copy_targets.len() ==> #[trigger] result.unwrap()@.copy_targets.targets[i]@ == plan.copy_targets[i]@,
            result.is_some() ==> effects::valid(result.unwrap()@.copy_sources) && effects::valid(result.unwrap()@.copy_targets),
            result.is_some() ==> result.unwrap()@.binding == binding@,
            result.is_some() ==> effects::valid(result.unwrap()@.configuration),
            result.is_some() ==> effects::valid(result.unwrap()@.starts)
                && effects::shutdown_valid(result.unwrap()@.shutdowns),
            result.is_some() ==> !result.unwrap()@.starts.reviewed,
    {
        plan.project()?;
        let retire = Targets::policy_exempt(targets(plan.retire.as_slice()));
        let restore = Targets::policy_exempt(targets(plan.restore.as_slice()));
        let copy_sources = Targets::policy_exempt(targets(plan.copy_sources.as_slice()));
        let copy_targets = Targets::policy_exempt(targets(plan.copy_targets.as_slice()));
        let pci_sources = Targets::policy_exempt(targets(plan.pci_sources.as_slice()));
        let pci_targets = Targets::policy_exempt(targets(plan.pci_targets.as_slice()));
        let configuration = Targets::policy_exempt(targets(plan.configure.as_slice()));
        let starts = Targets::policy_exempt(targets(plan.start.as_slice()));
        let shutdowns = Shutdowns::policy_exempt(targets(plan.stop.as_slice()), gui)?;
        Some(Self { starts, shutdowns, configuration, retire, restore, copy_sources, copy_targets, pci_sources, pci_targets, binding })
    }

    /// Bind decoded VM definitions to exactly the configuration targets in this plan.
    pub fn configuration(&self, definitions: Vec<crate::configuration::Definition>, setup: crate::configuration::Helper)
        -> (result: Option<crate::configuration::ConfigurationPlan>)
        ensures result.is_some() ==> result.unwrap()@.binding == self@.binding,
            result.is_some() && effects::valid(self@.configuration) ==>
                crate::configuration::valid(result.unwrap()@),
    {
        crate::configuration::ConfigurationPlan::new(self.configuration.duplicate(),
            PlanBinding::new(self.binding.as_str().to_string()), definitions, setup)
    }

    /// Retain decoded device declarations under this exact checked configuration namespace.
    pub fn devices(&self, definitions: Vec<crate::devices::Definition>) -> (out: Option<crate::devices::DevicePlan>)
        ensures out.is_some() ==> out.unwrap()@.2 == self@.binding && out.unwrap()@.1 == definitions@,
            out.is_some() && effects::valid(self@.configuration) ==> crate::devices::valid(out.unwrap()@),
            out.is_some() ==> out.unwrap()@.0.allowed == self@.configuration.allowed && out.unwrap()@.0.reviewed == self@.configuration.reviewed,
    {
        crate::devices::DevicePlan::new(self.configuration.duplicate(), definitions, PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Bind native storage to the same complete checked configuration namespace.
    pub fn storage(&self, definitions: Vec<crate::storage::Definition>) -> (out: Option<crate::storage::StoragePlan>)
        ensures out.is_some() ==> out.unwrap()@.2 == self@.binding,
            out.is_some() && effects::valid(self@.configuration) ==> crate::storage::valid(out.unwrap()@),
            out.is_some() ==> out.unwrap()@.0.allowed == self@.configuration.allowed && out.unwrap()@.0.reviewed == self@.configuration.reviewed,
    {
        crate::storage::StoragePlan::new(self.configuration.duplicate(), definitions, PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Bind copy definitions to the original ordered endpoints and complete execution plan.
    pub fn copies(&self, definitions: Vec<crate::copy::Definition>) -> (out: Option<crate::copy::CopyPlan>)
        ensures out.is_some() ==> out.unwrap()@.3 == self@.binding,
            out.is_some() ==> out.unwrap()@.0.allowed == self@.copy_sources.allowed && out.unwrap()@.0.reviewed == self@.copy_sources.reviewed,
            out.is_some() ==> out.unwrap()@.1.allowed == self@.copy_targets.allowed && out.unwrap()@.1.reviewed == self@.copy_targets.reviewed,
            out.is_some() && effects::valid(self@.copy_sources) && effects::valid(self@.copy_targets) ==> crate::copy::valid(out.unwrap()@),
    {
        crate::copy::CopyPlan::new(self.copy_sources.duplicate(), self.copy_targets.duplicate(),
            definitions, PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Bind reversible retirement to the exact approved retire or restore field.
    pub fn retirement(&self, definitions: Vec<crate::retirement::Definition>, kind: crate::retirement::Kind)
        -> (out: Option<crate::retirement::RetirementPlan>)
        ensures out.is_some() ==> out.unwrap()@.3 == self@.binding && out.unwrap()@.2 == kind,
            out.is_some() && effects::valid(self@.retire) && effects::valid(self@.restore) ==> crate::retirement::valid(out.unwrap()@),
            out.is_some() ==> out.unwrap()@.0.allowed == match kind { crate::retirement::Kind::Retire => self@.retire.allowed, crate::retirement::Kind::Restore => self@.restore.allowed },
            out.is_some() ==> out.unwrap()@.0.reviewed == match kind { crate::retirement::Kind::Retire => self@.retire.reviewed, crate::retirement::Kind::Restore => self@.restore.reviewed },
    {
        let scope = match kind { crate::retirement::Kind::Retire => self.retire.duplicate(), crate::retirement::Kind::Restore => self.restore.duplicate() };
        crate::retirement::RetirementPlan::new(scope, definitions, kind, PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Retain exact checked PCI endpoints; no dispatch-time pair can replace them.
    pub fn pci_handoff(&self) -> (out: crate::pci::HandoffPlan)
        ensures out@.2 == self@.binding,
            out@.0.allowed == self@.pci_sources.allowed && out@.0.reviewed == self@.pci_sources.reviewed,
            out@.1.allowed == self@.pci_targets.allowed && out@.1.reviewed == self@.pci_targets.reviewed,
            effects::valid(self@.pci_sources) ==> effects::valid(out@.0),
            effects::valid(self@.pci_targets) ==> effects::valid(out@.1),
            out@.0.targets.len() == self@.pci_sources.targets.len(),
            out@.1.targets.len() == self@.pci_targets.targets.len(),
            forall|i: int| 0 <= i < out@.0.targets.len() ==> #[trigger] out@.0.targets[i]@ == self@.pci_sources.targets[i]@,
            forall|i: int| 0 <= i < out@.1.targets.len() ==> #[trigger] out@.1.targets[i]@ == self@.pci_targets.targets[i]@,
    {
        crate::pci::HandoffPlan::new(self.pci_sources.duplicate(), self.pci_targets.duplicate(), PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Retain host artifacts decoded from this same admitted candidate before publication.
    /// Artifact admission and faithful release decoding remain trusted adapter boundaries.
    pub fn host(&self, definition: crate::host::Definition) -> (out: crate::host::HostPlan)
        ensures out@ == (definition, self@.binding),
    {
        crate::host::HostPlan::new(definition, PlanBinding::new(self.binding.as_str().to_string()))
    }

    /// Consume this checked plan without accepting a replacement plan binding.
    pub fn operation(self, attempt: AttemptId, authority: AuthorityBinding) -> (result: Option<AuthorizedOperation>)
        ensures
            result.is_some(),
            result.unwrap()@.plan == self@.binding,
            result.unwrap()@.scope == self@.starts,
            result.unwrap()@.shutdown == self@.shutdowns,
            result.unwrap()@.attempt == attempt@ && result.unwrap()@.authority == authority@,
    {
        AuthorizedOperation::new(self.starts, self.shutdowns, attempt, authority, self.binding, None, None)
    }

    /// Reconciliation retains original authority and uses only this freshly checked plan.
    pub fn recover(self, decision: RecoveryDecision, prior: &recovery::Journal,
        target: &recovery::Release, baseline: &recovery::Release,
        attempt: AttemptId, authority: AuthorityBinding) -> (result: Option<AuthorizedOperation>)
        ensures
            result.is_some() ==> result.unwrap()@.plan == self@.binding
                && result.unwrap()@.scope == self@.starts && result.unwrap()@.shutdown == self@.shutdowns,
            result.is_some() ==> result.unwrap()@.operation == recovery::operation(decision@.0),
            result.is_some() ==> result.unwrap()@.attempt == attempt@ && result.unwrap()@.authority == authority@,
            result.is_some() && decision@.0.operation.is_some() ==>
                authority@ == decision@.0.operation.unwrap().authority@,
    {
        decision.authorize(prior, target, baseline, self.starts, self.shutdowns,
            recovery::Attempt { id: attempt, authority, plan: self.binding })
    }
}

}
