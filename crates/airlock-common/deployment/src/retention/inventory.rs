//! Interpret complete decoded source groups before permitting retention changes.
//! Filesystem enumeration, authentication and raw record decoding remain adapter duties.

use super::*;
use crate::attachment::runtime_namespace;
use crate::storage::staging::persistence::RecordedResolution;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// Durable release disposition; an interrupted prune still requires protection.
#[derive(Clone, Copy)]
pub enum ReleaseState {
    /// Release roots or recovery references have not finished pruning.
    Present,
    /// The authoritative pruning journal records completed removal.
    Pruned,
}

/// One effective guest definition, independent of manifest generation pins.
pub struct EffectiveSystem {
    /// Guest identity before its runtime storage namespace is derived.
    pub domain: String,
    /// Effective immutable software source.
    pub system: String,
}

/// A manifest pin names a guest before its pool namespace is derived.
/// This cannot be substituted with a pool-observation `Generation`.
pub struct GuestPin {
    /// Original guest name from the release manifest.
    pub guest: String,
    /// Explicit positive generation within that guest's runtime storage namespace.
    pub number: u64,
}

/// All configuration and numeric pins decoded from one release.
pub struct ReleaseSource {
    /// Durable pruning disposition, never inferred from a missing file.
    pub state: ReleaseState,
    /// Complete effective guest definitions.
    pub systems: Vec<EffectiveSystem>,
    /// Complete explicit manifest pins, using guest names rather than pool namespaces.
    pub pins: Vec<GuestPin>,
}

/// Every original selection from one authenticated journal endpoint.
pub struct JournalSource {
    /// Completed pruning is the only disposition that drops endpoint protections.
    pub state: ReleaseState,
    /// Complete coordinate list, including existing and nonnative classifications.
    pub entries: Vec<RecordedResolution>,
}

/// One committed pool manifest before retention classification.
pub struct StoredGeneration {
    /// Exact namespace and positive generation from the authenticated record.
    pub generation: Generation,
    /// Decoded retention field; missing fields are errors at ingress.
    pub retained: bool,
}

/// Complete observations and policy choices collected under the pool lock.
pub struct PoolSources {
    /// Every committed manifest, including unselected and unretained generations.
    pub committed: Vec<StoredGeneration>,
    /// Every decoded current selection, before membership checking.
    pub selected: Vec<Generation>,
    /// Every decoded running reference, including references with missing logical records.
    pub running: Vec<Generation>,
    /// Every generation chosen by the existing recent-generation policy.
    pub recent: Vec<Generation>,
}

/// Exact generation equality, without conflating equal numbers in different namespaces.
pub open spec fn same(a: Generation, b: Generation) -> bool {
    a.domain@ == b.domain@ && a.number == b.number
}

/// Membership in an unfiltered list of generation observations.
pub open spec fn member(gs: Seq<Generation>, g: Generation) -> bool {
    exists|i: int| 0 <= i < gs.len() && same(#[trigger] gs[i], g)
}

/// A running reference has logical generation evidence only if the committed record exists.
pub open spec fn known(rows: Seq<StoredGeneration>, g: Generation) -> bool {
    exists|i: int| 0 <= i < rows.len() && same(#[trigger] rows[i].generation, g)
}

/// Check membership before classifying stale running references or accepting current selections.
pub fn committed(rows: &[StoredGeneration], g: &Generation) -> (yes: bool)
    ensures yes == known(rows@, *g),
{
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(),
            forall|j: int| 0 <= j < i ==> !same(#[trigger] rows[j].generation, *g),
        decreases rows.len() - i,
    {
        if rows[i].generation.domain == g.domain && rows[i].generation.number == g.number { return true; }
        i += 1;
    }
    false
}

/// Existential set interpretation is independent of traversal or output order.
pub open spec fn exists_in<A>(s: Seq<A>, p: spec_fn(A) -> bool) -> bool {
    exists|i: int| 0 <= i < s.len() && p(#[trigger] s[i])
}

/// The aggregate protects a candidate if any retained reference matches it.
pub open spec fn any(ps: Seq<Protection>, c: Candidate) -> bool {
    exists_in(ps, |p: Protection| protects(p, c))
}

pub(super) proof fn advance<A>(s: Seq<A>, n: int, p: spec_fn(A) -> bool)
    requires 0 <= n < s.len(),
    ensures exists_in(s.take(n + 1), p) == (exists_in(s.take(n), p) || p(s[n])),
{
    if exists_in(s.take(n + 1), p) {
        let i = choose|i: int| 0 <= i < n + 1 && p(s[i]);
        if i < n { assert(p(s.take(n)[i])); } else { assert(i == n); }
    }
    if exists_in(s.take(n), p) {
        let i = choose|i: int| 0 <= i < n && p(s[i]);
        assert(p(s.take(n + 1)[i]));
    }
    if p(s[n]) { assert(p(s.take(n + 1)[n])); }
}

/// One guest-named manifest pin's exact runtime-namespace meaning.
pub open spec fn guest_pin(g: GuestPin, c: Candidate) -> bool {
    g.guest@ + "-runtime-store"@ == c.generation.domain@ && g.number == c.generation.number
}

/// Explicit pins name guests; the interpreter derives their exact pool namespaces.
pub open spec fn pin_required(pins: Seq<GuestPin>, c: Candidate) -> bool {
    exists_in(pins, |g: GuestPin| guest_pin(g, c))
}

/// Whether the manifest fixes any numeric generation for this effective guest.
pub open spec fn named_pin(pins: Seq<GuestPin>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < pins.len() && #[trigger] pins[i].guest@ == name
}

fn pinned(pins: &[GuestPin], name: &str) -> (yes: bool)
    ensures yes == named_pin(pins@, name@),
{
    let mut i = 0;
    while i < pins.len()
        invariant 0 <= i <= pins.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] pins[j].guest@ != name@,
        decreases pins.len() - i,
    {
        if pins[i].guest.as_str() == name { return true; }
        i += 1;
    }
    false
}

/// One unpinned guest definition protects its own effective system source.
pub open spec fn system_required(s: EffectiveSystem, pins: Seq<GuestPin>, c: Candidate) -> bool {
    !named_pin(pins, s.domain@) && s.domain@ + "-runtime-store"@ == c.generation.domain@ && s.system@ == c.system@
}

/// Unpinned effective definitions conservatively protect every matching software generation.
pub open spec fn systems_required(systems: Seq<EffectiveSystem>, pins: Seq<GuestPin>, c: Candidate) -> bool {
    exists_in(systems, |s: EffectiveSystem| system_required(s, pins, c))
}

/// Independent release semantics, including completed versus interrupted pruning.
pub open spec fn release_required(r: ReleaseSource, c: Candidate) -> bool {
    r.state is Present && (pin_required(r.pins@, c) || systems_required(r.systems@, r.pins@, c))
}

/// Only concrete original journal selections add numeric pins.
pub open spec fn entry_required(e: RecordedResolution, c: Candidate) -> bool {
    match e {
        RecordedResolution::Retained(s) => s.domain@ + "-runtime-store"@ == c.generation.domain@
            && s.generation == c.generation.number,
        RecordedResolution::NonNative(_) | RecordedResolution::Existing(_) => false,
    }
}

/// Any original coordinate may contribute a concrete pin.
pub open spec fn entries_required(entries: Seq<RecordedResolution>, c: Candidate) -> bool {
    exists_in(entries, |e: RecordedResolution| entry_required(e, c))
}

/// Journal endpoint requirements survive until completed pruning.
pub open spec fn journal_required(j: JournalSource, c: Candidate) -> bool {
    j.state is Present && entries_required(j.entries@, c)
}

/// Every unpruned release group participates in the union.
pub open spec fn releases_required(rs: Seq<ReleaseSource>, c: Candidate) -> bool {
    exists_in(rs, |r: ReleaseSource| release_required(r, c))
}

/// Every unpruned original journal endpoint participates in the union.
pub open spec fn journals_required(js: Seq<JournalSource>, c: Candidate) -> bool {
    exists_in(js, |j: JournalSource| journal_required(j, c))
}

/// Complete machine requirements are the union of all release and journal endpoints.
pub open spec fn machine_required(rs: Seq<ReleaseSource>, js: Seq<JournalSource>, c: Candidate) -> bool {
    releases_required(rs, c) || journals_required(js, c)
}

/// Selected and policy-recent references must resolve; stale running references remain explicit.
pub open spec fn observations_required(gs: Seq<Generation>, rows: Seq<StoredGeneration>, existing_only: bool, c: Candidate) -> bool {
    exists_in(gs, |g: Generation| observation_required(g, rows, existing_only, c))
}

/// One observation protects its exact identity, subject only to the stale-running policy.
pub open spec fn observation_required(g: Generation, rows: Seq<StoredGeneration>, existing_only: bool, c: Candidate) -> bool {
    same(g, c.generation) && (!existing_only || known(rows, g))
}

/// Every committed manifest retaining its pin contributes its exact identity.
pub open spec fn retained_required(rows: Seq<StoredGeneration>, c: Candidate) -> bool {
    exists_in(rows, |r: StoredGeneration| r.retained && same(r.generation, c.generation))
}

/// Logical pool protection; stale running references retain separate physical pins at the adapter.
pub open spec fn pool_required(p: PoolSources, c: Candidate) -> bool {
    observations_required(p.selected@, p.committed@, false, c)
        || observations_required(p.recent@, p.committed@, false, c)
        || observations_required(p.running@, p.committed@, true, c)
        || retained_required(p.committed@, c)
}

fn known_list(rows: &[StoredGeneration], gs: &[Generation]) -> (yes: bool)
    ensures yes == (forall|i: int| 0 <= i < gs.len() ==> known(rows@, #[trigger] gs[i])),
{
    let mut i = 0;
    while i < gs.len()
        invariant 0 <= i <= gs.len(),
            forall|j: int| 0 <= j < i ==> known(rows@, #[trigger] gs[j]),
        decreases gs.len() - i,
    {
        if !committed(rows, &gs[i]) { return false; }
        i += 1;
    }
    true
}

fn observation_references(gs: &[Generation], rows: &[StoredGeneration], existing_only: bool) -> (out: Vec<Protection>)
    ensures forall|c: Candidate| #[trigger] any(out@, c) == observations_required(gs@, rows@, existing_only, c),
{
    let mut out = Vec::new();
    let mut i = 0;
    while i < gs.len()
        invariant 0 <= i <= gs.len(),
            forall|c: Candidate| #[trigger] any(out@, c) == observations_required(gs@.take(i as int), rows@, existing_only, c),
        decreases gs.len() - i,
    {
        if !existing_only || committed(rows, &gs[i]) {
            add(&mut out, Protection::Exact(Generation { domain: gs[i].domain.as_str().to_string(), number: gs[i].number }));
        }
        proof {
            assert forall|c: Candidate| #[trigger] any(out@, c)
                == observations_required(gs@.take(i as int + 1), rows@, existing_only, c)
            by { advance(gs@, i as int, |g: Generation| observation_required(g, rows@, existing_only, c)); }
        }
        i += 1;
    }
    out
}

fn retained_references(rows: &[StoredGeneration]) -> (out: Vec<Protection>)
    ensures forall|c: Candidate| #[trigger] any(out@, c) == retained_required(rows@, c),
{
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(),
            forall|c: Candidate| #[trigger] any(out@, c) == retained_required(rows@.take(i as int), c),
        decreases rows.len() - i,
    {
        if rows[i].retained {
            add(&mut out, Protection::Exact(Generation { domain: rows[i].generation.domain.as_str().to_string(), number: rows[i].generation.number }));
        }
        proof {
            assert forall|c: Candidate| #[trigger] any(out@, c) == retained_required(rows@.take(i as int + 1), c)
            by { advance(rows@, i as int, |r: StoredGeneration| r.retained && same(r.generation, c.generation)); }
        }
        i += 1;
    }
    out
}

/// Concatenation preserves both complete sets, regardless of duplication or incidental order.
proof fn union(a: Seq<Protection>, b: Seq<Protection>)
    ensures forall|c: Candidate| #[trigger] any(a + b, c) == (any(a, c) || any(b, c)),
{
    assert forall|c: Candidate| #[trigger] any(a + b, c) == (any(a, c) || any(b, c)) by {
        if any(a + b, c) {
            let i = choose|i: int| 0 <= i < (a + b).len() && protects((a + b)[i], c);
            if i < a.len() { assert(protects(a[i], c)); }
            else { assert(protects(b[i - a.len()], c)); }
        }
        if any(a, c) {
            let i = choose|i: int| 0 <= i < a.len() && protects(a[i], c);
            assert(protects((a + b)[i], c));
        }
        if any(b, c) {
            let i = choose|i: int| 0 <= i < b.len() && protects(b[i], c);
            assert(protects((a + b)[a.len() + i], c));
        }
    }
}

fn add(out: &mut Vec<Protection>, p: Protection)
    ensures final(out)@ == old(out)@.push(p),
        forall|c: Candidate| #[trigger] any(final(out)@, c) == (any(old(out)@, c) || protects(p, c)),
{
    let ghost before = out@;
    out.push(p);
    proof {
        union(before, seq![p]); assert(out@ == before + seq![p]);
        assert forall|c: Candidate| #[trigger] any(seq![p], c) == protects(p, c) by {
            if protects(p, c) { assert(protects(seq![p][0], c)); }
            if any(seq![p], c) {
                let i = choose|i: int| 0 <= i < 1 && protects(seq![p][i], c);
                assert(i == 0);
            }
        }
    }
}

fn release_references(r: &ReleaseSource) -> (out: Vec<Protection>)
    ensures forall|c: Candidate| #[trigger] any(out@, c) == release_required(*r, c),
{
    let mut out = Vec::new();
    if matches!(r.state, ReleaseState::Pruned) { return out; }

    let mut i = 0;
    while i < r.pins.len()
        invariant 0 <= i <= r.pins.len(), r.state is Present,
            forall|c: Candidate| #[trigger] any(out@, c) == pin_required(r.pins@.take(i as int), c),
        decreases r.pins.len() - i,
    {
        add(&mut out, Protection::Exact(Generation {
            domain: runtime_namespace(r.pins[i].guest.as_str()), number: r.pins[i].number,
        }));
        proof { assert forall|c: Candidate| #[trigger] any(out@, c) == pin_required(r.pins@.take(i as int + 1), c)
            by { advance(r.pins@, i as int, |g: GuestPin| guest_pin(g, c)); } }
        i += 1;
    }
    let mut i = 0;
    while i < r.systems.len()
        invariant 0 <= i <= r.systems.len(), r.state is Present,
            forall|c: Candidate| #[trigger] any(out@, c) == (pin_required(r.pins@, c)
                || systems_required(r.systems@.take(i as int), r.pins@, c)),
        decreases r.systems.len() - i,
    {
        if !pinned(&r.pins, r.systems[i].domain.as_str()) {
            add(&mut out, Protection::System { domain: runtime_namespace(r.systems[i].domain.as_str()),
                system: r.systems[i].system.as_str().to_string() });
        }
        proof { assert forall|c: Candidate| #[trigger] any(out@, c) == (pin_required(r.pins@, c)
            || systems_required(r.systems@.take(i as int + 1), r.pins@, c))
            by { advance(r.systems@, i as int, |x: EffectiveSystem| system_required(x, r.pins@, c)); } }
        i += 1;
    }
    out
}

fn journal_references(j: &JournalSource) -> (out: Vec<Protection>)
    ensures forall|c: Candidate| #[trigger] any(out@, c) == journal_required(*j, c),
{
    let mut out = Vec::new();
    if matches!(j.state, ReleaseState::Pruned) { return out; }

    let mut i = 0;
    while i < j.entries.len()
        invariant 0 <= i <= j.entries.len(), j.state is Present,
            forall|c: Candidate| #[trigger] any(out@, c) == entries_required(j.entries@.take(i as int), c),
        decreases j.entries.len() - i,
    {
        if let RecordedResolution::Retained(s) = &j.entries[i] {
            add(&mut out, Protection::Exact(Generation { domain: runtime_namespace(s.domain.as_str()), number: s.generation }));
        }
        proof {
            assert forall|c: Candidate| #[trigger] any(out@, c) == entries_required(j.entries@.take(i as int + 1), c)
            by { advance(j.entries@, i as int, |e: RecordedResolution| entry_required(e, c)); }
        }
        i += 1;
    }
    out
}

/// Two references denote the same exact coordinate or the same effective-system requirement.
pub open spec fn identical(a: Protection, b: Protection) -> bool {
    match (a, b) {
        (Protection::Exact(x), Protection::Exact(y)) => same(x, y),
        (Protection::System { domain: d, system: s }, Protection::System { domain: e, system: t }) => d@ == e@ && s@ == t@,
        (Protection::Exact(_), Protection::System { .. })
        | (Protection::System { .. }, Protection::Exact(_)) => false,
    }
}

fn contains_reference(references: &[Protection], p: &Protection) -> (yes: bool)
    ensures yes == (exists|i: int| 0 <= i < references.len() && identical(#[trigger] references[i], *p)),
{
    let mut i = 0;
    while i < references.len()
        invariant 0 <= i <= references.len(),
            forall|j: int| 0 <= j < i ==> !identical(#[trigger] references[j], *p),
        decreases references.len() - i,
    {
        let yes = match (&references[i], p) {
            (Protection::Exact(a), Protection::Exact(b)) => a.domain == b.domain && a.number == b.number,
            (Protection::System { domain: a, system: s }, Protection::System { domain: b, system: t }) => *a == *b && *s == *t,
            (Protection::Exact(_), Protection::System { .. })
            | (Protection::System { .. }, Protection::Exact(_)) => false,
        };
        if yes { return true; }
        i += 1;
    }
    false
}

/// Deduplication cannot lose a distinct requirement or create a new one.
// Keep the complete pre-pop remainder available at the loop's invariant boundary.
#[allow(clippy::manual_while_let_some)]
fn deduplicate(mut pending: Vec<Protection>) -> (out: Vec<Protection>)
    ensures forall|c: Candidate| #[trigger] any(out@, c) == any(pending@, c),
        forall|i: int, j: int| 0 <= i < j < out.len() ==> !identical(#[trigger] out[i], #[trigger] out[j]),
{
    let ghost original = pending@;
    let mut out = Vec::new();
    while !pending.is_empty()
        invariant forall|c: Candidate| #[trigger] any(out@, c) || any(pending@, c) <==> any(original, c),
            forall|i: int, j: int| 0 <= i < j < out.len() ==> !identical(#[trigger] out[i], #[trigger] out[j]),
        decreases pending.len(),
    {
        let ghost before = pending@;
        let p = pending.pop().unwrap();
        proof {
            union(pending@, seq![p]);
            assert(before == pending@ + seq![p]);
            assert forall|c: Candidate| #[trigger] any(seq![p], c) == protects(p, c) by {
                if protects(p, c) { assert(protects(seq![p][0], c)); }
                if any(seq![p], c) {
                    let i = choose|i: int| 0 <= i < 1 && protects(seq![p][i], c);
                    assert(i == 0);
                }
            }
        }
        if contains_reference(&out, &p) {
            proof {
                let i = choose|i: int| 0 <= i < out.len() && identical(out[i], p);
                assert forall|c: Candidate| protects(p, c) ==> any(out@, c) by {
                    assert(protects(p, c) == protects(out[i], c));
                }
            }
        } else { add(&mut out, p); }
    }
    out
}

/// A complete interpretation of the supplied source groups, with source meaning retained in proof evidence.
/// Authentication, enumeration and decoding precede this boundary; recent-generation ranking is policy input.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::{ProtectedGenerations, inventory::ProtectionInventory};
/// /// An adapter cannot drop protections from an already interpreted inventory.
/// fn discard(inventory: &mut ProtectionInventory) {
///     inventory.protected = ProtectedGenerations::new(Vec::new()).unwrap();
/// }
/// ```
pub struct ProtectionInventory {
    // References validated by the existing reclamation boundary.
    protected: ProtectedGenerations,
    // Erased original source requirements, independent of the output list.
    _required: Ghost<spec_fn(Candidate) -> bool>,
}

impl View for ProtectionInventory {
    type V = Seq<Protection>;

    closed spec fn view(&self) -> Self::V { self.protected@ }
}

impl ProtectionInventory {
    /// Original decoded-source meaning, retained across aggregation and request construction.
    pub closed spec fn required(&self, c: Candidate) -> bool { self._required@(c) }

    #[verifier::type_invariant]
    closed spec fn complete(&self) -> bool {
        forall|c: Candidate| #[trigger] any(self@, c) == self._required@(c)
    }

    fn checked(references: Vec<Protection>, required: Ghost<spec_fn(Candidate) -> bool>) -> (out: Option<Self>)
        requires forall|c: Candidate| #[trigger] any(references@, c) == required@(c),
        ensures out.is_some() ==> forall|c: Candidate| out.unwrap().required(c) == required@(c),
    {
        let references = deduplicate(references);
        let protected = ProtectedGenerations::new(references);
        protected.map(|protected: ProtectedGenerations| -> (out: Self)
            requires forall|c: Candidate| #[trigger] any(protected@, c) == required@(c),
            ensures out@ == protected@ && forall|c: Candidate| out.required(c) == required@(c),
        { Self { protected, _required: required } })
    }

    /// Interpret every decoded release and journal source; no caller supplies the projected set.
    pub fn machine(releases: &[ReleaseSource], journals: &[JournalSource]) -> (out: Option<Self>)
        ensures out.is_some() ==> forall|c: Candidate|
            out.unwrap().required(c) == machine_required(releases@, journals@, c),
    {
        let mut references: Vec<Protection> = Vec::new();
        let mut i = 0;
        while i < releases.len()
            invariant 0 <= i <= releases.len(),
                forall|c: Candidate| #[trigger] any(references@, c) == releases_required(releases@.take(i as int), c),
            decreases releases.len() - i,
        {
            let mut extra = release_references(&releases[i]);
            let ghost a = references@;
            let ghost b = extra@;
            references.append(&mut extra);
            proof { union(a, b); }
            proof { assert forall|c: Candidate| #[trigger] any(references@, c) == releases_required(releases@.take(i as int + 1), c)
                by { advance(releases@, i as int, |r: ReleaseSource| release_required(r, c)); } }
            i += 1;
        }
        let mut i = 0;
        while i < journals.len()
            invariant 0 <= i <= journals.len(),
                forall|c: Candidate| #[trigger] any(references@, c) == (releases_required(releases@, c)
                    || journals_required(journals@.take(i as int), c)),
            decreases journals.len() - i,
        {
            let mut extra = journal_references(&journals[i]);
            let ghost a = references@;
            let ghost b = extra@;
            references.append(&mut extra);
            proof { union(a, b); }
            proof { assert forall|c: Candidate| #[trigger] any(references@, c) == (releases_required(releases@, c)
                || journals_required(journals@.take(i as int + 1), c))
                by { advance(journals@, i as int, |j: JournalSource| journal_required(j, c)); } }
            i += 1;
        }
        let ghost rs = releases@;
        let ghost js = journals@;
        Self::checked(references, Ghost(|c: Candidate| machine_required(rs, js, c)))
    }

    /// Combine complete pool observations without filtering selected, running or retained inputs at the caller.
    pub fn pool(p: &PoolSources) -> (out: Option<Self>)
        ensures out.is_some() ==> (forall|c: Candidate| out.unwrap().required(c) == pool_required(*p, c))
            && (forall|i: int| 0 <= i < p.selected.len() ==> known(p.committed@, #[trigger] p.selected[i]))
            && (forall|i: int| 0 <= i < p.recent.len() ==> known(p.committed@, #[trigger] p.recent[i])),
    {
        if !known_list(&p.committed, &p.selected) || !known_list(&p.committed, &p.recent) { return None; }

        let mut references = observation_references(&p.selected, &p.committed, false);
        let mut recent = observation_references(&p.recent, &p.committed, false);
        let mut running = observation_references(&p.running, &p.committed, true);
        let mut retained = retained_references(&p.committed);
        let ghost a = references@;
        let ghost b = recent@;
        references.append(&mut recent);
        proof { union(a, b); }
        let ghost a = references@;
        let ghost b = running@;
        references.append(&mut running);
        proof { union(a, b); }
        let ghost a = references@;
        let ghost b = retained@;
        references.append(&mut retained);
        proof { union(a, b); }
        let ghost sources = *p;
        proof { assert forall|c: Candidate| #[trigger] any(references@, c) == pool_required(*p, c) by {} }
        Self::checked(references, Ghost(|c: Candidate| pool_required(sources, c)))
    }

    /// Preserve the checked pool inventory and every transported machine reference in their union.
    pub fn with_references(self, mut references: Vec<Protection>) -> (out: Option<Self>)
        ensures out.is_some() ==> forall|c: Candidate|
            out.unwrap().required(c) == (self.required(c) || any(references@, c)),
    {
        proof { use_type_invariant(&self); }
        let ghost original = references@;
        let ghost previous = self@;
        let ghost required = self._required@;
        proof { assert forall|c: Candidate| #[trigger] any(previous, c) == required(c) by {} }
        let mut combined = self.protected.into_references();
        let ghost a = combined@;
        combined.append(&mut references);
        proof { union(a, original); }
        proof {
            assert(a == previous);
            assert forall|c: Candidate| #[trigger] any(combined@, c) == (required(c) || any(original, c)) by {
                assert(any(previous, c) == required(c));
                assert(any(combined@, c) == (any(a, c) || any(original, c)));
            }
        }
        Self::checked(combined, Ghost(|c: Candidate| required(c) || any(original, c)))
    }

    /// Read the complete interpreted set for transport, without exposing mutable storage.
    pub fn references(&self) -> (out: &[Protection]) ensures out@ == self@,
    { self.protected.references() }

    /// Retention changes consume the interpreted set and preserve the original source guarantee.
    pub fn release<'a>(&'a self, candidate: Candidate, pin: Pin) -> (out: Option<ReleaseRequest<'a>>)
        ensures out.is_some() ==> !self.required(candidate) && out.unwrap()@ == candidate
            && out.unwrap().reference_set() == self@,
    {
        proof { use_type_invariant(self); }
        let out = self.protected.release(candidate, pin);
        if out.is_some() {
            proof { assert(!any(self@, candidate)); assert(!self.required(candidate)); }
        }
        out
    }

    /// Reclamation cannot accept a candidate required by any original source.
    pub fn reclaim(&self, candidate: Candidate) -> (out: Option<ReclaimRequest>)
        ensures out.is_some() ==> !self.required(candidate) && out.unwrap()@ == candidate
            && out.unwrap().reference_set() == self@,
    {
        proof { use_type_invariant(self); }
        let out = self.protected.reclaim(candidate);
        if out.is_some() {
            proof { assert(!any(self@, candidate)); assert(!self.required(candidate)); }
        }
        out
    }
}

}
