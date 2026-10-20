//! Explicit maintenance confirmation is separate from reversible deployment authority.
//! Retention, reference discovery and truthful publication remain coordinator adapter duties.

use crate::retirement::{Observation, Receipt, Status};
use airlock_machine_planning::names::owned_contains;
use vstd::prelude::*;

verus! {

/// Only an original retired or interrupted-purge record can admit maintenance deletion.
pub open spec fn eligible(r: Receipt) -> bool {
    r.id@ == r.transaction@ + "-"@ + r.name@ && r.name@.len() > 0 && r.transaction@.len() > 0
        && r.uuid@.len() > 0 && r.resources@.len() > 0 && r.generation@.len() > 0
        && (r.status is Trashed || r.status is Purging)
}

fn is_eligible(r: &Receipt) -> (yes: bool) ensures yes == eligible(*r),
{
    crate::retirement::record_id(r) && !r.name.is_empty() && !r.transaction.is_empty()
        && !r.uuid.is_empty() && !r.resources.is_empty() && !r.generation.is_empty()
        && matches!(r.status, Status::Trashed | Status::Purging)
}

/// Exact registration, complete volume identity and selected storage must still match before deletion.
pub open spec fn same_resources(r: Receipt, o: Observation) -> bool {
    r.name@ == o.name@ && r.uuid@ == o.uuid@ && r.resources@ == o.resources@
        && r.generation@ == o.generation@ && r.system@ == o.system@ && r.manifest@ == o.manifest@
        && o.power@ is Halted
}

/// Confirmation owns the protected original trash identity; no deployment request can become this type.
pub struct ConfirmedPurge {
    // Original confirmed record, retained unchanged through publication and cleanup.
    receipt: Receipt,
}

impl View for ConfirmedPurge {
    type V = Receipt;

    closed spec fn view(&self) -> Receipt { self.receipt }
}

impl ConfirmedPurge {
    /// Capture only a canonical protected record whose full ID was explicitly confirmed.
    pub fn new(id: &str, confirmation: &str, receipt: Receipt) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == receipt && eligible(receipt)
            && receipt.id@ == id@ && id@ == confirmation@,
    {
        if id != confirmation || id != receipt.id.as_str() || !is_eligible(&receipt) { return None; }
        Some(Self { receipt })
    }

    /// A remove intent additionally requires the original UUID/volumes/generation, halted state and retired tag.
    pub fn prepare<'a>(&'a self, observed: &Observation, tags: &[String]) -> (out: Option<PurgeIntent<'a>>)
        ensures out.is_some() ==> eligible(self@) && same_resources(self@, *observed)
            && airlock_machine_planning::names::owned_member(tags@, "airlock-retired"@),
            out.is_some() ==> out.unwrap()@ == self@,
    {
        let r = &self.receipt;
        if !is_eligible(r) || r.name.as_str() != observed.name.as_str() || r.uuid.as_str() != observed.uuid.as_str()
            || r.resources.as_str() != observed.resources.as_str() || r.generation.as_str() != observed.generation.as_str()
            || r.system.as_str() != observed.system.as_str() || r.manifest.as_str() != observed.manifest.as_str()
            || !observed.power.is_halted() || !owned_contains(tags, "airlock-retired") { return None; }
        Some(PurgeIntent { confirmed: self })
    }

    /// An absent VM permits only cleanup of an already published purge, never a new deletion.
    pub fn absent<'a>(&'a self, registered: &[String]) -> (out: Option<PurgeCleanup<'a>>)
        ensures out.is_some() ==> eligible(self@) && self@.status is Purging
            && !airlock_machine_planning::names::owned_member(registered@, self@.name@),
            out.is_some() ==> out.unwrap()@ == self@,
    {
        if !is_eligible(&self.receipt) || !matches!(self.receipt.status, Status::Purging)
            || owned_contains(registered, self.receipt.name.as_str()) { return None; }
        Some(PurgeCleanup { confirmed: self })
    }
}

/// Validated destruction awaiting durable Purging publication; not accepted by the remove adapter.
///
/// ```compile_fail
/// use airlock_deployment_core::purge::{PurgeIntent, RemoveRequest};
/// fn remove(_: RemoveRequest<'_>) {}
/// fn bypass(intent: PurgeIntent<'_>) { remove(intent); }
/// ```
pub struct PurgeIntent<'a> {
    // Explicit confirmation and original resource identity, not caller-selected target parameters.
    confirmed: &'a ConfirmedPurge,
}

impl<'a> View for PurgeIntent<'a> {
    type V = Receipt;

    closed spec fn view(&self) -> Receipt { self.confirmed@ }
}

impl<'a> PurgeIntent<'a> {
    /// Consume the intent only after the adapter durably saves Purging for this same record.
    /// File/parent sync success is a trusted persistence acknowledgment, as for private copying.
    pub fn published(self) -> (out: RemoveRequest<'a>) ensures out@ == self@,
    { RemoveRequest { confirmed: self.confirmed } }
}

/// The privileged remove adapter accepts this value without an independent VM target.
pub struct RemoveRequest<'a> {
    // Canonical original trash authority that passed both selection and publication.
    confirmed: &'a ConfirmedPurge,
}

impl<'a> View for RemoveRequest<'a> {
    type V = Receipt;

    closed spec fn view(&self) -> Receipt { self.confirmed@ }
}

impl<'a> RemoveRequest<'a> {
    /// A fixed remove command can only target the original confirmed VM name.
    pub fn arguments(&self) -> (out: Vec<String>)
        ensures out.len() == 2 && out[0]@ == "--force"@ && out[1]@ == self@.name@,
    { vec!["--force".to_string(), self.confirmed.receipt.name.as_str().to_string()] }

    /// A faithful successful remove acknowledgment permits cleanup of these same resources only.
    pub fn removed(self) -> (out: PurgeCleanup<'a>) ensures out@ == self@,
    { PurgeCleanup { confirmed: self.confirmed } }
}

/// Cleanup cannot select a new VM, generation or trash retention root after removal.
pub struct PurgeCleanup<'a> {
    // Original confirmed identity, obtained after successful removal or observed interrupted absence.
    confirmed: &'a ConfirmedPurge,
}

impl<'a> View for PurgeCleanup<'a> {
    type V = Receipt;

    closed spec fn view(&self) -> Receipt { self.confirmed@ }
}

impl<'a> PurgeCleanup<'a> {
    /// Exact VM/generation used by the retained-storage cleanup adapter.
    pub fn storage(&self) -> (out: (&str, &str))
        ensures out.0@ == self@.name@ && out.1@ == self@.generation@,
    { (self.confirmed.receipt.name.as_str(), self.confirmed.receipt.generation.as_str()) }

    /// Canonical confirmed identity used for trash retention-root cleanup.
    pub fn identity(&self) -> (out: &str) ensures out@ == self@.id@,
    { self.confirmed.receipt.id.as_str() }
}

}
