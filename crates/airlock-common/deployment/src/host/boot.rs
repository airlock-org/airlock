//! Restore one boot binding before runtime coordination starts.
//! Decoding, canonical JSON projections, protected files and successful durable writes are
//! adapter obligations. The checked plan returns the exact documents the adapter publishes.
//! No persisted cursor is trusted: every boot rechecks evidence and replays the fixed writes.

use super::Observation;
use vstd::prelude::*;

verus! {

/// A guest's positive generation within its own runtime-store namespace.
pub struct Guest {
    /// Guest VM identity, not a host storage namespace.
    pub domain: String,
    /// Positive storage generation, independent of any other guest's numbering.
    pub generation: u64,
}

/// The decoder supplies sorted entries and their canonical JSON document together.
pub struct Guests {
    /// Complete entries decoded from this document, in canonical domain order.
    pub entries: Vec<Guest>,
    /// Exact canonical JSON bytes to restore, including the empty object.
    pub document: String,
}

/// Old bindings lack a full receipt; new bindings fix all host generation metadata.
pub enum Host {
    /// Compatibility requires the booted system to match, without a receipt theorem.
    Legacy {
        /// Immutable system recorded by the original protocol.
        system: String,
    },
    /// Full publication identity used by current producers.
    Checked(
        /// Original complete publication observation.
        Observation,
    ),
}

/// Selected release and the immutable content captured when its boot binding was saved.
pub struct Selection {
    /// Protected release directory.
    pub release: String,
    /// Original rollback baseline in this transaction.
    pub previous: String,
    /// Original deployment attempt.
    pub attempt: String,
    /// Effective machine profile.
    pub profile: String,
    /// Exact selection document, with extensions retained.
    pub document: String,
    /// Complete manifest/topology digest; absent only in the legacy protocol.
    pub fingerprint: Option<String>,
}

/// Saved machine state belonging to a single host boot.
pub struct Binding {
    /// Host identity authenticated against the boot storage record.
    pub host: Host,
    /// A selected deployment, or the initial image's guest snapshot.
    pub selection: Option<Selection>,
    /// Exact guest generations belonging to this boot.
    pub guests: Guests,
}

/// Only committed outcomes permit restoration; selecting permits snapshot publication alone.
#[derive(Clone, Copy)]
pub enum Phase {
    /// Snapshot saved after verification but before final status publication.
    Selecting,
    /// Whole-machine verification completed.
    Complete,
    /// Guest verification completed; the selected host still needs its reboot.
    RebootRequired,
    /// Every other known deployment phase.
    Incomplete,
}

/// Independently loaded transaction evidence, never inferred from selection alone.
pub struct Status {
    /// Attempt identified by the status content and its filename.
    pub attempt: String,
    /// Release this transaction actually selected.
    pub release: String,
    /// Baseline retained by this transaction.
    pub previous: String,
    /// Persisted progress, not evidence of faithful external execution.
    pub phase: Phase,
    /// Exact status document used to restore latest.json.
    pub document: String,
}

/// Independent release and journal observations loaded from the selected directory.
pub struct Release {
    /// Directory from which the manifest was loaded.
    pub path: String,
    /// Manifest's immutable host system.
    pub host: String,
    /// Manifest's machine profile.
    pub profile: String,
    /// Digest of the complete manifest and topology, not merely the host field.
    pub fingerprint: String,
    /// Manifest's complete declared map; empty is still an exact constraint.
    pub declared: Guests,
    /// Independently loaded staged.json map.
    pub staged: Guests,
    /// Independently loaded status for the selected transaction.
    pub status: Status,
}

/// Every guest has one positive, unambiguous generation.
pub open spec fn valid(guests: Guests) -> bool {
    guests.document@.len() > 0 && forall|i: int| 0 <= i < guests.entries.len() ==>
        guests.entries[i].domain@.len() > 0 && guests.entries[i].generation > 0
            && forall|j: int| 0 <= j < i ==> guests.entries[i].domain@ != #[trigger] guests.entries[j].domain@
}

/// Compare the entire canonical map, including domains and empty declarations.
pub open spec fn same(a: Guests, b: Guests) -> bool {
    a.document@ == b.document@ && a.entries.len() == b.entries.len()
        && forall|i: int| 0 <= i < a.entries.len() ==> a.entries[i].domain@ == b.entries[i].domain@
            && a.entries[i].generation == b.entries[i].generation
}

fn guests_valid(g: &Guests) -> (yes: bool)
    ensures yes == valid(*g),
{
    if g.document.is_empty() { return false; }

    let mut i = 0;
    while i < g.entries.len()
        invariant 0 <= i <= g.entries.len(), g.document@.len() > 0,
            forall|k: int| 0 <= k < i ==> g.entries[k].domain@.len() > 0 && g.entries[k].generation > 0
                && forall|j: int| 0 <= j < k ==> g.entries[k].domain@ != #[trigger] g.entries[j].domain@,
        decreases g.entries.len() - i,
    {
        if g.entries[i].domain.is_empty() || g.entries[i].generation == 0 { return false; }
        let mut j = 0;
        while j < i
            invariant 0 <= j <= i < g.entries.len(),
                forall|k: int| 0 <= k < j ==> g.entries[i as int].domain@ != #[trigger] g.entries[k].domain@,
            decreases i - j,
        {
            if g.entries[i].domain == g.entries[j].domain { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

fn guests_equal(a: &Guests, b: &Guests) -> (yes: bool)
    ensures yes == same(*a, *b),
{
    if a.document != b.document || a.entries.len() != b.entries.len() { return false; }

    let mut i = 0;
    while i < a.entries.len()
        invariant 0 <= i <= a.entries.len(), a.entries.len() == b.entries.len(), a.document@ == b.document@,
            forall|j: int| 0 <= j < i ==> a.entries[j].domain@ == b.entries[j].domain@
                && a.entries[j].generation == b.entries[j].generation,
        decreases a.entries.len() - i,
    {
        if a.entries[i].domain != b.entries[i].domain || a.entries[i].generation != b.entries[i].generation { return false; }
        i += 1;
    }
    true
}

/// Records must agree on host, complete guest maps, release contents and transaction endpoints.
pub open spec fn compatible(binding: Binding, boot: Observation, release: Option<Release>) -> bool {
    boot.system@.len() > 0 && boot.generation@.len() > 0 && boot.generation@ != "0"@ && boot.record@.len() > 0
        && (match binding.host { Host::Legacy { system } => system@ == boot.system@,
            Host::Checked(saved) => super::receipt::same(saved, boot) })
        && valid(binding.guests)
        && (binding.host is Checked && binding.selection.is_some() ==> binding.selection.unwrap().fingerprint.is_some())
        && match binding.selection {
            None => release.is_none(),
            Some(s) => release.is_some() && {
                let r = release.unwrap();
                s.release@.len() > 0 && s.previous@.len() > 0 && s.attempt@.len() > 0 && s.profile@.len() > 0
                    && s.document@.len() > 0 && r.fingerprint@.len() > 0 && r.status.document@.len() > 0
                    && s.release@ == r.path@ && r.host@ == boot.system@ && s.profile@ == r.profile@
                    && (s.fingerprint.is_some() ==> s.fingerprint.unwrap()@ == r.fingerprint@)
                    && s.attempt@ == r.status.attempt@ && s.release@ == r.status.release@ && s.previous@ == r.status.previous@
                    && same(binding.guests, r.declared) && same(binding.guests, r.staged)
            },
        }
}

/// Forget only the borrow, preserving every field of independently loaded evidence.
pub open spec fn loaded(release: Option<&Release>) -> Option<Release> {
    match release { Some(r) => Some(*r), None => None }
}

/// Snapshot publication may run immediately before the final committed status is published.
pub fn snapshot(binding: &Binding, boot: &Observation, release: Option<&Release>) -> (yes: bool)
    ensures yes == (compatible(*binding, *boot, loaded(release))
        && (release.is_some() ==> !(release.unwrap().status.phase is Incomplete))),
{
    if boot.system.is_empty() || boot.generation.is_empty() || boot.generation.as_str() == "0" || boot.record.is_empty() { return false; }

    let host_matches = match &binding.host {
        Host::Legacy { system } => system.as_str() == boot.system.as_str(),
        Host::Checked(saved) => super::receipt::equal(saved, boot),
    };
    assert(host_matches == match binding.host { Host::Legacy { system } => system@ == boot.system@,
        Host::Checked(saved) => super::receipt::same(saved, *boot) });
    if !host_matches || !guests_valid(&binding.guests) { return false; }

    let content_bound = match (&binding.host, &binding.selection) {
        (Host::Checked(_), Some(s)) => s.fingerprint.is_some(),
        _ => true,
    };
    if !content_bound { return false; }

    match (&binding.selection, release) {
        (None, None) => true,
        (Some(s), Some(r)) => {
            if s.release.is_empty() || s.previous.is_empty() || s.attempt.is_empty() || s.profile.is_empty()
                || s.document.is_empty() || r.fingerprint.is_empty() || r.status.document.is_empty()
                || s.release != r.path || r.host != boot.system || s.profile != r.profile
                || s.attempt != r.status.attempt || s.release != r.status.release || s.previous != r.status.previous
                || matches!(r.status.phase, Phase::Incomplete) { return false; }

            let unchanged = match &s.fingerprint { Some(saved) => saved.as_str() == r.fingerprint.as_str(), None => true };
            if !unchanged { return false; }

            guests_equal(&binding.guests, &r.declared) && guests_equal(&binding.guests, &r.staged)
        },
        _ => false,
    }
}

/// Exact writes selected by the checked plan; adapters receive no independent target identities.
pub enum Effect {
    /// Publish the bound selection, or remove a stale selection for an image snapshot.
    Selection(
        /// Exact bound selection document, or absence for an image snapshot.
        Option<String>,
    ),
    /// Publish the corresponding journal, or remove stale latest status.
    Latest(
        /// Exact independently checked status, or absence for an image snapshot.
        Option<String>,
    ),
    /// Publish the complete saved guest map, even when empty.
    Guests(
        /// Complete canonical guest-generation document.
        String,
    ),
    /// Point the controller at the immutable booted host.
    Controller(
        /// Immutable booted host supplying the controller executable.
        String,
    ),
}

/// Each step publishes exactly the accepted binding or its independently checked status.
pub open spec fn requested(effect: Effect, b: Binding, r: Option<Release>, system: Seq<char>, step: nat) -> bool {
    match effect {
        Effect::Selection(doc) => step == 0 && match b.selection { Some(s) => doc.is_some() && doc.unwrap()@ == s.document@, None => doc.is_none() },
        Effect::Latest(doc) => step == 1 && match r { Some(r) => doc.is_some() && doc.unwrap()@ == r.status.document@, None => doc.is_none() },
        Effect::Guests(doc) => step == 2 && doc@ == b.guests.document@,
        Effect::Controller(host) => step == 3 && host@ == system,
    }
}

/// Only a checked restoration can construct a publication request.
/// The filesystem adapter consumes this value instead of accepting an independently built effect.
///
/// ```compile_fail,E0451
/// use airlock_deployment_core::host::boot::{Effect, RestoreRequest};
/// let request = RestoreRequest { effect: Effect::Controller("/wrong-host".into()) };
/// ```
pub struct RestoreRequest {
    // Exact next effect selected by the accepted plan, with no public raw constructor.
    effect: Effect,
}

impl View for RestoreRequest {
    type V = Effect;

    closed spec fn view(&self) -> Effect { self.effect }
}

impl RestoreRequest {
    /// Consume the opaque request and expose its unchanged payload to the filesystem adapter.
    pub fn into_effect(self) -> (out: Effect)
        ensures out == self@,
    { self.effect }
}

/// A validated boot restoration. Repeated boots rebuild this plan from original evidence.
pub struct Restoration<'a> {
    // Original saved binding, retained through every publication request.
    binding: &'a Binding,
    // Independently loaded release and journal; absent only for an image snapshot.
    release: Option<&'a Release>,
    // Exact host system selected by the boot entry and authenticated storage loader.
    boot: &'a Observation,
    // Volatile write cursor; never reconstructed from an incomplete on-disk write set.
    cursor: u8,
}

impl<'a> View for Restoration<'a> {
    type V = (Binding, Observation, Option<Release>, u8);

    closed spec fn view(&self) -> Self::V {
        (*self.binding, *self.boot, loaded(self.release), self.cursor)
    }
}

impl<'a> Restoration<'a> {
    /// Accepted evidence and a finite cursor remain fixed through publication and replay.
    pub closed spec fn invariant(&self) -> bool {
        compatible(*self.binding, *self.boot, loaded(self.release)) && self.cursor <= 4
            && (self.release.is_some() ==> matches!(self.release.unwrap().status.phase, Phase::Complete | Phase::RebootRequired))
    }

    /// Refuse incomplete transactions and any inconsistent evidence before producing a write.
    pub fn reconcile(binding: &'a Binding, boot: &'a Observation, release: Option<&'a Release>) -> (out: Option<Self>)
        ensures out.is_some() == (compatible(*binding, *boot, loaded(release))
            && (release.is_some() ==> matches!(release.unwrap().status.phase, Phase::Complete | Phase::RebootRequired))),
            out.is_some() ==> out.unwrap()@ == (*binding, *boot, loaded(release), 0u8) && out.unwrap().invariant(),
    {
        if !snapshot(binding, boot, release) { return None; }

        let committed = match release { Some(r) => matches!(r.status.phase, Phase::Complete | Phase::RebootRequired), None => true };
        if !committed { return None; }

        Some(Self { binding, release, boot, cursor: 0 })
    }

    /// Request fixed, idempotent publications in order. An I/O error aborts this plan;
    /// restart revalidates all evidence and begins again rather than skipping effects.
    // Explicit matches retain payload equality in Verus; Option::map loses that contract.
    #[allow(clippy::manual_map)]
    pub fn next_effect(&mut self) -> (out: Option<RestoreRequest>)
        ensures old(self).invariant() ==> final(self).invariant(),
            final(self)@.0 == old(self)@.0, final(self)@.1 == old(self)@.1, final(self)@.2 == old(self)@.2,
            out.is_some() == (old(self)@.3 < 4),
            out.is_some() ==> requested(out.unwrap()@, final(self)@.0, final(self)@.2, final(self)@.1.system@, old(self)@.3 as nat),
            final(self)@.3 == (if old(self)@.3 < 4 { old(self)@.3 + 1 } else { old(self)@.3 as int }),
    {
        let effect = match self.cursor {
            0 => Effect::Selection(match &self.binding.selection { Some(s) => Some(s.document.clone()), None => None }),
            1 => Effect::Latest(match self.release { Some(r) => Some(r.status.document.clone()), None => None }),
            2 => Effect::Guests(self.binding.guests.document.clone()),
            3 => Effect::Controller(self.boot.system.clone()),
            _ => return None,
        };

        self.cursor += 1;
        Some(RestoreRequest { effect })
    }
}

}
