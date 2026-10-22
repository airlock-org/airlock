//! Select exact unpinned, sealed segments for deletion within one locked pool snapshot.
//! The verified index is derived from the full trace, not from a caller-provided protection flag.

#[cfg(verus_keep_ghost)]
use super::inventory::advance;

use super::tracing::{Resource, Trace};
use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// Physical segment lifecycle, independent of logical generation retention.
#[derive(Clone, Copy)]
pub enum State {
    /// Appendable or incompletely sealed storage must survive cleanup.
    Open,
    /// Published immutable segment; references and current selection still constrain deletion.
    Sealed,
}

impl State {
    /// Reject unknown or contradictory persisted lifecycle fields.
    pub fn decode(status: &str, sealed: bool) -> (out: Option<Self>)
        ensures out.is_some() == ((status@ == "open"@ && !sealed) || (status@ == "sealed"@ && sealed)),
            out.is_some() ==> (out.unwrap() is Sealed <==> status@ == "sealed"@ && sealed),
    {
        if status == "open" && !sealed { Some(Self::Open) }
        else if status == "sealed" && sealed { Some(Self::Sealed) }
        else { None }
    }
}

/// Observed segment identity and lifecycle, decoded from the same physical record.
pub struct Candidate {
    /// Exact filename identity, preserving its original spelling.
    pub id: String,
    /// Consistent decoded lifecycle; an unclassified status cannot construct a candidate.
    pub state: State,
}

/// Any traced physical range protects the whole segment containing it.
pub open spec fn required(rs: Seq<Resource>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < rs.len() && match #[trigger] rs[i] {
        Resource::Segment(id) => id@ == name,
        Resource::Object(_) | Resource::View(_) | Resource::Root(_) => false,
    }
}

/// Indexed names represent exact identities, independently of order or duplicates.
pub open spec fn named(names: Seq<String>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < names.len() && #[trigger] names[i]@ == name
}

/// Only a known absent selection or a different, nonempty selected identity is compatible.
pub open spec fn not_current(c: Candidate, current: Option<String>) -> bool {
    match current { None => true, Some(id) => id@.len() > 0 && id@ != c.id@ }
}

/// Reference absence alone never authorizes deletion of an open or selected segment.
pub open spec fn deletable(rs: Seq<Resource>, c: Candidate, current: Option<String>) -> bool {
    c.id@.len() > 0 && c.state is Sealed && not_current(c, current) && !required(rs, c.id@)
}

fn add_name(names: &mut Vec<String>, name: &str)
    ensures forall|n: Seq<char>| #[trigger] named(final(names)@, n) == (named(old(names)@, n) || name@ == n),
{
    // Physical tracing visits each segment's ranges together. Compact adjacent
    // repetitions without assuming that all callers supply sorted references.
    if !names.is_empty() && names[names.len() - 1].as_str() == name { return; }
    let ghost before = names@;
    names.push(name.to_string());
    proof {
        assert forall|n: Seq<char>| #[trigger] named(names@, n) == (named(before, n) || name@ == n) by {
            if named(names@, n) {
                let j = choose|j: int| 0 <= j < names.len() && names[j]@ == n;
                if j < before.len() { assert(before[j]@ == n); }
                else { assert(j == before.len()); }
            }
            if named(before, n) {
                let j = choose|j: int| 0 <= j < before.len() && before[j]@ == n;
                assert(names[j]@ == n);
            }
            if name@ == n { assert(names[before.len() as int]@ == n); }
        }
    }
}

/// Immutable exact segment index; borrowing its trace prevents later extensions during selection.
pub struct SegmentIndex<'a> {
    // Pool coordinate accompanies every permission; adapters cannot resupply a different pool.
    pool: String,
    // Complete source trace for this collection pass, retained without rebuilding authority.
    _trace: &'a Trace,
    // Exact segment names with adjacent repetitions compacted for efficient deletion checks.
    names: Vec<String>,
}

impl<'a> View for SegmentIndex<'a> {
    type V = Seq<Resource>;

    closed spec fn view(&self) -> Self::V { self._trace@ }
}

impl<'a> SegmentIndex<'a> {
    /// Original pool identity whose physical records supplied the candidates.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.pool@.len() > 0 && forall|n: Seq<char>| #[trigger] named(self.names@, n) == required(self._trace@, n)
    }

    /// Interpret every traced segment reference without losing nonadjacent repetitions.
    pub fn new(pool: String, trace: &'a Trace) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == trace@ && out.unwrap().pool_name() == pool@,
    {
        if pool.is_empty() { return None; }

        let references = trace.references();
        let mut names = Vec::new();
        let mut i = 0;
        while i < references.len()
            invariant 0 <= i <= references.len(), pool@.len() > 0, references@ == trace@,
                forall|n: Seq<char>| #[trigger] named(names@, n) == required(references@.take(i as int), n),
            decreases references.len() - i,
        {
            match &references[i] {
                Resource::Segment(id) => {
                    if id.is_empty() { return None; }
                    add_name(&mut names, id.as_str());
                },
                Resource::Object(_) | Resource::View(_) | Resource::Root(_) => {},
            }
            proof {
                assert forall|n: Seq<char>| #[trigger] named(names@, n) == required(references@.take(i as int + 1), n) by {
                    advance(references@, i as int, |r: Resource| match r {
                        Resource::Segment(id) => id@ == n,
                        Resource::Object(_) | Resource::View(_) | Resource::Root(_) => false,
                    });
                }
            }
            i += 1;
        }
        Some(Self { pool, _trace: trace, names })
    }

    /// Permit exactly a nonempty, sealed, noncurrent target absent from the original trace.
    pub fn reclaim(&self, candidate: Candidate, current: &Option<String>) -> (out: Option<SegmentDeleteRequest<'a>>)
        ensures out.is_some() == deletable(self@, candidate, *current),
            out.is_some() ==> out.unwrap()@ == candidate && out.unwrap().pool_name() == self.pool_name()
                && out.unwrap().reference_set() == self@ && out.unwrap().selection() == *current,
    {
        proof { use_type_invariant(self); }
        if candidate.id.is_empty() || !matches!(candidate.state, State::Sealed) { return None; }
        let selection_ok = match current { None => true, Some(id) => !id.is_empty() && *id != candidate.id };
        if !selection_ok { return None; }

        let mut i = 0;
        while i < self.names.len()
            invariant 0 <= i <= self.names.len(), self.valid(),
                candidate.id@.len() > 0, candidate.state is Sealed, not_current(candidate, *current),
                forall|j: int| 0 <= j < i ==> #[trigger] self.names[j]@ != candidate.id@,
            decreases self.names.len() - i,
        {
            if self.names[i] == candidate.id {
                proof { assert(named(self.names@, candidate.id@)); }
                return None;
            }
            i += 1;
        }
        proof { assert(!named(self.names@, candidate.id@)); }

        Some(SegmentDeleteRequest {
            pool: self.pool.as_str().to_string(), candidate,
            _trace: self._trace, _current: Ghost(*current),
        })
    }
}

/// Opaque, consumed permission for one exact segment in one exact pool.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::segments::SegmentDeleteRequest;
/// /// An adapter cannot substitute a different physical target.
/// fn substitute(request: &mut SegmentDeleteRequest<'_>) { request.candidate.id = "000999".into(); }
/// ```
///
/// ```compile_fail,E0502
/// use airlock_deployment_core::retention::{segments::{Candidate, SegmentIndex, State}, tracing::Trace};
/// /// A permission cannot survive adding new pins to its original trace.
/// fn add_pins(trace: &mut Trace) {
///     let index = SegmentIndex::new("/pool".into(), trace).unwrap();
///     let request = index.reclaim(Candidate { id: "000001".into(), state: State::Sealed }, &None).unwrap();
///     trace.include_extents(&[]);
///     println!("{id}", id = request.id());
/// }
/// ```
///
/// ```compile_fail,E0382
/// use airlock_deployment_core::retention::segments::SegmentDeleteRequest;
/// /// A consumed permission cannot be dispatched a second time.
/// fn replay(request: SegmentDeleteRequest<'_>) {
///     let consumed = request;
///     let replayed = request;
///     drop((consumed, replayed));
/// }
/// ```
pub struct SegmentDeleteRequest<'a> {
    // Original pool, forwarded to the filesystem adapter without an independent root argument.
    pool: String,
    // Original observed sealed target, never reconstructed from a diagnostic report.
    candidate: Candidate,
    // Borrow the original complete trace; it cannot be extended while this permission survives.
    _trace: &'a Trace,
    // Erased original selection observation; absent is distinct from an invalid empty selection.
    _current: Ghost<Option<String>>,
}

impl<'a> View for SegmentDeleteRequest<'a> {
    type V = Candidate;

    closed spec fn view(&self) -> Self::V { self.candidate }
}

impl<'a> SegmentDeleteRequest<'a> {
    /// Original complete traced reference set used for the permission.
    pub closed spec fn reference_set(&self) -> Seq<Resource> { self._trace@ }

    /// Original pool coordinate, distinct from the segment filename within it.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    /// Current-selection observation used at construction.
    pub closed spec fn selection(&self) -> Option<String> { self._current@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.pool@.len() > 0 && deletable(self._trace@, self.candidate, self._current@)
    }

    /// Forward exactly the original checked pool identity.
    pub fn pool(&self) -> (out: &str) ensures out@ == self.pool_name(),
    { self.pool.as_str() }

    /// Forward exactly the original checked segment identity.
    pub fn id(&self) -> (out: &str) ensures out@ == self@.id@,
    { self.candidate.id.as_str() }

    /// Refuse changed identity, reopened storage or a newly selected target before unlinking.
    pub fn revalidate(&self, candidate: &Candidate, current: &Option<String>) -> (ok: bool)
        ensures ok == (candidate.id@ == self@.id@ && candidate.state is Sealed && not_current(*candidate, *current)),
            ok ==> deletable(self.reference_set(), *candidate, *current),
    {
        proof { use_type_invariant(self); }
        if candidate.id != self.candidate.id || !matches!(candidate.state, State::Sealed) { return false; }
        match current { None => true, Some(id) => !id.is_empty() && *id != candidate.id }
    }
}

}
