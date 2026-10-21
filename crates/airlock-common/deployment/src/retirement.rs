//! Authorise retirement/restoration against checked configuration and named storage observations.
//! Permanent purge has independent maintenance authority and is not a deployment action.
//!
//! ## Reversible retirement and restoration
//!
//! The [retirement boundary](crate::retirement) retains separate checked namespaces and
//! operation kinds. Retirement requires the complete baseline configuration, registered
//! UUID, complete volume set and currently selected generation/system/pool manifest to
//! agree. Restoration checks class/template/private-state compatibility and the original
//! private-volume identity while permitting an approved new root. Both require explicitly
//! halted named observations. Purging, purged and already restored records yield no request.
//! The trash identity must be the recorded transaction plus the same checked VM name.
//!
//! A baseline pin request retains the exact rollback release before disruption and uses
//! the protected proposal’s canonical transaction/VM root identity across retries. After the trash journal records retirement,
//! the process adapter consumes fixed autostart, tag and retention arguments. Restoration
//! emits only tag removal; neither family can construct a remove/purge command. Metadata
//! publication and removal of the validated trash retention root remain filesystem adapters.
//! The adapter retains the first proposal under the stable operation ID and reuses it
//! after a crash, refusing a replacement registration or volume set. Proposal lookup,
//! first-writer persistence, VM/volume observations and cryptographic encoding of
//! configuration/storage identities remain trusted; the machine lock does not exclude independent Qubes administrators.
//! Purge remains a separate, explicitly confirmed maintenance protocol, never a consequence
//! of possessing these deployment requests. The [purge boundary](crate::purge) captures the
//! canonical confirmed trash record and requires the original UUID, complete volume identity,
//! generation, system, manifest, retired tag and named halted observation before producing a
//! remove intent. Durable `Purging` publication turns that intent into the sole request accepted
//! by the maintenance remove adapter. Successful removal permits cleanup of the same generation
//! and canonical trash root. An absent VM permits cleanup only when the protected record already
//! says `Purging`; it cannot produce a remove request. The deployment runner refuses this
//! maintenance adapter.
//!
//! Retention expiry/waiver, release and policy reference discovery, storage alias checks,
//! protected-record authentication and actual publication remain runtime/I/O boundaries. Their
//! existing checks precede removal; Verus proves the request identity, observed resource checks,
//! halted condition and publication-token handoff. It does not prove those reference algorithms,
//! wall-clock correctness, faithful deletion, cleanup durability or atomicity with independent
//! Qubes administrators. A crash after removal is reconciled from registration observations.

use crate::{
    effects::{self, Targets},
    inventory::PowerState,
    operation::{DurableOperation, ExecutionContext, PlanBinding},
};
use vstd::prelude::*;

verus! {

/// Two distinct reversible lifecycle policies; neither permits VM deletion.
#[derive(Clone, Copy)]
pub enum Kind {
    /// Disable autostart, tag the VM retired and retain its recorded generation.
    Retire,
    /// Remove the retirement tag after checking compatible private storage.
    Restore,
}

/// Configuration captured from the appropriate immutable baseline or candidate.
pub struct Definition {
    /// Native storage adapter captured before publication.
    pub service: String,
    /// Original release retained as the rollback source for a newly retired VM.
    pub baseline: String,
    /// Exact VM in the checked retire or restore plan field.
    pub name: String,
    /// System closure expected for the retired baseline; restoration may install a newer root.
    pub system: String,
    /// Full baseline configuration for retire; class/template/private-state compatibility for restore.
    pub configuration: String,
}

/// Decoded durable trash state, including states that can never grant deployment permission.
#[derive(Clone, Copy)]
pub enum Status {
    /// Retirement recorded before its first mutation.
    Planned,
    /// Reversible retirement mutations may have started.
    Retiring,
    /// Retired VM and storage retained for recovery.
    Trashed,
    /// Permanent purge has separate authority; deployment cannot interfere.
    Purging,
    /// Resources were permanently removed.
    Purged,
    /// VM is already active again.
    Restored,
}

/// Protected trash evidence; this type itself confers no authority.
pub struct Receipt {
    /// Canonical transaction-and-VM identity of this protected trash record.
    pub id: String,
    /// Immutable rollback release recorded by this retirement proposal.
    pub baseline: String,
    /// Original retirement transaction, distinct from a later restoration attempt.
    pub transaction: String,
    /// VM whose trash record is being reconciled.
    pub name: String,
    /// Registered identity retained by the original trash record.
    pub uuid: String,
    /// Configuration interpreted according to the request family's semantics.
    pub configuration: String,
    /// All volume identities for retirement; persistent private volume identity for restoration.
    pub resources: String,
    /// Retained system closure recorded with the original trash configuration.
    pub system: String,
    /// Immutable pool manifest recorded for the retained generation.
    pub manifest: String,
    /// Exact generation whose retention may be changed.
    pub generation: String,
    /// Recorded lifecycle progress.
    pub status: Status,
}

/// Named Qubes observations cannot substitute another registration or volume set.
pub struct Observation {
    /// VM named by the observation adapter.
    pub name: String,
    /// Current registration UUID.
    pub uuid: String,
    /// Current storage identity under the same interpretation as the receipt.
    pub resources: String,
    /// Currently selected generation; required only when retiring the current baseline.
    pub generation: String,
    /// System identity read from that generation; restoration retains only private storage identity.
    pub system: String,
    /// Current pool manifest of that generation.
    pub manifest: String,
    /// Classified raw power state for this VM.
    pub power: PowerState,
}

/// Only supported non-purge states can be reconciled under deployment authority.
pub open spec fn supported(kind: Kind, status: Status) -> bool {
    match kind {
        Kind::Retire => status is Planned || status is Retiring || status is Trashed,
        Kind::Restore => status is Retiring || status is Trashed,
    }
}

/// Independent identity, configuration, storage and halted-state requirements.
pub open spec fn safe(d: Definition, kind: Kind, r: Receipt, o: Observation) -> bool {
    r.id@ == r.transaction@ + "-"@ + r.name@
        && d.name@ == r.name@ && d.name@ == o.name@ && d.configuration@ == r.configuration@
        && r.uuid@ == o.uuid@ && r.resources@ == o.resources@ && r.resources@.len() > 0
        && r.generation@.len() > 0 && o.power@ == crate::inventory::PowerClass::Halted && supported(kind, r.status)
        && match kind { Kind::Retire => r.baseline@ == d.baseline@ && r.generation@ == o.generation@ && r.system@ == d.system@
            && o.system@ == d.system@ && r.manifest@ == o.manifest@, Kind::Restore => true }
}

/// Check the actual record ID rather than allowing another VM's retention root to be substituted.
pub(crate) fn record_id(r: &Receipt) -> (yes: bool)
    ensures yes == (r.id@ == r.transaction@ + "-"@ + r.name@),
{
    let mut expected = r.transaction.as_str().to_string();
    expected.push('-');
    proof { reveal_strlit("-"); }
    assert(expected@ =~= r.transaction@ + "-"@);
    expected.push_str(r.name.as_str());
    expected.as_str() == r.id.as_str()
}

/// Immutable kind, checked namespace, configuration definitions and complete plan binding.
pub struct RetirementPlan {
    // Source-approved target subset for this particular operation family.
    targets: Targets,
    // Captured baseline or candidate definitions in the checked target order.
    definitions: Vec<Definition>,
    // Retire and restore cannot be substituted at dispatch.
    kind: Kind,
    // Exact original candidate/baseline/plan digest.
    binding: PlanBinding,
}

impl View for RetirementPlan {
    type V = (effects::Scope, Seq<Definition>, Kind, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.targets@, self.definitions@, self.kind, self.binding@) }
}

/// Every retained definition corresponds to exactly the same checked list entry.
pub open spec fn valid(s: (effects::Scope, Seq<Definition>, Kind, Seq<char>)) -> bool {
    effects::valid(s.0) && s.1.len() == s.0.targets.len()
        && forall|i: int| 0 <= i < s.1.len() ==> #[trigger] s.1[i].name@ == s.0.targets[i]@
}

impl RetirementPlan {
    /// Only AuthorizedPlan can bind this exact retire/restore namespace and operation kind.
    pub(crate) fn new(targets: Targets, definitions: Vec<Definition>, kind: Kind, binding: PlanBinding) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == (targets@, definitions@, kind, binding@),
            out.is_some() && effects::valid(targets@) ==> valid(out.unwrap()@),
    {
        let names = targets.names();
        if definitions.len() != names.len() { return None; }
        let mut i = 0;
        while i < definitions.len()
            invariant 0 <= i <= definitions.len(), definitions.len() == names.len(),
                forall|j: int| 0 <= j < i ==> #[trigger] definitions[j].name@ == names[j]@,
            decreases definitions.len() - i,
        {
            if definitions[i].name.as_str() != names[i].as_str() { return None; }
            i += 1;
        }
        Some(Self { targets, definitions, kind, binding })
    }

    /// Retain a retirement baseline before disruption; pinning does not require stopping a VM.
    pub fn pin<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext, index: usize, receipt: &Receipt)
        -> (out: Option<RetirementPin<'a>>)
        ensures out.is_some() ==> self@.3 == context.plan@ && crate::operation::executing(durable, context)
            && self@.2 is Retire && index < self@.1.len(),
            out.is_some() ==> out.unwrap()@ == (self@.1[index as int], receipt.id@)
                && receipt.id@ == receipt.transaction@ + "-"@ + self@.1[index as int].name@
                && receipt.baseline@ == self@.1[index as int].baseline@,
    {
        if self.binding.as_str() != context.plan.as_str() || !durable.allows_effect(context)
            || !matches!(self.kind, Kind::Retire) || index >= self.definitions.len() { return None; }
        let d = &self.definitions[index];
        if !self.targets.contains(d.name.as_str()) || !record_id(receipt)
            || receipt.name != d.name || receipt.configuration != d.configuration || receipt.baseline != d.baseline { return None; }
        let id = receipt.id.as_str().to_string();
        Some(RetirementPin { definition: d, id, _durable: durable })
    }

    /// Admit only an exact checked VM with compatible configuration, unchanged resources and durable authority.
    pub fn request<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        index: usize, receipt: &'a Receipt, observed: &Observation) -> (out: Option<RetirementRequest<'a>>)
        ensures out.is_some() ==> self@.3 == context.plan@ && crate::operation::executing(durable, context),
            out.is_some() ==> index < self@.1.len() && safe(self@.1[index as int], self@.2, *receipt, *observed),
            out.is_some() ==> out.unwrap()@ == (self@.1[index as int], self@.2, *receipt),
            out.is_some() && valid(self@) && self@.0.reviewed ==> self@.0.allowed.contains(receipt.name@),
    {
        if self.binding.as_str() != context.plan.as_str() || !durable.allows_effect(context) { return None; }
        if index >= self.definitions.len() { return None; }
        let d = &self.definitions[index];
        if !record_id(receipt) { return None; }
        if !self.targets.contains(d.name.as_str()) || d.name != receipt.name || d.name != observed.name
            || d.configuration != receipt.configuration || receipt.uuid != observed.uuid
            || receipt.resources != observed.resources || receipt.resources.is_empty()
            || receipt.generation.is_empty() || !observed.power.is_halted() { return None; }
        match self.kind {
            Kind::Retire => {
                if !matches!(receipt.status, Status::Planned | Status::Retiring | Status::Trashed)
                    || receipt.baseline != d.baseline || receipt.generation != observed.generation || receipt.system != d.system
                    || observed.system != d.system || receipt.manifest != observed.manifest { return None; }
            },
            Kind::Restore => if !matches!(receipt.status, Status::Retiring | Status::Trashed) { return None; },
        }
        proof {
            if valid(self@) { assert(self@.0.targets[index as int]@ == receipt.name@); }
        }
        Some(RetirementRequest { definition: d, kind: self.kind, receipt, _durable: durable })
    }
}

/// Exact reversible effect; no remove/purge command can be selected by this type.
pub struct RetirementRequest<'a> {
    // Baseline or candidate configuration admitted by the complete plan.
    definition: &'a Definition,
    // Fixed retire-versus-restore semantics.
    kind: Kind,
    // Validated named resource evidence; targets and generation cannot be replaced.
    receipt: &'a Receipt,
    // Keep the executing operation borrowed through every mutation.
    _durable: &'a DurableOperation,
}

impl<'a> View for RetirementRequest<'a> {
    type V = (Definition, Kind, Receipt);

    closed spec fn view(&self) -> Self::V { (*self.definition, self.kind, *self.receipt) }
}

impl<'a> RetirementRequest<'a> {
    /// Sole VM whose retirement state may change.
    pub fn target(&self) -> (name: &str)
        ensures name@ == self@.0.name@,
    { self.definition.name.as_str() }

    /// Preserve the distinct operation family at the privileged adapter.
    pub fn kind(&self) -> (kind: Kind)
        ensures kind == self@.1,
    { self.kind }

    /// Canonical trash identity whose metadata and retention root may be updated.
    pub fn trash_id(&self) -> (id: &str)
        ensures id@ == self@.2.id@,
    { self.receipt.id.as_str() }

    /// Exact protected generation whose retention is requested.
    pub fn generation(&self) -> (generation: &str)
        ensures generation@ == self@.2.generation@,
    { self.receipt.generation.as_str() }

    /// Native storage adapter selected before publication, not resolved again at dispatch.
    pub fn retention_program(&self) -> (program: &str)
        ensures program@ == self@.0.service@,
    { self.definition.service.as_str() }

    /// Only retirement can retain the exact observed generation; restoration does not run storage GC.
    pub fn retention_arguments(&self) -> (args: Option<Vec<String>>)
        ensures args.is_some() == (self@.1 is Retire),
            args.is_some() ==> args.unwrap().len() == 11
                && args.unwrap()[0]@ == "storage"@ && args.unwrap()[1]@ == "pool"@ && args.unwrap()[2]@ == "set-retention"@
                && args.unwrap()[3]@ == "--pool-root"@ && args.unwrap()[4]@ == "/var/lib/airlock/closure-store"@
                && args.unwrap()[5]@ == "--domain"@ && args.unwrap()[6]@ == self@.0.name@
                && args.unwrap()[7]@ == "--generation"@ && args.unwrap()[8]@ == self@.2.generation@
                && args.unwrap()[9]@ == "--retain"@ && args.unwrap()[10]@ == "true"@,
    {
        match self.kind {
            Kind::Retire => Some(vec!["storage".to_string(), "pool".to_string(), "set-retention".to_string(), "--pool-root".to_string(),
                "/var/lib/airlock/closure-store".to_string(), "--domain".to_string(), self.target().to_string(),
                "--generation".to_string(), self.generation().to_string(), "--retain".to_string(), "true".to_string()]),
            Kind::Restore => None,
        }
    }

    /// Fix the sole target and tag mutation; no arbitrary tag or VM can be passed by the adapter.
    pub fn tag_arguments(&self) -> (args: Vec<String>)
        ensures args.len() == 3 && args[0]@ == self@.0.name@ && args[2]@ == "airlock-retired"@,
            args[1]@ == match self@.1 { Kind::Retire => "add"@, Kind::Restore => "del"@ },
    {
        vec![self.target().to_string(), match self.kind { Kind::Retire => "add".to_string(), Kind::Restore => "del".to_string() }, "airlock-retired".to_string()]
    }

    /// Only retirement can disable autostart; restoration does not silently enable it.
    pub fn autostart_arguments(&self) -> (args: Option<Vec<String>>)
        ensures args.is_some() == (self@.1 is Retire),
            args.is_some() ==> args.unwrap().len() == 3 && args.unwrap()[0]@ == self@.0.name@
                && args.unwrap()[1]@ == "autostart"@ && args.unwrap()[2]@ == "False"@,
    {
        match self.kind { Kind::Retire => Some(vec![self.target().to_string(), "autostart".to_string(), "False".to_string()]), Kind::Restore => None }
    }
}

/// A baseline retention pin carries its exact release and transaction/VM root identity.
pub struct RetirementPin<'a> {
    // Original baseline definition admitted by the retire plan.
    definition: &'a Definition,
    // Canonical executing-attempt and VM identity.
    id: String,
    // Preserve the executing gate throughout root publication.
    _durable: &'a DurableOperation,
}

impl<'a> View for RetirementPin<'a> {
    type V = (Definition, Seq<char>);

    closed spec fn view(&self) -> Self::V { (*self.definition, self.id@) }
}

impl<'a> RetirementPin<'a> {
    /// Exact immutable baseline object to retain, never a dispatch-time override.
    pub fn release(&self) -> (release: &str)
        ensures release@ == self@.0.baseline@,
    { self.definition.baseline.as_str() }

    /// Root identity corresponding to this executing retirement attempt and checked VM.
    pub fn trash_id(&self) -> (id: &str)
        ensures id@ == self@.1,
    { self.id.as_str() }

    /// Fix the sole baseline object passed to the existing Nix retention adapter.
    pub fn arguments(&self, root: &str) -> (args: Vec<String>)
        ensures args.len() == 4 && args[0]@ == "--add-root"@ && args[1]@ == root@
            && args[2]@ == "--realise"@ && args[3]@ == self@.0.baseline@,
    { vec!["--add-root".to_string(), root.to_string(), "--realise".to_string(), self.release().to_string()] }
}

}
