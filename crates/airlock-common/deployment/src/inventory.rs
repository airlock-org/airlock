//! Account for every decoded Qubes inventory row before permitting a shutdown.
//! Qubes observations and baseline relationships remain external evidence.

use airlock_machine_planning::names::owned_contains;
use vstd::prelude::*;

verus! {

/// Classification of Qubes domain activity used by shutdown observation.
/// Qubes' [`get_power_state`](https://github.com/QubesOS/qubes-core-admin/blob/v4.3.44/qubes/vm/qubesvm.py#L2250)
/// distinguishes paused and other active states from `Halted`.
#[derive(Clone, Copy)]
pub enum PowerClass {
    /// Qubes reports that the domain has stopped.
    Halted,
    /// A known non-halted state, including paused and partially started domains.
    Active,
    /// An unsupported or missing state; shutdown must fail closed.
    Uncertain,
}

/// Opaque classification that must be constructed from a decoded Qubes STATE field.
/// Callers cannot mark a domain halted by independently supplying an activity flag.
///
/// ```compile_fail
/// use airlock_deployment_core::inventory::{PowerClass, PowerState};
/// // Classification cannot bypass the verified raw-state decision.
/// let power = PowerState { class: PowerClass::Halted };
/// ```
#[derive(Clone, Copy)]
pub struct PowerState {
    // Only the verified raw-state classifier can construct this classification.
    class: PowerClass,
}

impl View for PowerState {
    type V = PowerClass;

    closed spec fn view(&self) -> PowerClass { self.class }
}

/// Qubes states in which a domain can still depend on another domain's services.
pub open spec fn active_state(state: Seq<char>) -> bool {
    state == "Running"@ || state == "Paused"@ || state == "Transient"@
        || state == "Halting"@ || state == "Dying"@ || state == "Crashed"@
        || state == "Suspended"@
}

impl PowerState {
    /// Distinguish supported Qubes activity states from missing or unsupported evidence.
    pub fn is_known(&self) -> (known: bool)
        ensures known == !(self@ is Uncertain),
    { !matches!(self.class, PowerClass::Uncertain) }

    /// Require an explicit halted observation before configuration or destructive work.
    pub fn is_halted(&self) -> (halted: bool)
        ensures halted == (self@ is Halted),
    { matches!(self.class, PowerClass::Halted) }

    /// Classify Qubes' decoded STATE field without treating unknown states as halted.
    pub fn classify(state: &str) -> (power: Self)
        ensures
            (power@ is Halted) == (state@ == "Halted"@),
            (power@ is Active) == active_state(state@),
            (power@ is Uncertain) == (state@ != "Halted"@ && !active_state(state@)),
    {
        if state == "Halted" { Self { class: PowerClass::Halted } }
        else if state == "Running" || state == "Paused" || state == "Transient"
            || state == "Halting" || state == "Dying" || state == "Crashed"
            || state == "Suspended" { Self { class: PowerClass::Active } }
        else { Self { class: PowerClass::Uncertain } }
    }
}

/// One decoded inventory row and its separately observed dependency evidence.
pub struct DomainObservation {
    /// Qubes VM name; the adapter validates names while decoding every inventory row.
    pub name: String,
    /// Verified classification of the raw Qubes power-state field.
    pub power: PowerState,
    /// Live GuiVM property; required for every active peer, even during normal shutdown.
    pub gui_provider: Option<String>,
    /// Whether protected baseline relationships identify this peer as a hard dependent.
    pub hard_dependent: bool,
    /// Whether the protected baseline declares this domain; unknown active peers block force.
    pub declared: bool,
}

/// Active peers whose GUI property must be queried before dispatch.
pub open spec fn relevant(row: DomainObservation, target: Seq<char>) -> bool {
    row.name@ != "dom0"@ && row.name@ != target && row.power@ is Active
}

/// A complete row either cannot be affected or meets all shutdown dependency rules.
pub open spec fn row_permitted(row: DomainObservation, target: Seq<char>,
    expected: Seq<String>, forced: bool) -> bool {
    &&& !(row.power@ is Uncertain)
    &&& (relevant(row, target) ==> !row.hard_dependent && (!forced || row.declared)
        && match row.gui_provider {
            Some(provider) => provider@ != target || airlock_machine_planning::names::owned_member(expected, row.name@),
            None => false,
        })
}

impl DomainObservation {
    /// Select every known active peer for a GUI query, including paused VMs.
    /// Uncertain states are rejected by the request gate even without a query.
    pub fn needs_probe(&self, target: &str) -> (needed: bool)
        ensures needed == relevant(*self, target@),
    {
        self.name.as_str() != "dom0" && self.name.as_str() != target
            && matches!(self.power.class, PowerClass::Active)
    }

    /// Check live GUI evidence and protected relationships for this inventory row.
    fn permits(&self, target: &str, expected: &[String], forced: bool) -> (accepted: bool)
        ensures accepted == row_permitted(*self, target@, expected@, forced),
    {
        if matches!(self.power.class, PowerClass::Uncertain) { return false; }
        if !self.needs_probe(target) { return true; }
        if self.hard_dependent || (forced && !self.declared) { return false; }

        match &self.gui_provider {
            Some(provider) => provider.as_str() != target
                || owned_contains(expected, self.name.as_str()),
            None => false,
        }
    }
}

/// Full inventory evidence for one shutdown target; this value grants no authority.
/// Missing active-peer observations and uncertain power states prevent requests.
pub struct ShutdownObservation {
    // Binds target-relative hard-dependency evidence to the requested provider.
    target: String,
    // Every decoded inventory row, including halted, paused and unknown states.
    domains: Vec<DomainObservation>,
}

/// Immutable observation inputs retained until the request gate checks every row.
pub ghost struct Snapshot {
    /// Target against which the adapter observed hard dependencies.
    pub target: Seq<char>,
    /// Complete decoded inventory and per-row evidence.
    pub domains: Seq<DomainObservation>,
}

impl View for ShutdownObservation {
    type V = Snapshot;

    closed spec fn view(&self) -> Snapshot {
        Snapshot { target: self.target@, domains: self.domains@ }
    }
}

/// A request accounts for every inventory row and preserves the observation target.
pub open spec fn permitted(snapshot: Snapshot, target: Seq<char>,
    expected: Seq<String>, forced: bool) -> bool {
    snapshot.target == target && forall|i: int| 0 <= i < snapshot.domains.len() ==>
        #[trigger] row_permitted(snapshot.domains[i], target, expected, forced)
}

/// An active peer whose live GUI connection would be affected by stopping this target.
pub open spec fn gui_client(row: DomainObservation, target: Seq<char>) -> bool {
    relevant(row, target) && match row.gui_provider {
        Some(provider) => provider@ == target,
        None => false,
    }
}

/// All active GUI clients in the supplied inventory belong to the approved scope.
pub open spec fn clients_within(snapshot: Snapshot, target: Seq<char>, allowed: Seq<Seq<char>>) -> bool {
    forall|i: int| 0 <= i < snapshot.domains.len() && #[trigger] gui_client(snapshot.domains[i], target)
        ==> allowed.contains(snapshot.domains[i].name@)
}

/// Compose complete observation checking with the checked plan's original approval.
pub proof fn approved_clients(snapshot: Snapshot, target: Seq<char>, expected: Seq<String>,
    forced: bool, allowed: Seq<Seq<char>>)
    requires
        permitted(snapshot, target, expected, forced),
        forall|j: int| 0 <= j < expected.len() ==> allowed.contains(#[trigger] expected[j]@),
    ensures clients_within(snapshot, target, allowed),
{
    assert forall|i: int| 0 <= i < snapshot.domains.len() && #[trigger] gui_client(snapshot.domains[i], target)
        implies allowed.contains(snapshot.domains[i].name@) by {
        assert(row_permitted(snapshot.domains[i], target, expected, forced));
        assert(airlock_machine_planning::names::owned_member(expected, snapshot.domains[i].name@));
        let j = choose|j: int| 0 <= j < expected.len()
            && #[trigger] expected[j]@ == snapshot.domains[i].name@;
        assert(allowed.contains(expected[j]@));
    }
}

impl ShutdownObservation {
    /// Retain all decoded inventory rows, without accepting or filtering their evidence.
    pub fn new(target: String, domains: Vec<DomainObservation>) -> (observation: Self)
        ensures observation@.target == target@, observation@.domains == domains@,
    {
        Self { target, domains }
    }

    /// Borrow the complete immutable inventory for diagnostics and adapter tests.
    pub fn domains(&self) -> (domains: &[DomainObservation])
        ensures domains@ == self@.domains,
    { self.domains.as_slice() }

    /// Decide whether all inventory rows permit this exact target, mode and client set.
    pub(crate) fn permits(&self, target: &str, expected: &[String], forced: bool) -> (accepted: bool)
        ensures accepted == permitted(self@, target@, expected@, forced),
    {
        if self.target.as_str() != target { return false; }

        let mut i = 0;
        while i < self.domains.len()
            invariant
                0 <= i <= self.domains.len(),
                self@.target == target@,
                forall|k: int| 0 <= k < i ==>
                    #[trigger] row_permitted(self@.domains[k], target@, expected@, forced),
            decreases self.domains.len() - i,
        {
            if !self.domains[i].permits(target, expected, forced) {
                assert(!row_permitted(self@.domains[i as int], target@, expected@, forced));
                return false;
            }
            i += 1;
        }
        true
    }
}

}
