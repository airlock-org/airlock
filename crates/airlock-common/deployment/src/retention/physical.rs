//! Resolve every original logical requirement before granting physical reclamation authority.
//! Decoders supply named extents; complete source discovery and truthful backing records remain adapter duties.

#[cfg(verus_keep_ghost)]
use super::inventory::{advance, exists_in};

use super::{
    runtime::{Observation, Phase, Presence},
    segments::SegmentIndex,
    tracing::{Extent, Resource, Trace},
};
use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// One resolution at the same position as its original logical requirement.
pub struct Resolution {
    /// Full original resource category and identity, checked against the immutable source trace.
    pub resource: Resource,
    /// One canonical object range, every retained view range, or no ranges for a separate root artifact.
    pub extents: Vec<Extent>,
}

/// Every discovered consumer is supplied before the verified classifier decides whether to retain its pins.
pub enum Consumer {
    /// A running VM pins its recorded locations even without a surviving logical generation.
    Running {
        /// Canonical running-reference identity validated by the filesystem adapter.
        identity: String,
        /// Complete original physical pin collection.
        extents: Vec<Extent>,
    },
    /// A kernel mapping may release pins only on exact-name affirmative absence outside preparation.
    Runtime {
        /// Mapping identity from the validated persisted record.
        identity: String,
        /// Original persisted lifecycle.
        phase: Phase,
        /// Evidence tied to the exact queried name, including uncertain outcomes.
        observation: Observation,
        /// Complete original physical pin collection.
        extents: Vec<Extent>,
    },
}

/// Complete decoded resolutions and unfiltered consumer observations for one locked pool snapshot.
pub struct Inputs {
    /// Exactly one row per original trace entry, preserving its order and duplicates.
    pub resolutions: Vec<Resolution>,
    /// Every discovered running reference and runtime record, before absence classification.
    pub consumers: Vec<Consumer>,
}

/// Root artifacts occupy a separate namespace; object and view references require physical backing.
pub open spec fn shape(row: Resolution) -> bool {
    match row.resource {
        Resource::Object(n) => n@.len() > 0 && row.extents.len() == 1,
        Resource::View(n) => n@.len() > 0 && row.extents.len() > 0,
        Resource::Root(n) => n@.len() > 0 && row.extents.len() == 0,
        // This interpreter takes logical sources; already-physical inputs must not be silently dropped.
        Resource::Segment(_) => false,
    }
}

/// Consumer identity is independent of its pin-retention decision.
pub open spec fn named(c: Consumer) -> bool {
    match c {
        Consumer::Running { identity, .. } => identity@.len() > 0,
        Consumer::Runtime { identity, observation, .. } => identity@.len() > 0 && identity@ == observation.identity(),
    }
}

/// Unknown outcomes and interrupted preparations retain all original pins.
pub open spec fn retained(c: Consumer) -> bool {
    match c {
        Consumer::Running { .. } => true,
        Consumer::Runtime { phase, observation, .. } => !(observation.result() is Absent) || phase is Preparing,
    }
}

/// Physical pins carried by a consumer, without JSON projection or filtering.
pub open spec fn pins(c: Consumer) -> Seq<Extent> {
    match c { Consumer::Running { extents, .. } | Consumer::Runtime { extents, .. } => extents@ }
}

/// Every required logical identity must be resolved; every retained consumer has a valid nonempty pin set.
pub open spec fn valid(logical: Seq<Resource>, input: Inputs) -> bool {
    input.resolutions.len() == logical.len()
        && (forall|i: int| 0 <= i < logical.len() ==> super::tracing::same(logical[i], #[trigger] input.resolutions[i].resource)
            && shape(input.resolutions[i]) && super::tracing::valid_extents(input.resolutions[i].extents@))
        && (forall|i: int| 0 <= i < input.consumers.len() ==> named(#[trigger] input.consumers[i])
            && (retained(input.consumers[i]) ==> pins(input.consumers[i]).len() > 0 && super::tracing::valid_extents(pins(input.consumers[i]))))
}

/// Independent union: every resolved range and every retained consumer protects its entire segment.
pub open spec fn required(input: Inputs, r: Resource) -> bool {
    exists_in(input.resolutions@, |row: Resolution| super::tracing::extent_requires(row.extents@, r))
        || exists_in(input.consumers@, |c: Consumer| retained(c) && super::tracing::extent_requires(pins(c), r))
}

fn matches(row: &Resolution, original: &Resource) -> (yes: bool)
    ensures yes == (super::tracing::same(row.resource, *original) && shape(*row)),
{
    match (&row.resource, original) {
        (Resource::Object(a), Resource::Object(b)) => !a.is_empty() && *a == *b && row.extents.len() == 1,
        (Resource::View(a), Resource::View(b)) => !a.is_empty() && *a == *b && !row.extents.is_empty(),
        (Resource::Root(a), Resource::Root(b)) => !a.is_empty() && *a == *b && row.extents.is_empty(),
        _ => false,
    }
}

impl Consumer {
    /// Classify the original consumer without allowing unknown or mismatched evidence to release its pins.
    pub fn retains_pins(&self) -> (out: Option<bool>)
        ensures out.is_some() == named(*self), out.is_some() ==> out.unwrap() == retained(*self),
    {
        match self {
            Consumer::Running { identity, .. } => if identity.is_empty() { None } else { Some(true) },
            Consumer::Runtime { identity, phase, observation, .. } => {
                if identity.is_empty() || identity.as_str() != observation.name() { return None; }
                Some(!matches!(observation.presence(), Presence::Absent) || matches!(phase, Phase::Preparing))
            },
        }
    }
}

fn extents(c: &Consumer) -> (out: &[Extent]) ensures out@ == pins(*c),
{ match c { Consumer::Running { extents, .. } | Consumer::Runtime { extents, .. } => extents } }

/// Checked physical protection tied to the immutable original logical trace and exact pool.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::{physical::PhysicalProtection, tracing::Trace};
/// /// A caller cannot replace complete protection with an empty trace before deletion.
/// fn discard(protection: &mut PhysicalProtection<'_>) { protection.trace = Trace::new(); }
/// ```
///
/// ```compile_fail,E0502
/// use airlock_deployment_core::retention::{physical::{Inputs, PhysicalProtection}, tracing::Trace};
/// /// New logical requirements invalidate the snapshot, so the borrow must end before extending it.
/// fn extend(logical: &mut Trace, input: Inputs) {
///     let protection = PhysicalProtection::resolve("/pool".into(), logical, input).unwrap();
///     logical.include_object("new-requirement".into());
///     let _ = protection.index();
/// }
/// ```
pub struct PhysicalProtection<'a> {
    // Original locked pool coordinate, preserved through segment request selection.
    pool: String,
    // Borrow prevents logical source changes while physical permissions remain usable.
    _logical: &'a Trace,
    // Exact union of all resolved and retained consumer segment requirements.
    trace: Trace,
    // Erased original input, retaining the source/identity and complete-union proof witnesses.
    _input: Ghost<Inputs>,
}

impl<'a> View for PhysicalProtection<'a> {
    type V = Seq<Resource>;

    closed spec fn view(&self) -> Self::V { self.trace@ }
}

impl<'a> PhysicalProtection<'a> {
    /// Full original decoded inputs, retained as erased specification evidence.
    pub closed spec fn sources(&self) -> Inputs { self._input@ }

    /// Original pool identity used by the segment request adapter.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    #[verifier::type_invariant]
    closed spec fn invariant(&self) -> bool {
        self.pool@.len() > 0 && valid(self._logical@, self._input@)
            && forall|r: Resource| #[trigger] super::tracing::contains(self.trace@, r) == required(self._input@, r)
    }

    /// Refuse incomplete or mismatched resolutions before issuing a physical reclamation capability.
    pub fn resolve(pool: String, logical: &'a Trace, input: Inputs) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().pool_name() == pool@ && out.unwrap().sources() == input
            && valid(logical@, input)
            && forall|r: Resource| #[trigger] super::tracing::contains(out.unwrap()@, r) == required(input, r),
    {
        if pool.is_empty() { return None; }
        let original = logical.references();
        if original.len() != input.resolutions.len() { return None; }
        let mut trace = Trace::new();
        let mut i = 0;
        while i < input.resolutions.len()
            invariant 0 <= i <= input.resolutions.len(), original@ == logical@,
                original.len() == input.resolutions.len(), pool@.len() > 0,
                forall|j: int| 0 <= j < i ==> super::tracing::same(original[j], #[trigger] input.resolutions[j].resource)
                    && shape(input.resolutions[j]) && super::tracing::valid_extents(input.resolutions[j].extents@),
                forall|r: Resource| #[trigger] super::tracing::contains(trace@, r)
                    == exists_in(input.resolutions@.take(i as int), |row: Resolution| super::tracing::extent_requires(row.extents@, r)),
            decreases input.resolutions.len() - i,
        {
            let row = &input.resolutions[i];
            if !matches(row, &original[i]) || !trace.include_extents(&row.extents) { return None; }
            proof {
                assert forall|r: Resource| #[trigger] super::tracing::contains(trace@, r)
                    == exists_in(input.resolutions@.take(i as int + 1), |row: Resolution| super::tracing::extent_requires(row.extents@, r))
                by { advance(input.resolutions@, i as int, |row: Resolution| super::tracing::extent_requires(row.extents@, r)); }
            }
            i += 1;
        }
        let mut i = 0;
        while i < input.consumers.len()
            invariant 0 <= i <= input.consumers.len(), original@ == logical@,
                original.len() == input.resolutions.len(), pool@.len() > 0,
                forall|j: int| 0 <= j < original.len() ==> super::tracing::same(original[j], #[trigger] input.resolutions[j].resource)
                    && shape(input.resolutions[j]) && super::tracing::valid_extents(input.resolutions[j].extents@),
                forall|j: int| 0 <= j < i ==> named(#[trigger] input.consumers[j])
                    && (retained(input.consumers[j]) ==> pins(input.consumers[j]).len() > 0 && super::tracing::valid_extents(pins(input.consumers[j]))),
                forall|r: Resource| #[trigger] super::tracing::contains(trace@, r)
                    == (exists_in(input.resolutions@, |row: Resolution| super::tracing::extent_requires(row.extents@, r))
                        || exists_in(input.consumers@.take(i as int), |c: Consumer| retained(c) && super::tracing::extent_requires(pins(c), r))),
            decreases input.consumers.len() - i,
        {
            let c = &input.consumers[i];
            let keep = c.retains_pins()?;
            if keep {
                let pins = extents(c);
                if pins.is_empty() || !trace.include_extents(pins) { return None; }
            }
            proof {
                assert forall|r: Resource| #[trigger] super::tracing::contains(trace@, r)
                    == (exists_in(input.resolutions@, |row: Resolution| super::tracing::extent_requires(row.extents@, r))
                        || exists_in(input.consumers@.take(i as int + 1), |c: Consumer| retained(c) && super::tracing::extent_requires(pins(c), r)))
                by { advance(input.consumers@, i as int, |c: Consumer| retained(c) && super::tracing::extent_requires(pins(c), r)); }
            }
            i += 1;
        }
        Some(Self { pool, _logical: logical, trace, _input: Ghost(input) })
    }

    /// Preserve the original pool through the external filesystem adapter.
    pub fn pool(&self) -> (out: &str) ensures out@ == self.pool_name(),
    { self.pool.as_str() }

    /// Build deletion's existing exact index directly from the complete checked physical union.
    pub fn index(&self) -> (out: Option<SegmentIndex<'_>>)
        ensures out.is_some() ==> out.unwrap().pool_name() == self.pool_name()
            && (forall|r: Resource| #[trigger] super::tracing::contains(out.unwrap()@, r) == required(self.sources(), r))
            && (forall|name: String| #[trigger] super::segments::required(out.unwrap()@, name@)
                == required(self.sources(), Resource::Segment(name))),
    {
        proof {
            use_type_invariant(self);
            // Segment deletion's independent membership rule must agree with physical resolution.
            assert forall|name: String| #[trigger] super::segments::required(self.trace@, name@)
                == required(self.sources(), Resource::Segment(name)) by {
                if super::tracing::contains(self.trace@, Resource::Segment(name)) {
                    let i = choose|i: int| 0 <= i < self.trace@.len()
                        && super::tracing::same(self.trace@[i], Resource::Segment(name));
                    assert(self.trace@[i] is Segment);
                    assert(super::segments::required(self.trace@, name@));
                }
                if super::segments::required(self.trace@, name@) {
                    let i = choose|i: int| 0 <= i < self.trace@.len() && match #[trigger] self.trace@[i] {
                        Resource::Segment(id) => id@ == name@, _ => false,
                    };
                    assert(super::tracing::same(self.trace@[i], Resource::Segment(name)));
                }
            }
        }
        SegmentIndex::new(self.pool.as_str().to_string(), &self.trace)
    }
}

}
