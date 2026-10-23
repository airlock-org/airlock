//! Admission claims become usable evidence only after independent storage inspection.
//! Protected release loading, source approval, record authentication and field decoding remain adapters.

use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// Full validated boot-image identity; equal digests alone do not erase size or boot-contract differences.
pub struct Root {
    /// Canonical sparse stream digest.
    pub digest: String,
    /// Reconstructed disk length.
    pub logical_bytes: u64,
    /// Canonical encoded stream length.
    pub payload_bytes: u64,
    /// Supported filesystem format supplied by the descriptor decoder.
    pub format: String,
    /// Supported guest boot contract supplied by that same descriptor.
    pub boot_abi: String,
}

/// Immutable guest artifacts whose exact identities survive generation reuse.
pub struct Artifacts {
    /// Built system closure.
    pub system: String,
    /// Original immutable root source.
    pub root_source: String,
    /// Paired kernel selection.
    pub kernel: String,
    /// Independently validated sparse root descriptor.
    pub root: Root,
}

/// Recorded source provenance, retained rather than rewritten when a generation is reused.
pub struct Source {
    /// Full submitted commit identity.
    pub revision: String,
    /// Exact dependency lock identity.
    pub lock_hash: String,
}

/// Decoded transport claims; no field here independently grants execution authority.
pub struct Claim {
    /// Consuming guest, not its storage namespace.
    pub guest: String,
    /// Positive admitted generation.
    pub generation: u64,
    /// Manifest independently addressed during inspection.
    pub manifest: String,
    /// Claimed immutable boot artifacts.
    pub artifacts: Artifacts,
    /// Complete closure content identity.
    pub closure_hash: String,
    /// Provenance actually recorded in the admitted generation.
    pub source: Source,
    /// Explicit successful receipt outcome.
    pub passed: bool,
    /// Claimed activation outcome; admission requires false.
    pub activated: bool,
    /// Claimed retention outcome; admission requires true.
    pub retained: bool,
    /// Explicit reuse claim, with omission retaining the fresh-source rule.
    pub reused: Option<bool>,
}

/// Recognized admission sender decoded from protected generation approval metadata.
#[derive(Clone, Copy)]
pub enum Sender {
    /// Whole-machine controller.
    Controller,
    /// Existing separately authorized interactive guest-admission path.
    Assistant,
}

/// Facts freshly read from committed storage, independently of the transport receipt.
pub struct Inspection {
    /// Actual runtime storage namespace.
    pub namespace: String,
    /// Actual committed generation.
    pub generation: u64,
    /// Actual committed manifest coordinate.
    pub manifest: String,
    /// Independently inspected immutable artifacts.
    pub artifacts: Artifacts,
    /// Actual closure descriptor source.
    pub closure_source: String,
    /// Actual immutable closure content identity.
    pub closure_hash: String,
    /// Actual recorded provenance.
    pub source: Source,
    /// Successful generation inspection.
    pub passed: bool,
    /// Successful storage integrity check.
    pub verified: bool,
    /// Actual retained-generation protection.
    pub retained: bool,
    /// Sender from the protected approval record.
    pub sender: Sender,
    /// Original source covered by that approval record.
    pub approval_source: String,
}

/// One guest's artifacts and current source requirement from an authenticated candidate release.
pub struct Expected {
    /// Exact guest declared by the release.
    pub guest: String,
    /// Exact built boot artifacts declared for this guest.
    pub artifacts: Artifacts,
    /// Exact closure source declared by the release.
    pub closure_source: String,
    /// Current candidate source, distinct from an older reused generation's provenance.
    pub source: Source,
    /// Complete original release fingerprint supplied by protected release loading.
    pub candidate: String,
}

/// Full root identity equality, independent of receipt layout or field order.
pub open spec fn same_root(a: Root, b: Root) -> bool {
    a.digest@ == b.digest@ && a.logical_bytes == b.logical_bytes && a.payload_bytes == b.payload_bytes
        && a.format@ == b.format@ && a.boot_abi@ == b.boot_abi@
}

/// Immutable artifact equality never substitutes source provenance for actual artifact identity.
pub open spec fn same_artifacts(a: Artifacts, b: Artifacts) -> bool {
    a.system@ == b.system@ && a.root_source@ == b.root_source@ && a.kernel@ == b.kernel@ && same_root(a.root, b.root)
}

/// Commit and dependency identities must agree together.
pub open spec fn same_source(a: Source, b: Source) -> bool { a.revision@ == b.revision@ && a.lock_hash@ == b.lock_hash@ }

/// A successful transport claim is still only a claim; this predicate grants no storage authority.
pub open spec fn claim_matches(c: Claim, e: Expected) -> bool {
    c.guest@.len() > 0 && c.generation > 0 && c.passed && c.retained && !c.activated
        && c.guest@ == e.guest@ && same_artifacts(c.artifacts, e.artifacts)
        && (same_source(c.source, e.source) || c.reused == Some(true))
}

/// Every accepted claim agrees with the actual committed, verified, retained generation and approval source.
pub open spec fn authenticated(c: Claim, i: Inspection) -> bool {
    c.guest@.len() > 0 && c.generation > 0 && c.manifest@.len() > 0 && c.passed && c.retained && !c.activated
        && i.passed && i.verified && i.retained && i.namespace@ == c.guest@ + "-runtime-store"@
        && i.generation == c.generation && i.manifest@ == c.manifest@
        && same_artifacts(c.artifacts, i.artifacts) && c.closure_hash@ == i.closure_hash@
        && same_source(c.source, i.source) && i.approval_source@ == i.closure_source@
}

/// Reuse permits old provenance only after actual immutable artifacts and closure source match the candidate.
pub open spec fn candidate_matches(c: Claim, i: Inspection, e: Expected) -> bool {
    authenticated(c, i) && claim_matches(c, e) && i.sender is Controller
        && i.closure_source@ == e.closure_source@ && e.candidate@.len() > 0
}

fn root_matches(a: &Root, b: &Root) -> (yes: bool) ensures yes == same_root(*a, *b),
{ a.digest == b.digest && a.logical_bytes == b.logical_bytes && a.payload_bytes == b.payload_bytes && a.format == b.format && a.boot_abi == b.boot_abi }

fn artifacts_match(a: &Artifacts, b: &Artifacts) -> (yes: bool) ensures yes == same_artifacts(*a, *b),
{ a.system == b.system && a.root_source == b.root_source && a.kernel == b.kernel && root_matches(&a.root, &b.root) }

fn source_matches(a: &Source, b: &Source) -> (yes: bool) ensures yes == same_source(*a, *b),
{ a.revision == b.revision && a.lock_hash == b.lock_hash }

/// Check transport claims consistently on the worker/controller side; independent authentication remains mandatory in dom0.
pub fn check_claim(c: &Claim, e: &Expected) -> (yes: bool) ensures yes == claim_matches(*c, *e),
{
    !c.guest.is_empty() && c.generation > 0 && c.passed && c.retained && !c.activated
        && c.guest == e.guest && artifacts_match(&c.artifacts, &e.artifacts)
        && (source_matches(&c.source, &e.source) || matches!(c.reused, Some(true)))
}

/// Receipt evidence sealed by comparison with independently loaded committed storage.
pub struct Authenticated {
    // Original receipt claims; mutable access is never exposed.
    claim: Claim,
    // Full independent inspection and approval evidence.
    inspection: Inspection,
}

impl Authenticated {
    /// Original claims retained for subsequent candidate binding.
    pub closed spec fn claims(&self) -> Claim { self.claim }

    /// Original independently inspected authority.
    pub closed spec fn observed(&self) -> Inspection { self.inspection }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool { authenticated(self.claim, self.inspection) }

    /// Fail closed on every identity, provenance, status or approval-source mismatch.
    pub fn check(claim: Claim, inspection: Inspection) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().claims() == claim && out.unwrap().observed() == inspection,
    {
        if claim.guest.is_empty() || claim.generation == 0 || claim.manifest.is_empty()
            || !claim.passed || !claim.retained || claim.activated || !inspection.passed || !inspection.verified || !inspection.retained
            || inspection.namespace != crate::names::runtime_namespace(claim.guest.as_str())
            || inspection.generation != claim.generation || inspection.manifest != claim.manifest
            || !artifacts_match(&claim.artifacts, &inspection.artifacts) || inspection.closure_hash != claim.closure_hash
            || !source_matches(&claim.source, &inspection.source) || inspection.approval_source != inspection.closure_source { return None; }
        Some(Self { claim, inspection })
    }

    /// Preserve the original recognized sender policy without granting controller authority to an assistant receipt.
    pub fn controller(&self) -> (yes: bool) ensures yes == (self.observed().sender is Controller),
    { matches!(self.inspection.sender, Sender::Controller) }

    /// Bind authenticated storage evidence to one exact candidate; the reuse flag alone never suffices.
    pub fn bind(self, expected: Expected) -> (out: Option<Bound>)
        ensures out.is_some() ==> out.unwrap().evidence() == self && out.unwrap().expectation() == expected,
    {
        proof { use_type_invariant(&self); }
        if !check_claim(&self.claim, &expected) || !self.controller() || expected.candidate.is_empty()
            || self.inspection.closure_source != expected.closure_source { return None; }
        Some(Bound { evidence: self, expected })
    }
}

/// Candidate-specific admission evidence; this grants no deployment activation authority.
///
/// ```compile_fail,E0616
/// use airlock_machine_planning::admission::{Bound, Expected};
/// /// A caller cannot attach an authenticated generation to a different candidate.
/// fn substitute(bound: &mut Bound, expected: Expected) { bound.expected = expected; }
/// ```
pub struct Bound {
    // Original independently authenticated claims and inspection.
    evidence: Authenticated,
    // Exact original candidate and expected immutable artifacts.
    expected: Expected,
}

impl Bound {
    /// Original independent authentication result.
    pub closed spec fn evidence(&self) -> Authenticated { self.evidence }

    /// Complete original candidate expectation.
    pub closed spec fn expectation(&self) -> Expected { self.expected }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool { candidate_matches(self.evidence.claim, self.evidence.inspection, self.expected) }

    /// Exact admitted guest for candidate publication.
    pub fn guest(&self) -> (out: &str) ensures out@ == self.evidence().claims().guest@,
    { self.evidence.claim.guest.as_str() }

    /// Exact independently inspected generation, without looking up latest again.
    pub fn generation(&self) -> (out: u64) ensures out == self.evidence().claims().generation,
    { self.evidence.claim.generation }

    /// Original generation provenance, including valid historical provenance on reuse.
    pub fn source(&self) -> (out: &Source) ensures *out == self.evidence().claims().source,
    { &self.evidence.claim.source }
}

/// Full candidate expectation equality for complete receipt-set checking.
pub open spec fn same_expected(a: Expected, b: Expected) -> bool {
    a.guest@ == b.guest@ && same_artifacts(a.artifacts, b.artifacts) && a.closure_source@ == b.closure_source@
        && same_source(a.source, b.source) && a.candidate@ == b.candidate@
}

fn expected_matches(a: &Expected, b: &Expected) -> (yes: bool) ensures yes == same_expected(*a, *b),
{ a.guest == b.guest && artifacts_match(&a.artifacts, &b.artifacts) && a.closure_source == b.closure_source && source_matches(&a.source, &b.source) && a.candidate == b.candidate }

/// Complete, duplicate-free guest evidence whose generation fields may be published into one candidate.
pub struct BoundSet {
    // Every original bound receipt in the canonical order supplied by the release adapter.
    entries: Vec<Bound>,
    // Original candidate binding shared by the complete collection.
    candidate: String,
    // Erased complete expected guest list, independent of receipt enumeration.
    _expected: Ghost<Seq<Expected>>,
}

impl View for BoundSet {
    type V = Seq<Bound>;

    closed spec fn view(&self) -> Self::V { self.entries@ }
}

impl BoundSet {
    /// Original release binding retained by the complete collection.
    pub closed spec fn binding(&self) -> Seq<char> { self.candidate@ }

    /// Forward the same release binding to candidate publication.
    pub fn candidate(&self) -> (out: &str) ensures out@ == self.binding(),
    { self.candidate.as_str() }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.candidate@.len() > 0 && self.entries.len() == self._expected@.len()
            && (forall|i: int| 0 <= i < self.entries.len() ==> same_expected(#[trigger] self.entries[i].expectation(), self._expected@[i])
                && self.entries[i].expectation().candidate@ == self.candidate@)
            && (forall|i: int, j: int| 0 <= j < i < self._expected@.len() ==> self._expected@[i].guest@ != self._expected@[j].guest@)
    }

    /// Refuse missing, extra, reordered, duplicate or substituted candidate evidence before publication.
    pub fn new(expected: &[Expected], entries: Vec<Bound>, candidate: String) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == entries@,
    {
        if candidate.is_empty() || expected.len() != entries.len() { return None; }
        let mut i = 0;
        while i < entries.len()
            invariant 0 <= i <= entries.len(), expected.len() == entries.len(), candidate@.len() > 0,
                forall|k: int| 0 <= k < i ==> same_expected(#[trigger] entries[k].expectation(), expected[k]) && entries[k].expectation().candidate@ == candidate@,
                forall|k: int, j: int| 0 <= j < k < i ==> expected[k].guest@ != expected[j].guest@,
            decreases entries.len() - i,
        {
            if !expected_matches(&expected[i], &entries[i].expected) || entries[i].expected.candidate != candidate { return None; }
            let mut j = 0;
            while j < i
                invariant 0 <= j <= i < expected.len(), forall|k: int| 0 <= k < j ==> expected[i as int].guest@ != expected[k].guest@,
                decreases i - j,
            {
                if expected[i].guest == expected[j].guest { return None; }
                j += 1;
            }
            i += 1;
        }
        Some(Self { entries, candidate, _expected: Ghost(expected@) })
    }

    /// Publish only the complete collection accepted by this candidate's binding gate.
    pub fn entries(&self) -> (out: &[Bound]) ensures out@ == self@,
    { &self.entries }
}

}
