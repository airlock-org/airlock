//! A checked host generation carries the publication result into later boot requests.
//! The wire decoder authenticates field shape; faithful storage observations and hashes are trusted.

use super::{Action, Definition, HostPlan, HostRequest};
use crate::operation::{DurableOperation, ExecutionContext};
use vstd::prelude::*;

verus! {

/// Typed wire identities are reduced to canonical strings at this verification boundary.
pub struct Observation {
    /// Immutable host system in the validated boot record.
    pub system: String,
    /// Canonical positive generation, decoded through Rust's GenerationId type.
    pub generation: String,
    /// Digest of the complete boot record, including storage paths and checksums.
    pub record: String,
}

/// Both the generation and its complete record must remain identical.
pub open spec fn same(a: Observation, b: Observation) -> bool {
    a.system@ == b.system@ && a.generation@ == b.generation@ && a.record@ == b.record@
}

/// Compare canonical observed identities without trusting an adapter-provided permission flag.
pub fn equal(a: &Observation, b: &Observation) -> (yes: bool)
    ensures yes == same(*a, *b),
{ a.system == b.system && a.generation == b.generation && a.record == b.record }

/// An admitted prepared view fixes its original identity; publication captures a new result.
pub open spec fn admitted(d: Definition, observation: Observation) -> bool {
    observation.system@ == d.host@ && observation.system@.len() > 0
        && observation.generation@.len() > 0 && observation.generation@ != "0"@
        && observation.record@.len() > 0
        && (d.expected.is_some() ==> same(d.expected.unwrap(), observation))
}

fn accepts(d: &Definition, observation: &Observation) -> (yes: bool)
    ensures yes == admitted(*d, *observation),
{
    if observation.system != d.host || observation.system.is_empty()
        || observation.generation.is_empty() || observation.generation.as_str() == "0"
        || observation.record.is_empty() { return false; }
    match &d.expected { Some(expected) => equal(expected, observation), None => true }
}

/// Candidate profile/boot mutations require an observed, checked generation.
pub open spec fn required(action: Action) -> bool {
    matches!(action, Action::Profile | Action::InstallBoot | Action::SealBoot | Action::Bind | Action::Select)
}

pub(super) fn needs_receipt(action: Action) -> (yes: bool)
    ensures yes == required(action),
{ matches!(action, Action::Profile | Action::InstallBoot | Action::SealBoot | Action::Bind | Action::Select) }

/// An observation checked against the original host plan and one exact executing attempt.
pub struct CheckedGeneration {
    // Result of successful publication/validation, never reconstructed at a later dispatch.
    observation: Observation,
    // Attempt in which this result was observed.
    attempt: String,
    // Stable deployment operation owning the observation.
    operation: String,
    // Original approval, including approved candidate identity and effect scope.
    authority: String,
    // Complete candidate/baseline/plan binding.
    plan: String,
}

impl View for CheckedGeneration {
    type V = (Observation, Seq<char>, Seq<char>, Seq<char>, Seq<char>);

    closed spec fn view(&self) -> Self::V {
        (self.observation, self.attempt@, self.operation@, self.authority@, self.plan@)
    }
}

/// A checked result cannot migrate to another attempt, operation, authority or plan.
pub open spec fn matches_context(receipt: CheckedGeneration, d: Definition, context: ExecutionContext) -> bool {
    admitted(d, receipt@.0) && receipt@.1 == context.attempt@ && receipt@.2 == context.operation@
        && receipt@.3 == context.authority@ && receipt@.4 == context.plan@
}

impl<'a> HostRequest<'a> {
    /// Consume a publication/validation request only after its adapter returned a typed observation.
    pub fn checked(self, observation: Observation) -> (out: Option<CheckedGeneration>)
        ensures out.is_some() == ((self@.1 is Publish || self@.1 is Validate) && admitted(self@.0, observation)),
            out.is_some() ==> out.unwrap()@ == (observation, self@.2, self@.3, self@.4, self@.5),
    {
        if !matches!(self.action, Action::Publish | Action::Validate)
            || !accepts(self.definition, &observation) { return None; }
        Some(CheckedGeneration { observation, attempt: self.attempt, operation: self.operation,
            authority: self.authority, plan: self.plan })
    }
}

impl HostPlan {
    /// Boot mutations consume a matching checked result and the current durable execution gate.
    pub fn request_checked<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        action: Action, receipt: &'a CheckedGeneration) -> (out: Option<HostRequest<'a>>)
        ensures out.is_some() == (self@.1 == context.plan@ && crate::operation::executing(durable, context)
            && required(action) && matches_context(*receipt, self@.0, *context)),
            out.is_some() ==> out.unwrap()@ == (self@.0, action, context.attempt@, context.operation@,
                context.authority@, context.plan@, Some(receipt@.0)),
    {
        if self.binding.as_str() != context.plan.as_str() || !durable.allows_effect(context)
            || !needs_receipt(action) || !accepts(&self.definition, &receipt.observation)
            || receipt.attempt.as_str() != context.attempt.as_str()
            || receipt.operation.as_str() != context.operation.as_str()
            || receipt.authority.as_str() != context.authority.as_str()
            || receipt.plan.as_str() != context.plan.as_str() { return None; }
        Some(HostRequest { definition: &self.definition, action,
            attempt: context.attempt.as_str().to_string(), operation: context.operation.as_str().to_string(),
            authority: context.authority.as_str().to_string(), plan: context.plan.as_str().to_string(),
            checked: Some(receipt), _durable: durable })
    }
}

impl CheckedGeneration {
    /// Expose only the immutable result to fixed argument construction.
    pub(super) fn observation(&self) -> (out: &Observation)
        ensures *out == self@.0,
    { &self.observation }
}

/// Preserve the host identity and filesystem context through the private mount process.
/// Missing checked fields cannot silently select administrative mode within this transformation.
pub fn isolated_arguments(system: &str, service: &str, pool: &str, esp: &str,
    offline: bool, expected: Option<&Observation>) -> (out: Option<Vec<String>>)
    ensures out.is_some() == (expected.is_none() || expected.unwrap().system@ == system@),
        out.is_some() ==> {
            let args = out.unwrap();
            let offset: int = if offline { 1 } else { 0 };
            &&& args[0]@ == "--mount"@ && args[1]@ == "--propagation"@ && args[2]@ == "private"@
            &&& args[3]@ == service@ && args[4]@ == "storage"@ && args[5]@ == "host"@
            &&& args[6]@ == "--pool-root"@ && args[7]@ == pool@
            &&& args[8]@ == "--efi-directory"@ && args[9]@ == esp@
            &&& offline ==> args[10]@ == "--offline"@
            &&& args[10 + offset]@ == "install-boot-isolated"@ && args[11 + offset]@ == system@
            &&& match expected {
                Some(e) => args.len() == 18 + offset && args[12 + offset]@ == "--expected-system"@
                    && args[13 + offset]@ == e.system@ && args[14 + offset]@ == "--expected-generation"@
                    && args[15 + offset]@ == e.generation@ && args[16 + offset]@ == "--expected-record"@
                    && args[17 + offset]@ == e.record@,
                None => args.len() == 13 + offset && args[12 + offset]@ == "--administrative"@,
            }
        },
{
    let matches = match expected { Some(e) => e.system.as_str() == system, None => true };
    if !matches { return None; }

    let mut args = vec!["--mount".to_string(), "--propagation".to_string(), "private".to_string(),
        service.to_string(), "storage".to_string(), "host".to_string(),
        "--pool-root".to_string(), pool.to_string(), "--efi-directory".to_string(), esp.to_string()];
    if offline { args.push("--offline".to_string()); }
    args.push("install-boot-isolated".to_string()); args.push(system.to_string());
    if let Some(e) = expected {
        args.push("--expected-system".to_string()); args.push(e.system.clone());
        args.push("--expected-generation".to_string()); args.push(e.generation.clone());
        args.push("--expected-record".to_string()); args.push(e.record.clone());
    } else { args.push("--administrative".to_string()); }

    Some(args)
}

}
