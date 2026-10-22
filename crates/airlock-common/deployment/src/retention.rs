//! Preserve named generations and effective systems when releasing pins or reclaiming storage.
//! Inventory interpretation accounts for every supplied decoded source group. Authentication,
//! filesystem enumeration and durability remain adapter duties; independent administrators are not excluded.

/// Interpret complete release, journal and pool source groups.
pub mod inventory;

use vstd::prelude::*;

verus! {

/// One exact logical storage generation, within its consuming namespace.
pub struct Generation {
    /// Namespace owning the generation; equal numbers in other namespaces are distinct.
    pub domain: String,
    /// Positive generation number, never a relative selection such as latest.
    pub number: u64,
}

/// A reference that prevents reclamation, independent of where it was discovered.
pub enum Protection {
    /// Original journal pin, manifest pin, selected generation, running reference or retention pin.
    Exact(
        /// Exact protected namespace and generation.
        Generation,
    ),
    /// Unpinned effective configuration protects every matching retained generation.
    System {
        /// Namespace consuming this software system.
        domain: String,
        /// Immutable system source required by the effective configuration.
        system: String,
    },
}

/// Storage's retention classification; an explicit user decision is never automatically released.
pub enum Pin {
    /// Original admission pin with no later retention decision.
    Automatic,
    /// Explicitly retained, including a user reaffirming the original value.
    Explicit,
    /// Already released; repetition requires no new pin mutation.
    Released,
}

impl Pin {
    /// Interpret decoded retention evidence without treating a reaffirmed pin as automatic.
    pub fn classify(retained: bool, explicitly_updated: bool) -> (out: Self)
        ensures (out is Automatic <==> retained && !explicitly_updated),
            (out is Explicit <==> retained && explicitly_updated),
            (out is Released <==> !retained),
    {
        if !retained { Self::Released }
        else if explicitly_updated { Self::Explicit }
        else { Self::Automatic }
    }
}

/// The independently observed generation whose protection is being evaluated.
pub struct Candidate {
    /// Full namespace and numeric generation loaded from its committed record.
    pub generation: Generation,
    /// Immutable source loaded from that same record.
    pub system: String,
}

/// A reference protects exactly its namespace/generation or its namespace/system.
pub open spec fn protects(p: Protection, c: Candidate) -> bool {
    match p {
        Protection::Exact(g) => g.domain@ == c.generation.domain@ && g.number == c.generation.number,
        Protection::System { domain, system } => domain@ == c.generation.domain@ && system@ == c.system@,
    }
}

/// Unknown or incomplete protection coordinates cannot be interpreted as an absent reference.
pub open spec fn well_formed(p: Protection) -> bool {
    match p {
        Protection::Exact(g) => g.domain@.len() > 0 && g.number > 0,
        Protection::System { domain, system } => domain@.len() > 0 && system@.len() > 0,
    }
}

/// No reference in the complete supplied set permits deleting a matching generation.
pub open spec fn reclaimable(ps: Seq<Protection>, c: Candidate) -> bool {
    c.generation.domain@.len() > 0 && c.generation.number > 0 && c.system@.len() > 0
        && forall|i: int| 0 <= i < ps.len() ==> !protects(#[trigger] ps[i], c)
}

fn matches(p: &Protection, c: &Candidate) -> (yes: bool)
    ensures yes == protects(*p, *c),
{
    match p {
        Protection::Exact(g) => g.domain == c.generation.domain && g.number == c.generation.number,
        Protection::System { domain, system } => *domain == c.generation.domain && *system == c.system,
    }
}

/// Complete immutable references retained for every decision in one collection pass.
pub struct ProtectedGenerations {
    // Original references, including source-based compatibility protections.
    references: Vec<Protection>,
}

impl View for ProtectedGenerations {
    type V = Seq<Protection>;

    closed spec fn view(&self) -> Self::V { self.references@ }
}

impl ProtectedGenerations {
    /// Retain every supplied reference; invalid empty references refuse the entire pass.
    pub fn new(references: Vec<Protection>) -> (out: Option<Self>)
        ensures out.is_some() == (forall|i: int| 0 <= i < references.len() ==> well_formed(#[trigger] references[i])),
            out.is_some() ==> out.unwrap()@ == references@,
    {
        let mut i = 0;
        while i < references.len()
            invariant 0 <= i <= references.len(),
                forall|j: int| 0 <= j < i ==> well_formed(#[trigger] references[j]),
            decreases references.len() - i,
        {
            match &references[i] {
                Protection::Exact(g) => { if g.domain.is_empty() || g.number == 0 { return None; } },
                Protection::System { domain, system } => { if domain.is_empty() || system.is_empty() { return None; } },
            }
            i += 1;
        }
        Some(Self { references })
    }

    /// Read the complete immutable set for transport to the locked storage receiver.
    pub fn references(&self) -> (out: &[Protection]) ensures out@ == self@,
    { &self.references }

    /// Move the complete immutable set into a subsequent verified aggregation.
    pub fn into_references(self) -> (out: Vec<Protection>) ensures out@ == self@,
    { self.references }

    /// Permit exactly a well-formed candidate not protected by any supplied reference.
    pub fn reclaim(&self, candidate: Candidate) -> (out: Option<ReclaimRequest>)
        ensures out.is_some() == reclaimable(self@, candidate),
            out.is_some() ==> out.unwrap()@ == candidate && out.unwrap().reference_set() == self@,
    {
        if candidate.generation.domain.is_empty() || candidate.generation.number == 0 || candidate.system.is_empty() { return None; }

        let mut i = 0;
        while i < self.references.len()
            invariant 0 <= i <= self.references.len(),
                forall|j: int| 0 <= j < i ==> !protects(#[trigger] self.references[j], candidate),
            decreases self.references.len() - i,
        {
            if matches(&self.references[i], &candidate) { return None; }
            i += 1;
        }
        Some(ReclaimRequest { candidate, _references: Ghost(self@) })
    }

    /// Release only an original automatic pin, subject to the same complete protection checks.
    pub fn release<'a>(&'a self, candidate: Candidate, pin: Pin) -> (out: Option<ReleaseRequest<'a>>)
        ensures out.is_some() == (pin is Automatic && reclaimable(self@, candidate)),
            out.is_some() ==> out.unwrap()@ == candidate && out.unwrap().reference_set() == self@,
    {
        match pin {
            Pin::Automatic => self.reclaim(candidate).map(|request: ReclaimRequest| -> (out: ReleaseRequest<'a>)
                requires request.reference_set() == self@,
                ensures out@ == request@ && out.reference_set() == request.reference_set(),
            { ReleaseRequest { request, protected: self, _pin: Ghost(pin) } }),
            Pin::Explicit | Pin::Released => None,
        }
    }
}

/// Opaque permission to forget exactly one unprotected generation in the observed collection pass.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::ReclaimRequest;
/// /// A storage adapter cannot replace the checked generation with another deletion target.
/// fn substitute(request: &mut ReclaimRequest) {
///     request.candidate.generation.number = 9;
/// }
/// ```
pub struct ReclaimRequest {
    // Original committed candidate; adapters cannot resupply a different target.
    candidate: Candidate,
    // Erased witness retains the protection set checked for this candidate.
    _references: Ghost<Seq<Protection>>,
}

impl View for ReclaimRequest {
    type V = Candidate;

    closed spec fn view(&self) -> Self::V { self.candidate }
}

impl ReclaimRequest {
    /// Original checked references, retained only in erased proof evidence.
    pub closed spec fn reference_set(&self) -> Seq<Protection> { self._references@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool { reclaimable(self._references@, self.candidate) }

    /// Exact namespace passed to the storage adapter.
    pub fn domain(&self) -> (out: &str) ensures out@ == self@.generation.domain@,
    { self.candidate.generation.domain.as_str() }

    /// Exact original numeric generation, never looked up again as latest.
    pub fn generation(&self) -> (out: u64) ensures out == self@.generation.number,
    { self.candidate.generation.number }
}

/// Opaque automatic-pin release; explicit user retention cannot construct this request.
pub struct ReleaseRequest<'a> {
    // The same exact unprotected target accepted by reclamation.
    request: ReclaimRequest,
    // Borrow the original immutable set for transport; the adapter cannot substitute it.
    protected: &'a ProtectedGenerations,
    // Erased evidence that the checked original pin was automatic.
    _pin: Ghost<Pin>,
}

impl<'a> View for ReleaseRequest<'a> {
    type V = Candidate;

    closed spec fn view(&self) -> Self::V { self.request@ }
}

impl<'a> ReleaseRequest<'a> {
    /// Original protection set, without exposing mutable reference storage to callers.
    pub closed spec fn reference_set(&self) -> Seq<Protection> { self.request.reference_set() }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self._pin@ is Automatic && self.request.reference_set() == self.protected@
    }

    /// Transport the same protections used to permit this exact request.
    pub fn references(&self) -> (out: &[Protection])
        ensures out@ == self.reference_set(),
    {
        proof { use_type_invariant(self); }
        self.protected.references()
    }

    /// Exact original namespace for the pin-release adapter.
    pub fn domain(&self) -> (out: &str) ensures out@ == self@.generation.domain@,
    { self.request.domain() }

    /// Exact original generation for the pin-release adapter.
    pub fn generation(&self) -> (out: u64) ensures out == self@.generation.number,
    { self.request.generation() }
}

}
