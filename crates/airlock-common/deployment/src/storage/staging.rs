//! Bind complete generation selections to checked storage definitions and one executing attempt.
//! Committed-record authentication and source decoding remain adapter obligations.

use super::{Backend, Definition, StoragePlan, StoreRequest};
use crate::{
    boot::{SelectedGeneration, Selection},
    operation::{DurableOperation, ExecutionContext},
};
use vstd::prelude::*;

verus! {

/// Required staging work follows the original configure and stage fields, not adapter preference.
pub enum Requirement {
    /// The checked definition has no native storage to select.
    NonNative,
    /// Existing native storage keeps its original reuse/prepared policy when no staging was planned.
    Existing,
    /// Resolve committed storage, preserving an explicitly declared generation when present.
    Resolve(
        /// Original concrete generation, or an unpinned retained/prepared lookup obligation.
        Option<u64>,
    ),
}

/// Complete classification supplied after authenticating the requested committed records.
pub enum Resolution {
    /// Explicitly accounted-for nonnative definition.
    NonNative,
    /// Explicit unchanged/unpinned compatibility policy; it never permits new package admission.
    Existing,
    /// Complete boot identity checked against independently loaded committed evidence.
    Retained(
        /// Full identity already matched against committed storage evidence.
        Box<SelectedGeneration>,
    ),
}

/// Original native sources and fixed supported layout determine the meaning of a selection.
pub open spec fn agrees(d: Definition, s: Selection) -> bool {
    d.native.is_some() && {
        let n = d.native.unwrap();
        s.domain@ == d.name@ && s.generation > 0
            && s.system@ == n.system@ && s.root@ == n.root@ && s.source@ == n.source@ && s.kernel@ == n.kernel@
            && s.mode@ == "shared-closure"@ && s.device@ == "/dev/xvde"@ && s.blob_device@ == "/dev/xvdf"@
            && s.mountpoint@ == "/nix/store"@ && s.backend@ == "erofs-objectmap"@
            && (n.generation.is_some() ==> s.generation == n.generation.unwrap())
    }
}

/// Every native VM with an original pin or planned staging requires a concrete selection.
pub open spec fn required(d: Definition, stage: crate::effects::Scope) -> bool {
    d.native.is_some() && (d.native.unwrap().generation.is_some() || crate::effects::planned(stage, d.name@))
}

/// Required entries cannot disappear; unchanged entries cannot acquire unjustified generation selections.
pub open spec fn entry(d: Definition, stage: crate::effects::Scope, r: Resolution) -> bool {
    match r {
        Resolution::NonNative => d.native.is_none(),
        Resolution::Existing => d.native.is_some() && !required(d, stage),
        Resolution::Retained(s) => required(d, stage) && agrees(d, s@),
    }
}

fn selection_matches(d: &Definition, s: &Selection) -> (yes: bool)
    ensures yes == agrees(*d, *s),
{
    let n = match &d.native { Some(n) => n, None => return false };
    if s.domain != d.name || s.generation == 0 || s.system != n.system || s.root != n.root
        || s.source != n.source || s.kernel != n.kernel || s.mode.as_str() != "shared-closure"
        || s.device.as_str() != "/dev/xvde" || s.blob_device.as_str() != "/dev/xvdf"
        || s.mountpoint.as_str() != "/nix/store" || s.backend.as_str() != "erofs-objectmap" { return false; }
    match n.generation { Some(g) => s.generation == g, None => true }
}

fn entry_matches(plan: &StoragePlan, d: &Definition, r: &Resolution) -> (yes: bool)
    ensures yes == entry(*d, plan@.3, *r),
{
    let needed = d.native.is_some() && (d.native.as_ref().unwrap().generation.is_some() || plan.stage.contains(d.name.as_str()));
    match r {
        Resolution::NonNative => d.native.is_none(),
        Resolution::Existing => d.native.is_some() && !needed,
        Resolution::Retained(selected) => needed && selection_matches(d, selected.selection()),
    }
}

/// Accepted staging has one justified entry for every original configure coordinate.
pub open spec fn complete(definitions: Seq<Definition>, stage: crate::effects::Scope, entries: Seq<Resolution>) -> bool {
    definitions.len() == entries.len() && forall|i: int| 0 <= i < definitions.len() ==>
        entry(#[trigger] definitions[i], stage, entries[i])
            && definitions[i].name@.len() > 0
            && forall|j: int| 0 <= j < i ==> definitions[i].name@ != #[trigger] definitions[j].name@
}

/// Opaque selection set; decoded maps or individual generation numbers cannot replace it.
///
/// ```compile_fail,E0451
/// use airlock_deployment_core::storage::staging::StagedGenerations;
/// let staged = StagedGenerations { entries: Vec::new(), attempt: String::new(),
///     operation: String::new(), authority: String::new(), plan: String::new() };
/// ```
pub struct StagedGenerations {
    // Exact complete classification retained after checking the original storage plan.
    entries: Vec<Resolution>,
    // Attempt which authenticated these records under an executing durable intent.
    attempt: String,
    // Stable original operation identity, preserved by authorised recovery.
    operation: String,
    // Original source approval and effect scope encoding.
    authority: String,
    // Candidate, baseline, revision and complete immutable plan binding.
    plan: String,
}

impl View for StagedGenerations {
    type V = (Seq<Resolution>, Seq<char>, Seq<char>, Seq<char>, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.entries@, self.attempt@, self.operation@, self.authority@, self.plan@) }
}

/// A staging result cannot be substituted across identities, scopes, plans or attempts.
pub open spec fn matches_context(s: StagedGenerations, c: ExecutionContext) -> bool {
    s@.1 == c.attempt@ && s@.2 == c.operation@ && s@.3 == c.authority@ && s@.4 == c.plan@
}

impl StoragePlan {
    /// Determine staging obligations from the original checked definition and stage namespace.
    pub fn requirement(&self, index: usize) -> (out: Option<Requirement>)
        ensures out.is_some() == (index < self@.1.len()),
            out.is_some() ==> match out.unwrap() {
                Requirement::NonNative => self@.1[index as int].native.is_none(),
                Requirement::Existing => self@.1[index as int].native.is_some() && !required(self@.1[index as int], self@.3),
                Requirement::Resolve(pin) => required(self@.1[index as int], self@.3) && pin == self@.1[index as int].native.unwrap().generation,
            },
    {
        if index >= self.definitions.len() { return None; }

        let d = &self.definitions[index];
        match &d.native {
            None => Some(Requirement::NonNative),
            Some(n) => {
                if n.generation.is_some() || self.stage.contains(d.name.as_str()) { Some(Requirement::Resolve(n.generation)) }
                else { Some(Requirement::Existing) }
            },
        }
    }

    /// Validate the entire set before any backend runs; no missing entry becomes a fallback policy.
    pub fn check_staged(&self, durable: &DurableOperation, c: &ExecutionContext, entries: Vec<Resolution>) -> (out: Option<StagedGenerations>)
        ensures out.is_some() == (crate::operation::executing(durable, c) && self@.2 == c.plan@
                && complete(self@.1, self@.3, entries@)),
            out.is_some() ==> complete(self@.1, self@.3, out.unwrap()@.0),
            out.is_some() ==> out.unwrap()@ == (entries@, c.attempt@, c.operation@, c.authority@, c.plan@),
    {
        if !durable.allows_effect(c) || self.binding.as_str() != c.plan.as_str() || entries.len() != self.definitions.len() { return None; }

        let mut i = 0;
        while i < entries.len()
            invariant 0 <= i <= entries.len(), entries.len() == self.definitions.len(),
                forall|k: int| 0 <= k < i ==> entry(#[trigger] self.definitions[k], self@.3, entries[k])
                    && self.definitions[k].name@.len() > 0
                    && forall|j: int| 0 <= j < k ==> self.definitions[k].name@ != #[trigger] self.definitions[j].name@,
            decreases entries.len() - i,
        {
            let d = &self.definitions[i];
            if d.name.is_empty() || !entry_matches(self, d, &entries[i]) { return None; }

            let mut j = 0;
            while j < i
                invariant 0 <= j <= i < self.definitions.len(),
                    forall|k: int| 0 <= k < j ==> self.definitions[i as int].name@ != #[trigger] self.definitions[k].name@,
                decreases i - j,
            {
                if self.definitions[i].name.as_str() == self.definitions[j].name.as_str() { return None; }
                j += 1;
            }
            i += 1;
        }
        Some(StagedGenerations { entries, attempt: c.attempt.as_str().to_string(), operation: c.operation.as_str().to_string(),
            authority: c.authority.as_str().to_string(), plan: c.plan.as_str().to_string() })
    }

    /// Bind the checked entry to its original backend and fresh executing context.
    pub fn select_staged<'a>(&'a self, durable: &'a DurableOperation, c: &ExecutionContext,
        staged: &'a StagedGenerations, index: usize) -> (out: Option<StagedBackend<'a>>)
        ensures out.is_some() ==> crate::operation::executing(durable, c) && self@.2 == c.plan@ && matches_context(*staged, *c),
            out.is_some() ==> index < self@.1.len() && index < staged@.0.len()
                && entry(self@.1[index as int], self@.3, staged@.0[index as int]),
            out.is_some() ==> out.unwrap()@ == (self@.1[index as int], staged@.0[index as int]),
    {
        if staged.attempt.as_str() != c.attempt.as_str() || staged.operation.as_str() != c.operation.as_str()
            || staged.authority.as_str() != c.authority.as_str() || staged.plan.as_str() != c.plan.as_str()
            || index >= staged.entries.len() { return None; }

        let backend = self.select(durable, c, index)?;
        if !entry_matches(self, backend.definition(), &staged.entries[index]) { return None; }

        Some(StagedBackend { backend, resolution: &staged.entries[index] })
    }
}

impl StagedGenerations {
    /// Compatibility records are derived from this complete accepted set, never read back for native execution.
    pub fn entries(&self) -> (out: &[Resolution]) ensures out@ == self@.0,
    { &self.entries }
}

/// One backend with its exact checked staging entry; only this type can request store installation.
pub struct StagedBackend<'a> {
    // Original source and durable execution capability.
    backend: Backend<'a>,
    // Original checked selection or explicit unchanged-storage policy.
    resolution: &'a Resolution,
}

impl<'a> View for StagedBackend<'a> {
    type V = (Definition, Resolution);

    closed spec fn view(&self) -> Self::V { (self.backend@, *self.resolution) }
}

impl<'a> StagedBackend<'a> {
    /// Other storage operations retain the original source and durable gate.
    pub fn backend(&self) -> (out: &Backend<'a>) ensures out@ == self@.0,
    { &self.backend }

    /// Borrow the complete authenticated identity, not a separately supplied generation number.
    pub fn selected(&self) -> (out: Option<&SelectedGeneration>)
        ensures match self@.1 { Resolution::Retained(s) => out.is_some() && out.unwrap()@ == s@, _ => out.is_none() },
    { match self.resolution { Resolution::Retained(s) => Some(s), _ => None } }

    /// Preserve the selection through the existing verified store request, with no dispatch-time override.
    pub fn store<'b>(&'b self) -> (out: Option<StoreRequest<'b>>)
        ensures out.is_some() ==> out.unwrap()@.0 == self@.0,
            out.is_some() ==> match self@.1 { Resolution::Retained(s) => out.unwrap()@.1 == Some(s@.generation), _ => out.unwrap()@.1.is_none() },
    {
        let generation = match self.resolution { Resolution::Retained(s) => Some(s.selection().generation), _ => None };
        self.backend.store(generation)
    }
}

}
