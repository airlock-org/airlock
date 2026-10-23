//! A failed observation cannot establish that storage has no kernel consumer.
//! Adapters supply a complete, successfully decoded mapping inventory or no inventory.
//! Truthful kernel enumeration and exclusion of concurrent Airlock attach operations remain trusted.

use vstd::prelude::*;

verus! {

/// What a completed kernel inventory establishes about one exact mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// The inventory contains the original mapping name.
    Present,
    /// A complete inventory excludes the original mapping name.
    Absent,
    /// Failed, incomplete or malformed evidence establishes neither outcome.
    Unknown,
}

/// Persisted preparation must survive collection until serialized recovery reconciles it.
#[derive(Clone, Copy)]
pub enum Phase {
    /// Published before attempting external creation.
    Preparing,
    /// Creation completed and the mapping may still have consumers.
    Active,
    /// Removal requested; deferred removal may still be outstanding.
    Retiring,
}

impl Phase {
    /// Encode exactly the lifecycle whose protection rules were checked.
    pub fn status(&self) -> (out: &'static str)
        ensures (*self is Preparing ==> out@ == "preparing"@),
            (*self is Active ==> out@ == "active"@),
            (*self is Retiring ==> out@ == "retiring"@),
    {
        match self {
            Self::Preparing => "preparing",
            Self::Active => "active",
            Self::Retiring => "retiring",
        }
    }

    /// Reject unclassified persisted states rather than silently releasing their protection.
    pub fn decode(status: &str) -> (out: Option<Self>)
        ensures out.is_some() == (status@ == "preparing"@ || status@ == "active"@ || status@ == "retiring"@),
            out.is_some() ==> (out.unwrap() is Preparing <==> status@ == "preparing"@),
            out.is_some() ==> (out.unwrap() is Active <==> status@ == "active"@),
            out.is_some() ==> (out.unwrap() is Retiring <==> status@ == "retiring"@),
    {
        if status == "preparing" { Some(Self::Preparing) }
        else if status == "active" { Some(Self::Active) }
        else if status == "retiring" { Some(Self::Retiring) }
        else { None }
    }
}

/// Collection cannot discard an interrupted preparation; recovery holds attach exclusion.
#[derive(Clone, Copy)]
pub enum Context {
    /// Read-only tracing during collection or compaction.
    Collection,
    /// Explicit detach or recovery serialized against Airlock creation.
    Reconciliation,
}

/// A complete inventory contains only nonempty identities; absence is exact string exclusion.
pub open spec fn complete(names: Seq<String>) -> bool {
    forall|i: int| 0 <= i < names.len() ==> #[trigger] names[i]@.len() > 0
}

/// Independent membership specification, unaffected by inventory ordering or duplicates.
pub open spec fn contains(names: Seq<String>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < names.len() && #[trigger] names[i]@ == name
}

/// Invalid identity or incomplete evidence has no affirmative absence interpretation.
pub open spec fn classification(name: Seq<char>, inventory: Option<Vec<String>>) -> Presence {
    match inventory {
        Some(names) => if name.len() == 0 || !complete(names@) { Presence::Unknown }
            else if contains(names@, name) { Presence::Present } else { Presence::Absent },
        None => Presence::Unknown,
    }
}

/// Observation tied to its original mapping name; certainty is derived from supplied inventory evidence.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::runtime::{Observation, Presence};
/// /// An observer cannot turn a failed query into affirmative absence by editing its result.
/// fn change(observation: &mut Observation) { observation.presence = Presence::Absent; }
/// ```
pub struct Observation {
    // Original queried identity, retained through permission construction and dispatch.
    name: String,
    // Result of exact membership in the supplied complete inventory.
    presence: Presence,
}

impl Observation {
    /// Original mapping identity, independent of the observation outcome.
    pub closed spec fn identity(&self) -> Seq<char> { self.name@ }

    /// Certainty established by the supplied inventory.
    pub closed spec fn result(&self) -> Presence { self.presence }

    /// Interpret a complete successful enumeration; missing or invalid input remains unknown.
    pub fn from_inventory(name: String, inventory: Option<Vec<String>>) -> (out: Self)
        ensures out.identity() == name@, out.result() == classification(name@, inventory),
    {
        if name.is_empty() { return Self { name, presence: Presence::Unknown }; }
        let names = match inventory {
            None => return Self { name, presence: Presence::Unknown },
            Some(names) => names,
        };
        let mut i = 0;
        let mut found = false;
        while i < names.len()
            invariant 0 <= i <= names.len(), name@.len() > 0, inventory == Some(names),
                forall|j: int| 0 <= j < i ==> #[trigger] names[j]@.len() > 0,
                found == (exists|j: int| 0 <= j < i && #[trigger] names[j]@ == name@),
            decreases names.len() - i,
        {
            if names[i].is_empty() {
                proof {
                    assert(!complete(names@));
                    assert(inventory == Some(names));
                    assert(classification(name@, inventory) is Unknown);
                }
                return Self { name, presence: Presence::Unknown };
            }
            found = found || names[i] == name;
            i += 1;
        }
        Self { name, presence: if found { Presence::Present } else { Presence::Absent } }
    }

    /// Expose certainty without collapsing unknown outcomes into absence.
    pub fn presence(&self) -> (out: Presence)
        ensures out == self.result(),
    { self.presence }

    /// Exact mapping whose inventory evidence produced this observation.
    pub fn name(&self) -> (out: &str)
        ensures out@ == self.identity(),
    { self.name.as_str() }

    /// Permit pin release only for the original observed absent mapping and a supported lifecycle.
    pub fn release(self, pool: String, phase: Phase, context: Context) -> (out: Option<ReleaseRequest>)
        ensures out.is_some() == (pool@.len() > 0 && releasable(self, phase, context)),
            out.is_some() ==> out.unwrap().pool_name() == pool@ && out.unwrap().target_name() == self.identity(),
    {
        if pool.is_empty() || self.name.is_empty() || !matches!(self.presence, Presence::Absent) {
            return None;
        }
        if matches!(phase, Phase::Preparing) && matches!(context, Context::Collection) {
            return None;
        }
        Some(ReleaseRequest { pool, observation: self, _phase: Ghost(phase), _context: Ghost(context) })
    }
}

/// Absence is necessary in every context; collection additionally preserves unpublished creation outcomes.
pub open spec fn releasable(o: Observation, phase: Phase, context: Context) -> bool {
    o.identity().len() > 0 && o.result() is Absent
        && (!(phase is Preparing) || context is Reconciliation)
}

/// Consumed permission for one original pool/mapping; no independent deletion target is required.
pub struct ReleaseRequest {
    // Original pool under whose admission exclusion the observation was made.
    pool: String,
    // Exact target and affirmative absence evidence.
    observation: Observation,
    // Validated lifecycle from the original runtime reference.
    _phase: Ghost<Phase>,
    // Collection or serialized reconciliation semantics used to authorize release.
    _context: Ghost<Context>,
}

impl ReleaseRequest {
    /// Original pool retained in the permission.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    /// Original mapping retained in the permission.
    pub closed spec fn target_name(&self) -> Seq<char> { self.observation.name@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.pool@.len() > 0 && releasable(self.observation, self._phase@, self._context@)
    }

    /// Preserve the original pool through filesystem dispatch.
    pub fn pool(&self) -> (out: &str)
        ensures out@ == self.pool_name(),
    { self.pool.as_str() }

    /// Preserve the original absent mapping through filesystem dispatch.
    pub fn name(&self) -> (out: &str)
        ensures out@ == self.target_name(),
    { self.observation.name.as_str() }
}

}
