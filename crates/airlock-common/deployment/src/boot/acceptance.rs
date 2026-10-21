//! Bind boot acceptance to generation and service observations from the same guest instances.
//! Probes supply observations; these checks neither run probes nor promise continuing health.

use vstd::prelude::*;

verus! {

/// One observed guest incarnation within a host boot; Xen identity zero is dom0.
pub struct Instance {
    /// Host kernel boot identity, preventing evidence reuse across host restarts.
    pub boot: String,
    /// Exact guest whose state was observed.
    pub domain: String,
    /// Positive Xen domain identity, assumed stable during the observation window.
    pub xid: u32,
}

impl Instance {
    /// Retain one immutable observation across capability checks; this does not resample a VM.
    pub fn snapshot(&self) -> (out: Self)
        ensures same(*self, out),
    {
        Self { boot: self.boot.clone(), domain: self.domain.clone(), xid: self.xid }
    }
}

/// Exact instance equality, including the host boot.
pub open spec fn same(a: Instance, b: Instance) -> bool {
    a.boot@ == b.boot@ && a.domain@ == b.domain@ && a.xid == b.xid
}

fn matching(a: &Instance, b: &Instance) -> (yes: bool)
    ensures yes == same(*a, *b),
{ a.boot == b.boot && a.domain == b.domain && a.xid == b.xid }

/// Service capabilities required independently of the guest answering qrexec.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Service {
    /// Active local graphical login.
    Desktop,
    /// Input receiver and dom0 forwarding discovery.
    Input,
    /// Healthy worker build-store service and authenticated mounted view.
    Worker,
    /// Healthy network-provider forwarding services.
    Network,
}

/// Effective boot requirements projected from selected topology and installed host policy.
pub struct Requirement {
    /// Declared guest identity.
    pub domain: String,
    /// Exact selected immutable system.
    pub system: String,
    /// Whether an active local graphical login is required.
    pub desktop: bool,
    /// Whether input receiver and forwarding health are required.
    pub input: bool,
    /// Whether this guest provides a required build worker.
    pub worker: bool,
    /// Whether this guest provides required network forwarding.
    pub network: bool,
}

/// Only the two installed worker roles require the build-store health protocol.
pub open spec fn worker_role(role: Option<&str>) -> bool {
    match role { Some(r) => r@ == "sys-ctrl"@ || r@ == "usr-assistant"@, None => false }
}

/// Input is required for the exact configured destination, never for an unrelated guest.
pub open spec fn input_for(target: Option<&str>, domain: Seq<char>) -> bool {
    match target { Some(t) => t@ == domain, None => false }
}

impl Requirement {
    /// Project every applicable capability without allowing a worker role to alias another guest.
    pub fn new(domain: String, system: String, desktop: bool, input_target: Option<&str>, role: Option<&str>, network: bool) -> (out: Option<Self>)
        ensures out.is_some() == (domain@.len() > 0 && domain@ != "dom0"@ && system@.len() > 0
            && (worker_role(role) ==> match role { Some(r) => r@ == domain@, None => false })),
            out.is_some() ==> out.unwrap().domain == domain && out.unwrap().system == system
                && out.unwrap().desktop == (desktop && domain@ == "sys-gui"@)
                && out.unwrap().input == input_for(input_target, domain@)
                && out.unwrap().worker == worker_role(role) && out.unwrap().network == network,
    {
        let worker = match role { Some(r) => r == "sys-ctrl" || r == "usr-assistant", None => false };
        let role_matches = match role { Some(r) => r == domain.as_str(), None => false };
        if domain.is_empty() || domain.as_str() == "dom0" || system.is_empty() || (worker && !role_matches) { return None; }
        let input = match input_target { Some(t) => t == domain.as_str(), None => false };
        let desktop = desktop && domain.as_str() == "sys-gui";
        Some(Self { domain, system, desktop, input, worker, network })
    }
}

/// Equality includes every required capability, so later checks cannot weaken the policy.
pub open spec fn equivalent(a: Requirement, b: Requirement) -> bool {
    a.domain@ == b.domain@ && a.system@ == b.system@ && a.desktop == b.desktop
        && a.input == b.input && a.worker == b.worker && a.network == b.network
}

fn equivalent_exec(a: &Requirement, b: &Requirement) -> (yes: bool)
    ensures yes == equivalent(*a, *b),
{
    a.domain == b.domain && a.system == b.system && a.desktop == b.desktop
        && a.input == b.input && a.worker == b.worker && a.network == b.network
}

/// Matching generation observed between two observations of the same running guest.
pub struct GenerationReady {
    // Original observed incarnation; never reconstructed from a later caller's target.
    instance: Instance,
    // Actual immutable generation whose identity matched the expectation.
    system: String,
}

impl View for GenerationReady {
    type V = (Instance, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.instance, self.system@) }
}

impl GenerationReady {
    /// Reject a wrong generation, missing identity or restart during the generation probe.
    pub fn check(expected: &str, actual: String, before: Instance, after: &Instance) -> (out: Option<Self>)
        ensures out.is_some() == (expected@.len() > 0 && actual@ == expected@
            && before.boot@.len() > 0 && before.domain@.len() > 0 && before.xid > 0 && same(before, *after)),
            out.is_some() ==> out.unwrap()@ == (before, expected@),
    {
        if expected.is_empty() || actual.as_str() != expected || before.boot.is_empty()
            || before.domain.is_empty() || before.xid == 0 || !matching(&before, after) { return None; }
        Some(Self { instance: before, system: actual })
    }
}

/// A successful service probe bracketed by the same running guest incarnation.
pub struct ServiceReady {
    // Capability actually queried by the external adapter.
    service: Service,
    // Instance observed before and after that successful query.
    instance: Instance,
}

impl View for ServiceReady {
    type V = (Service, Instance);

    closed spec fn view(&self) -> Self::V { (self.service, self.instance) }
}

impl ServiceReady {
    /// The adapter calls this only after the fixed read-only capability probe succeeds.
    pub fn check(service: Service, before: Instance, after: &Instance) -> (out: Option<Self>)
        ensures out.is_some() == (before.boot@.len() > 0 && before.domain@.len() > 0
            && before.xid > 0 && same(before, *after)),
            out.is_some() ==> out.unwrap()@ == (service, before),
    {
        if before.boot.is_empty() || before.domain.is_empty() || before.xid == 0
            || !matching(&before, after) { return None; }
        Some(Self { service, instance: before })
    }
}

/// Independent evidence requirement: the requested capability belongs to this exact incarnation.
pub closed spec fn covers(evidence: Seq<ServiceReady>, service: Service, instance: Instance) -> bool {
    exists|i: int| 0 <= i < evidence.len()
        && #[trigger] evidence[i].service == service && same(evidence[i].instance, instance)
}

fn service_matches(a: Service, b: Service) -> (yes: bool)
    ensures yes == (a == b),
{
    matches!((a, b), (Service::Desktop, Service::Desktop) | (Service::Input, Service::Input)
        | (Service::Worker, Service::Worker) | (Service::Network, Service::Network))
}

fn covered(evidence: &[ServiceReady], service: Service, instance: &Instance) -> (yes: bool)
    ensures yes == covers(evidence@, service, *instance),
{
    let mut i = 0;
    while i < evidence.len()
        invariant 0 <= i <= evidence.len(),
            forall|j: int| 0 <= j < i ==> !(#[trigger] evidence[j].service == service
                && same(evidence[j].instance, *instance)),
        decreases evidence.len() - i,
    {
        if service_matches(evidence[i].service, service) && matching(&evidence[i].instance, instance) { return true; }
        i += 1;
    }
    false
}

/// Complete readiness for one declared guest; fields cannot be fabricated outside this module.
///
/// ```compile_fail
/// use airlock_deployment_core::boot::acceptance::GuestReady;
/// // A caller cannot replace the checked capability set.
/// let evidence = GuestReady { requirement: todo!(), generation: todo!(), _services: vec![] };
/// ```
pub struct GuestReady {
    // Complete effective requirements used by the successful check.
    requirement: Requirement,
    // Generation and incarnation checked by the production generation gate.
    generation: GenerationReady,
    // Retained service evidence establishing every required capability.
    _services: Vec<ServiceReady>,
}

/// Every required capability has same-instance evidence, in addition to the selected generation.
pub closed spec fn satisfied(r: Requirement, g: GenerationReady, s: Seq<ServiceReady>) -> bool {
    r.domain@ == g.instance.domain@ && r.system@ == g.system@
        && g.instance.boot@.len() > 0 && g.instance.xid > 0 && r.system@.len() > 0
        && (r.desktop ==> covers(s, Service::Desktop, g.instance))
        && (r.input ==> covers(s, Service::Input, g.instance))
        && (r.worker ==> covers(s, Service::Worker, g.instance))
        && (r.network ==> covers(s, Service::Network, g.instance))
}

impl View for GuestReady {
    type V = (Requirement, GenerationReady, Seq<ServiceReady>);

    closed spec fn view(&self) -> Self::V { (self.requirement, self.generation, self._services@) }
}

impl GuestReady {
    /// Every constructible guest readiness value has all its required evidence.
    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool { satisfied(self.requirement, self.generation, self._services@) }

    /// Refuse missing capabilities and evidence for another guest, generation or incarnation.
    pub fn check(requirement: Requirement, generation: GenerationReady, services: Vec<ServiceReady>) -> (out: Option<Self>)
        ensures out.is_some() == satisfied(requirement, generation, services@),
            out.is_some() ==> out.unwrap()@ == (requirement, generation, services@)
                && satisfied(out.unwrap()@.0, out.unwrap()@.1, out.unwrap()@.2),
    {
        if requirement.domain != generation.instance.domain || requirement.system != generation.system
            || generation.instance.boot.is_empty() || generation.instance.xid == 0 || requirement.system.is_empty()
            || (requirement.desktop && !covered(&services, Service::Desktop, &generation.instance))
            || (requirement.input && !covered(&services, Service::Input, &generation.instance))
            || (requirement.worker && !covered(&services, Service::Worker, &generation.instance))
            || (requirement.network && !covered(&services, Service::Network, &generation.instance)) { return None; }
        Some(Self { requirement, generation, _services: services })
    }
}

/// The two intentional publication points: usable desktop and complete required boot services.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Background workers may still be starting; critical desktop startup has completed.
    Desktop,
    /// All required guests and applicable service capabilities have been observed ready.
    Complete,
}

/// Complete ordered evidence, with a final incarnation observation for every required guest.
pub closed spec fn acceptable(boot: Seq<char>, scope: Scope, requirements: Seq<Requirement>, guests: Seq<GuestReady>, current: Seq<Instance>) -> bool {
    boot.len() > 0 && requirements.len() == guests.len() && guests.len() == current.len()
        && (scope is Desktop ==> exists|i: int| 0 <= i < requirements.len() && #[trigger] requirements[i].desktop)
        && forall|i: int| 0 <= i < requirements.len() ==> equivalent(#[trigger] requirements[i], guests[i].requirement)
            && satisfied(guests[i].requirement, guests[i].generation, guests[i]._services@)
            && guests[i].generation.instance.boot@ == boot
            && same(guests[i].generation.instance, current[i])
}

/// Opaque permission to publish a readiness observation, not persistent authority or a health lease.
pub struct AcceptedBoot {
    // Host boot to which all checked guest observations belong.
    boot: String,
    // Publication point for which the complete requirement set was collected.
    scope: Scope,
    // Whether checked evidence includes the guarded graphical desktop.
    desktop: bool,
}

impl View for AcceptedBoot {
    type V = (Seq<char>, Scope, bool);

    closed spec fn view(&self) -> Self::V { (self.boot@, self.scope, self.desktop) }
}

impl AcceptedBoot {
    /// Check the complete requirement set and reject stale evidence at final observation.
    pub fn check(boot: String, scope: Scope, requirements: &[Requirement], guests: &[GuestReady], current: &[Instance]) -> (out: Option<Self>)
        ensures out.is_some() == acceptable(boot@, scope, requirements@, guests@, current@),
            out.is_some() ==> out.unwrap()@ == (boot@, scope, exists|i: int| 0 <= i < requirements.len() && #[trigger] requirements[i].desktop),
    {
        if boot.is_empty() || requirements.len() != guests.len() || guests.len() != current.len() { return None; }
        let mut i = 0;
        let mut desktop = false;
        while i < requirements.len()
            invariant desktop == (exists|j: int| 0 <= j < i && #[trigger] requirements[j].desktop),
                0 <= i <= requirements.len(), requirements.len() == guests.len(), guests.len() == current.len(),
                forall|j: int| 0 <= j < i ==> equivalent(#[trigger] requirements[j], guests[j].requirement)
                    && satisfied(guests[j].requirement, guests[j].generation, guests[j]._services@)
                    && guests[j].generation.instance.boot@ == boot@
                    && same(guests[j].generation.instance, current[j]),
            decreases requirements.len() - i,
        {
            let g = &guests[i];
            if !equivalent_exec(&requirements[i], &g.requirement) || g.generation.instance.boot != boot
                || !matching(&g.generation.instance, &current[i]) { return None; }
            // GuestReady's private constructor is the only source of this evidence.
            proof { use_type_invariant(g); }
            desktop = desktop || requirements[i].desktop;
            i += 1;
        }
        if matches!(scope, Scope::Desktop) && !desktop { return None; }
        Some(Self { boot, scope, desktop })
    }

    /// Only checked graphical evidence can establish that the desktop was usable.
    pub fn desktop(&self) -> (yes: bool)
        ensures yes == self@.2,
    { self.desktop }

    /// Publication cannot substitute a different host boot or acceptance scope.
    pub fn permits(&self, boot: &str, scope: Scope) -> (yes: bool)
        ensures yes == (self@.0 == boot@ && self@.1 == scope),
    { self.boot.as_str() == boot && matches!((self.scope, scope), (Scope::Desktop, Scope::Desktop) | (Scope::Complete, Scope::Complete)) }
}

}
