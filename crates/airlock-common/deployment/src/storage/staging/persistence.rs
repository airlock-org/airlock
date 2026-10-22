//! Retain exact guest selections across recovery while rechecking each attempt's authority.
//! Snapshots are inert journal evidence; only a checked pin can enter durable execution.
//! Exact per-entry equality is independent of lookup order and generation freshness. Each
//! attempt must satisfy the current checked plan as well as any original target snapshot.
//! Adapters authenticate journal provenance and committed storage, project every field,
//! and retain the endpoint registry. These checks do not prove durable filesystem writes
//! or prevent independent storage administration from removing a selected generation.

use super::*;

verus! {

/// Decoded journal classification, carrying no storage authentication or execution permission.
pub enum RecordedResolution {
    /// Exact VM originally classified as nonnative.
    NonNative(
        /// Original VM name, retained even without a native backend.
        String,
    ),
    /// Exact VM retaining the explicit unchanged/unpinned policy.
    Existing(
        /// Original VM whose unchanged-storage policy was recorded.
        String,
    ),
    /// Full original generation and guest boot layout.
    Retained(
        /// Complete generation identity, including its consuming VM and layout.
        Box<Selection>,
    ),
}

/// Inert selection evidence retained inside the authoritative deployment journal.
pub struct Snapshot {
    /// Stable deployment operation, independent of the current recovery attempt.
    pub operation: String,
    /// Original protected source decision and scope encoding.
    pub authority: String,
    /// Canonical target release path and content fingerprint.
    pub target: String,
    /// Complete original coordinate order, including explicit compatibility classifications.
    pub entries: Vec<RecordedResolution>,
}

/// Recovered selections preserve both concrete identities and explicit unchanged classifications.
pub open spec fn retained(d: Definition, saved: RecordedResolution, fresh: Resolution) -> bool {
    match (saved, fresh) {
        (RecordedResolution::NonNative(name), Resolution::NonNative) => name@ == d.name@,
        (RecordedResolution::Existing(name), Resolution::Existing) => name@ == d.name@,
        (RecordedResolution::Retained(old), Resolution::Retained(new)) => crate::boot::same(*old, new@),
        _ => false,
    }
}

/// Stable identity and the entire target-specific selection set survive a changed attempt/plan.
pub open spec fn preserves(saved: Snapshot, c: ExecutionContext, target: Seq<char>,
    definitions: Seq<Definition>, entries: Seq<Resolution>) -> bool {
    saved.operation@ == c.operation@ && saved.authority@ == c.authority@ && saved.target@ == target
        && saved.entries.len() == definitions.len() && entries.len() == definitions.len()
        && forall|i: int| 0 <= i < definitions.len() ==>
            retained(#[trigger] definitions[i], saved.entries[i], entries[i])
}

fn entry_preserved(d: &Definition, saved: &RecordedResolution, fresh: &Resolution) -> (yes: bool)
    ensures yes == retained(*d, *saved, *fresh),
{
    match (saved, fresh) {
        (RecordedResolution::NonNative(name), Resolution::NonNative) => name.as_str() == d.name.as_str(),
        (RecordedResolution::Existing(name), Resolution::Existing) => name.as_str() == d.name.as_str(),
        (RecordedResolution::Retained(old), Resolution::Retained(new)) => crate::boot::matching(old, new.selection()),
        _ => false,
    }
}

/// Compare every recorded coordinate and identity rather than accepting a publication flag.
fn snapshot_matches(plan: &StoragePlan, saved: &Snapshot, c: &ExecutionContext, target: &str,
    entries: &[Resolution]) -> (yes: bool)
    ensures yes == preserves(*saved, *c, target@, plan@.1, entries@),
{
    if saved.operation.as_str() != c.operation.as_str() || saved.authority.as_str() != c.authority.as_str()
        || saved.target.as_str() != target || saved.entries.len() != entries.len()
        || entries.len() != plan.definitions.len() { return false; }

    let mut i = 0;
    while i < entries.len()
        invariant 0 <= i <= entries.len(), entries.len() == plan.definitions.len(),
            saved.entries.len() == entries.len(),
            forall|j: int| 0 <= j < i ==> retained(#[trigger] plan.definitions[j], saved.entries[j], entries[j]),
        decreases entries.len() - i,
    {
        if !entry_preserved(&plan.definitions[i], &saved.entries[i], &entries[i]) {
            assert(!retained(plan@.1[i as int], saved.entries@[i as int], entries@[i as int]));
            return false;
        }
        i += 1;
    }
    true
}

/// An authenticated complete selection set waiting for authoritative intent publication.
/// It cannot dispatch storage effects; activation still needs the current durable gate.
///
/// ```compile_fail,E0451
/// use airlock_deployment_core::storage::staging::{StagedGenerations, persistence::PinnedGenerations};
/// /// Constructing a pin without the prepublication check is forbidden.
/// fn bypass(staged: StagedGenerations) -> PinnedGenerations {
///     PinnedGenerations { staged, target: "candidate".into() }
/// }
/// ```
pub struct PinnedGenerations {
    // Complete checked entries bound to the new attempt and its full current plan.
    staged: StagedGenerations,
    // Immutable target endpoint, distinct from recovery's possibly reversed baseline.
    target: String,
}

impl View for PinnedGenerations {
    type V = (StagedGenerations, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.staged, self.target@) }
}

impl StoragePlan {
    /// Validate selections before publication, preserving any original target snapshot exactly.
    /// This grants no execution authority and performs no external effect.
    pub fn pin(&self, c: &ExecutionContext, target: String, saved: Option<&Snapshot>,
        entries: Vec<Resolution>) -> (out: Option<PinnedGenerations>)
        ensures out.is_some() == (target@.len() > 0 && self@.2 == c.plan@
            && complete(self@.1, self@.3, entries@)
            && (saved.is_some() ==> preserves(*saved.unwrap(), *c, target@, self@.1, entries@))),
            out.is_some() ==> out.unwrap()@.1 == target@
                && out.unwrap()@.0@ == (entries@, c.attempt@, c.operation@, c.authority@, c.plan@),
            out.is_some() ==> complete(self@.1, self@.3, out.unwrap()@.0@.0),
    {
        if target.is_empty() || self.binding.as_str() != c.plan.as_str() || !complete_entries(self, &entries) { return None; }
        if saved.is_some() && !snapshot_matches(self, saved.unwrap(), c, target.as_str(), &entries) { return None; }
        Some(PinnedGenerations { staged: StagedGenerations { entries, attempt: c.attempt.as_str().to_string(),
            operation: c.operation.as_str().to_string(), authority: c.authority.as_str().to_string(), plan: c.plan.as_str().to_string() }, target })
    }

    /// Consume the prepublication pin only under its unchanged, successfully published execution intent.
    pub fn activate_pinned(&self, durable: &DurableOperation, c: &ExecutionContext,
        pinned: PinnedGenerations, saved: &Snapshot) -> (out: Option<StagedGenerations>)
        ensures out.is_some() == (crate::operation::executing(durable, c) && self@.2 == c.plan@
            && matches_context(pinned@.0, *c) && complete(self@.1, self@.3, pinned@.0@.0)
            && preserves(*saved, *c, pinned@.1, self@.1, pinned@.0@.0)),
            out.is_some() ==> out.unwrap()@ == pinned@.0@
                && complete(self@.1, self@.3, out.unwrap()@.0) && matches_context(out.unwrap(), *c),
            out.is_some() ==> preserves(*saved, *c, pinned@.1, self@.1, out.unwrap()@.0),
    {
        let staged = pinned.staged;
        if !durable.allows_effect(c) || self.binding.as_str() != c.plan.as_str()
            || staged.attempt.as_str() != c.attempt.as_str() || staged.operation.as_str() != c.operation.as_str()
            || staged.authority.as_str() != c.authority.as_str() || staged.plan.as_str() != c.plan.as_str()
            || !complete_entries(self, &staged.entries)
            || !snapshot_matches(self, saved, c, pinned.target.as_str(), &staged.entries) { return None; }
        Some(staged)
    }
}

impl PinnedGenerations {
    /// Read the complete checked coordinate set for journal encoding before publication.
    pub fn entries(&self) -> (out: &[Resolution]) ensures out@ == self@.0@.0,
    { &self.staged.entries }

    /// Retain the stable operation rather than a recovery attempt's new identity.
    pub fn operation(&self) -> (out: &str) ensures out@ == self@.0@.2,
    { self.staged.operation.as_str() }

    /// Retain the unchanged approval and permitted effect scope.
    pub fn authority(&self) -> (out: &str) ensures out@ == self@.0@.3,
    { self.staged.authority.as_str() }

    /// Retain the exact fingerprinted release endpoint for this set of selections.
    pub fn target(&self) -> (out: &str) ensures out@ == self@.1,
    { self.target.as_str() }
}

}
