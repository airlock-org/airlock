//! Authorise an exact private-volume replacement and refuse uncertain destructive replay.
//! Filesystem acknowledgments and Qubes observations are adapter inputs, not proof of disk contents.
//!
//! ## Private-volume replacement
//!
//! The [copy boundary](crate::copy) keeps the original pair order and resource definitions
//! under the complete plan binding. Names, registration UUIDs, pool/volume identities and
//! the destination configuration must agree across preflight, the protected receipt and
//! current named observations. A clone requires explicitly halted endpoints and enough
//! capacity. The request owns neither an arbitrary VM name nor a new approval flag.
//!
//! `CopyIntent` is not accepted by the Qubes adapter. The coordinator first pins its
//! validated destination and durably saves `Copying`, then consumes the intent into a
//! clone request. This publication acknowledgment relies on truthful filesystem success.
//! A `Copying` receipt after an error or restart represents uncertainty; it blocks automatic
//! copying and needs explicit reconciliation. A complete receipt never requests a clone,
//! including when the destination has since been used. Recovery preflight also executes
//! verified comparison against the prior authorised journal’s copy definition, preserving
//! source identity, destination configuration, any pinned target and monotonic progress.
//! An incomplete copy missing that original intent is refused. Journal selection and
//! faithful decoding remain adapter duties. This is a safety guarantee, not
//! exactly-once execution or guaranteed automatic recovery. Qubes must preserve the observed
//! registrations and disks through its name-based clone calls; independent administrators
//! can race the machine lock. Neither disk contents nor that external atomicity are proved.

use crate::{
    effects::{self, Targets},
    operation::{DurableOperation, ExecutionContext, PlanBinding},
};
use vstd::prelude::*;

verus! {

/// Registered VM and independent persistent private-volume identity.
pub struct Volume {
    /// Qubes VM name from the checked copy plan or a named observation.
    pub name: String,
    /// Registration identity, detecting name reuse.
    pub uuid: String,
    /// Storage pool identity.
    pub pool: String,
    /// Volume identity within that pool.
    pub vid: String,
}

/// The complete resource identity, rather than a VM name alone.
pub open spec fn same(a: Volume, b: Volume) -> bool {
    a.name@ == b.name@ && a.uuid@ == b.uuid@ && a.pool@ == b.pool@ && a.vid@ == b.vid@
}

/// Compare every component of a decoded private-volume identity.
fn equal(a: &Volume, b: &Volume) -> (yes: bool)
    ensures yes == same(*a, *b),
{ a.name == b.name && a.uuid == b.uuid && a.pool == b.pool && a.vid == b.vid }

/// An absent destination has not been pinned; a recorded one must still match exactly.
fn pinned_matches(pinned: &Option<Volume>, target: &Volume) -> (yes: bool)
    ensures yes == match pinned { Some(v) => same(*v, *target), None => true },
{
    match pinned { Some(v) => equal(v, target), None => true }
}

/// Immutable preflight evidence already included in the complete plan binding.
pub struct Definition {
    /// Original source VM, registration and disk.
    pub source: Volume,
    /// Destination VM created or reconciled by the checked configuration plan.
    pub target: String,
    /// Exact destination configuration digest.
    pub configuration: String,
    /// Preflight progress cannot be rolled back by a later stale receipt.
    pub status: Status,
    /// Previously pinned destination when resuming an existing copy record.
    pub pinned: Option<Volume>,
}

/// Persistent progress never implies that an unrecorded external effect failed.
#[derive(Clone, Copy)]
pub enum Status {
    /// No clone has been requested under this receipt.
    Planned,
    /// Clone may have completed; automatic overwrite is refused.
    Copying,
    /// Completion recorded; recovery must not overwrite newer private data.
    Complete,
}

/// Recovery may advance recorded progress but cannot regain initial-copy permission.
pub open spec fn progressed(initial: Status, current: Status) -> bool {
    match initial { Status::Planned => true, Status::Copying => !(current is Planned), Status::Complete => current is Complete }
}

/// Compare the actual recorded states without treating uncertainty as failure.
fn progress_matches(initial: Status, current: Status) -> (yes: bool)
    ensures yes == progressed(initial, current),
{
    match initial { Status::Planned => true, Status::Copying => !matches!(current, Status::Planned), Status::Complete => matches!(current, Status::Complete) }
}

/// A recovery preflight preserves original copy resources and cannot regress protected progress.
pub open spec fn compatible(original: Definition, current: Definition) -> bool {
    same(original.source, current.source) && original.target@ == current.target@
        && original.configuration@ == current.configuration@ && progressed(original.status, current.status)
        && match original.pinned { Some(a) => current.pinned.is_some() && same(a, current.pinned.unwrap()), None => true }
}

/// Check resources retained in the prior authorised journal before accepting fresh recovery observations.
pub fn preserves(original: &Definition, current: &Definition) -> (accepted: bool)
    ensures accepted == compatible(*original, *current),
{
    if !equal(&original.source, &current.source) || original.target != current.target
        || original.configuration != current.configuration || !progress_matches(original.status, current.status) { return false; }
    match &original.pinned {
        Some(pinned) => match &current.pinned { Some(now) => equal(pinned, now), None => false },
        None => true,
    }
}

/// Missing prior copy authority permits only a completed no-op, never a fresh overwrite.
/// Otherwise every preflight resource and recorded progress must preserve the original intent.
pub fn recovery(original: Option<&Definition>, current: &Definition) -> (accepted: bool)
    ensures accepted == match original { Some(saved) => compatible(*saved, *current), None => current.status is Complete },
{
    match original { Some(saved) => preserves(saved, current), None => matches!(current.status, Status::Complete) }
}

/// Decoded protected receipt; constructing evidence grants no clone permission.
pub struct Receipt {
    /// Original recorded source identity.
    pub source: Volume,
    /// Recorded destination name.
    pub target: String,
    /// Recorded destination configuration digest.
    pub configuration: String,
    /// Destination identity pinned before cloning.
    pub pinned: Option<Volume>,
    /// Durable copy progress.
    pub status: Status,
}

/// Current named observations from Qubes' volume and power interfaces.
pub struct Observation {
    /// Currently registered source and disk.
    pub source: Volume,
    /// Currently registered destination and disk.
    pub target: Volume,
    /// Verified classification of the named source's raw power state.
    pub source_power: crate::inventory::PowerState,
    /// Verified classification of the named destination's raw power state.
    pub target_power: crate::inventory::PowerState,
    /// Observed source volume capacity.
    pub source_size: u64,
    /// Observed destination volume capacity.
    pub target_size: u64,
}

/// Receipt, preflight and observation name the same operation and independent disks.
pub open spec fn identities(d: Definition, r: Receipt, o: Observation) -> bool {
    &&& progressed(d.status, r.status)
    &&& same(d.source, r.source) && same(d.source, o.source)
    &&& d.target@ == r.target@ && d.target@ == o.target.name@
    &&& d.configuration@ == r.configuration@
    &&& match d.pinned { Some(v) => same(v, o.target), None => true }
    &&& match r.pinned { Some(v) => same(v, o.target), None => !(r.status is Complete) }
    &&& (o.source.pool@ != o.target.pool@ || o.source.vid@ != o.target.vid@)
}

/// Exactly the safely repeatable initial copy; uncertain and completed states never issue a clone.
pub open spec fn copyable(d: Definition, r: Receipt, o: Observation) -> bool {
    identities(d, r, o) && r.status is Planned
        && o.source_power@ == crate::inventory::PowerClass::Halted
        && o.target_power@ == crate::inventory::PowerClass::Halted
        && o.target_size >= o.source_size
}

/// Immutable checked endpoint sets, definitions and candidate/baseline/plan binding.
pub struct CopyPlan {
    // Source scope admitted by the original approval.
    sources: Targets,
    // Destination scope admitted by the same approval.
    targets: Targets,
    // Exact preflight identities cannot be replaced at dispatch.
    definitions: Vec<Definition>,
    // Complete candidate, baseline, revision and plan binding.
    binding: PlanBinding,
}

impl View for CopyPlan {
    type V = (effects::Scope, effects::Scope, Seq<Definition>, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.sources@, self.targets@, self.definitions@, self.binding@) }
}

/// Each retained pair occurs at the same index in the checked copy fields.
pub open spec fn valid(s: (effects::Scope, effects::Scope, Seq<Definition>, Seq<char>)) -> bool {
    &&& effects::valid(s.0) && effects::valid(s.1)
    &&& s.2.len() == s.0.targets.len() && s.2.len() == s.1.targets.len()
    &&& forall|i: int| 0 <= i < s.2.len() ==> #[trigger] s.2[i].source.name@ == s.0.targets[i]@
        && s.2[i].target@ == s.1.targets[i]@
}

impl CopyPlan {
    /// Refuse missing, extra, reordered or substituted copy pairs before publication.
    pub(crate) fn new(sources: Targets, targets: Targets, definitions: Vec<Definition>, binding: PlanBinding)
        -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@.3 == binding@ && out.unwrap()@.2 == definitions@,
            out.is_some() ==> out.unwrap()@.0 == sources@ && out.unwrap()@.1 == targets@,
            out.is_some() && effects::valid(sources@) && effects::valid(targets@) ==> valid(out.unwrap()@),
    {
        let src = sources.names();
        let dst = targets.names();
        if definitions.len() != src.len() || definitions.len() != dst.len() { return None; }
        let mut i = 0;
        while i < definitions.len()
            invariant 0 <= i <= definitions.len(), definitions.len() == src.len(), definitions.len() == dst.len(),
                forall|j: int| 0 <= j < i ==> #[trigger] definitions[j].source.name@ == src[j]@ && definitions[j].target@ == dst[j]@,
            decreases definitions.len() - i,
        {
            if definitions[i].source.name.as_str() != src[i].as_str() || definitions[i].target.as_str() != dst[i].as_str() { return None; }
            assert(definitions[i as int].source.name@ == src[i as int]@);
            assert(definitions[i as int].target@ == dst[i as int]@);
            proof {
                assert forall|j: int| 0 <= j < i + 1 implies #[trigger] definitions[j].source.name@ == src[j]@ && definitions[j].target@ == dst[j]@ by {
                    if j == i { } else { assert(j < i); }
                }
            }
            i += 1;
        }
        Some(Self { sources, targets, definitions, binding })
    }

    /// Select only a checked pair with unchanged authority, identities and safe observed state.
    /// Complete receipts yield no operation; Copying is an uncertain outcome requiring reconciliation.
    pub fn prepare<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        index: usize, receipt: &Receipt, observed: &'a Observation) -> (out: Result<Option<CopyIntent<'a>>, Denial>)
        ensures out.is_ok() ==> crate::operation::executing(durable, context) && self@.3 == context.plan@,
            out.is_ok() ==> index < self@.2.len() && identities(self@.2[index as int], *receipt, *observed),
            out.is_ok() && out.unwrap().is_none() ==> receipt.status is Complete,
            out.is_ok() && out.unwrap().is_some() ==> copyable(self@.2[index as int], *receipt, *observed),
            out.is_ok() && out.unwrap().is_some() ==> out.unwrap().unwrap()@ == (self@.2[index as int], *observed),
            out.is_ok() && out.unwrap().is_some() && valid(self@) && self@.0.reviewed ==> self@.0.allowed.contains(observed.source.name@),
            out.is_ok() && out.unwrap().is_some() && valid(self@) && self@.1.reviewed ==> self@.1.allowed.contains(observed.target.name@),
    {
        if self.binding.as_str() != context.plan.as_str() || !durable.allows_effect(context) { return Err(Denial::Context); }
        if index >= self.definitions.len() { return Err(Denial::Identity); }
        let d = &self.definitions[index];
        if !progress_matches(d.status, receipt.status) { return Err(Denial::Identity); }
        if !self.sources.contains(d.source.name.as_str()) || !self.targets.contains(d.target.as_str()) { return Err(Denial::Identity); }
        if !equal(&d.source, &receipt.source) || !equal(&d.source, &observed.source)
            || d.target != receipt.target || d.target != observed.target.name
            || d.configuration != receipt.configuration { return Err(Denial::Identity); }
        if !pinned_matches(&d.pinned, &observed.target) { return Err(Denial::Identity); }
        if let Some(pinned) = &receipt.pinned { if !equal(pinned, &observed.target) { return Err(Denial::Identity); } }
        else if matches!(receipt.status, Status::Complete) { return Err(Denial::Identity); }
        if observed.source.pool == observed.target.pool && observed.source.vid == observed.target.vid { return Err(Denial::Identity); }
        if matches!(receipt.status, Status::Complete) { return Ok(None); }
        if matches!(receipt.status, Status::Copying) { return Err(Denial::Uncertain); }
        if !observed.source_power.is_halted() || !observed.target_power.is_halted()
            || observed.target_size < observed.source_size { return Err(Denial::State); }
        proof {
            if valid(self@) {
                let i = index as int;
                assert(self@.0.targets[i]@ == observed.source.name@);
                assert(self@.1.targets[i]@ == observed.target.name@);
            }
        }
        Ok(Some(CopyIntent { definition: d, observed, _durable: durable }))
    }
}

/// Rejection never yields an executable clone request.
#[derive(Debug)]
pub enum Denial {
    /// Authority, attempt, candidate, baseline or durable gate differs.
    Context,
    /// Receipt or observed resource differs from the original pair.
    Identity,
    /// Capacity or halted-state conditions do not hold.
    State,
    /// A previously requested clone has an unrecorded outcome.
    Uncertain,
}

/// Validated copy parameters awaiting durable Copying publication; not accepted by Qubes.
///
/// ```compile_fail
/// use airlock_deployment_core::copy::{CopyIntent, CopyRequest};
/// fn adapter(_: CopyRequest<'_>) {}
/// fn bypass(intent: CopyIntent<'_>) { adapter(intent); }
/// ```
pub struct CopyIntent<'a> {
    // Exact preflight pair whose membership and resource checks passed.
    definition: &'a Definition,
    // Named, validated source and target observations.
    observed: &'a Observation,
    // Prevents gate interruption before publication and dispatch finish.
    _durable: &'a DurableOperation,
}

impl<'a> View for CopyIntent<'a> {
    type V = (Definition, Observation);

    closed spec fn view(&self) -> Self::V { (*self.definition, *self.observed) }
}

impl<'a> CopyIntent<'a> {
    /// Borrow the exact target to pin in the Copying receipt.
    pub fn target(&self) -> (target: &Volume)
        ensures same(*target, self@.1.target),
    { &self.observed.target }

    /// Consume the intent after the adapter acknowledges durable Copying publication.
    /// Truthful success of file/parent fsync is an explicit trusted adapter contract.
    pub fn published(self) -> (request: CopyRequest<'a>)
        ensures request@ == self@,
    { CopyRequest { intent: self } }
}

/// The Qubes clone adapter accepts this request, never independent source/target names.
pub struct CopyRequest<'a> {
    // Publication consumes the validated intent; callers cannot construct this field.
    intent: CopyIntent<'a>,
}

impl<'a> View for CopyRequest<'a> {
    type V = (Definition, Observation);

    closed spec fn view(&self) -> Self::V { self.intent@ }
}

impl<'a> CopyRequest<'a> {
    /// Sole source passed to Qubes CloneFrom.
    pub fn source(&self) -> (source: &str)
        ensures source@ == self@.0.source.name@,
    { self.intent.definition.source.name.as_str() }

    /// Sole destination passed to Qubes CloneTo.
    pub fn target(&self) -> (target: &str)
        ensures target@ == self@.0.target@,
    { self.intent.definition.target.as_str() }
}

}
