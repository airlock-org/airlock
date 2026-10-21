//! Preserve an authenticated generation through setup and retained-only storage requests.
//! Wire decoding and decimal rendering belong to thin adapters; no request can choose admission.

#[cfg(verus_keep_ghost)]
use super::same;

use super::{Selection, matching};
use crate::attachment::runtime_namespace;
use crate::storage::{Power, RootArtifact};
use vstd::prelude::*;

verus! {

/// A positive generation whose complete boot identity matched committed storage evidence.
/// This proves compatibility, not human approval or filesystem authenticity.
///
/// ```compile_fail
/// use airlock_deployment_core::boot::{SelectedGeneration, Selection};
/// /// Direct construction would skip validation and is therefore forbidden.
/// fn bypass_check(selection: Selection) -> SelectedGeneration {
///     SelectedGeneration { selection }
/// }
/// ```
pub struct SelectedGeneration {
    // Exact requested identity retained after comparison with committed evidence.
    selection: Selection,
}

impl View for SelectedGeneration {
    type V = Selection;

    closed spec fn view(&self) -> Self::V { self.selection }
}

impl SelectedGeneration {
    /// Reject incomplete, zero or substituted generation evidence.
    pub fn check(wanted: Selection, committed: &Selection) -> (out: Option<Self>)
        ensures out.is_some() == (wanted.generation > 0 && same(wanted, *committed)),
            out.is_some() ==> same(out.unwrap()@, wanted) && out.unwrap()@.generation > 0,
    {
        if wanted.generation == 0 || !matching(&wanted, committed) { return None; }
        Some(Self { selection: wanted })
    }

    /// Borrow the original identity; there is no mutable accessor or public constructor.
    pub fn selection(&self) -> (out: &Selection)
        ensures *out == self@,
    { &self.selection }

    /// Bind execution to the same effective topology after crossing a setup boundary.
    pub fn setup<'a>(&'a self, wanted: &Selection) -> (out: Option<SetupRequest<'a>>)
        ensures out.is_some() == (self@.generation > 0 && same(self@, *wanted)),
            out.is_some() ==> out.unwrap()@ == self@,
    {
        if self.selection.generation == 0 || !matching(&self.selection, wanted) { return None; }
        Some(SetupRequest { selected: self })
    }
}

/// One process argument, keeping generation identity numeric until the final adapter.
pub enum Argument {
    /// Literal protocol flag or an exact selected identity.
    Text(
        /// Unchanged flag or selected identity.
        String,
    ),
    /// Positive selected generation, rendered as decimal only by the process adapter.
    Generation(
        /// Exact generation before decimal encoding.
        u64,
    ),
}

/// Argument contains the specified literal or selected string.
pub open spec fn text(arg: Argument, value: Seq<char>) -> bool {
    arg is Text && arg->Text_0@ == value
}

/// Exact retained-only storage invocation; all identities come from one selection.
pub open spec fn store_arguments(s: Selection, args: Seq<Argument>) -> bool {
    args.len() == 25
        && text(args[0], "storage"@) && text(args[1], "domain"@)
        && text(args[2], "ensure-store"@) && text(args[3], s.domain@)
        && text(args[4], s.source@) && text(args[5], s.source@)
        && text(args[6], "--mode"@) && text(args[7], s.mode@)
        && text(args[8], "--system"@) && text(args[9], s.system@)
        && text(args[10], "--root"@) && text(args[11], s.root@)
        && text(args[12], "--kernel"@) && text(args[13], s.kernel@)
        && text(args[14], "--device"@) && text(args[15], s.device@)
        && text(args[16], "--blob-device"@) && text(args[17], s.blob_device@)
        && text(args[18], "--mountpoint"@) && text(args[19], s.mountpoint@)
        && text(args[20], "--view-backend"@) && text(args[21], s.backend@)
        && text(args[22], "--generation"@) && args[23] is Generation
        && args[23]->Generation_0 == s.generation && text(args[24], "--require-retained"@)
}

/// A backend execution request that preserves the checked selection and cannot request fresh admission.
pub struct SetupRequest<'a> {
    // Borrow prevents changing the selected identities during request dispatch.
    selected: &'a SelectedGeneration,
}

impl<'a> View for SetupRequest<'a> {
    type V = Selection;

    closed spec fn view(&self) -> Self::V { self.selected@ }
}

fn literal(value: &str) -> (out: Argument)
    ensures text(out, value@),
{ Argument::Text(value.to_string()) }

impl<'a> SetupRequest<'a> {
    /// Borrow only the identity matched by this setup request.
    pub fn selection(&self) -> (out: &Selection)
        ensures *out == self@,
    { &self.selected.selection }

    /// Dispatch must retain every boot field and explicitly require the original generation.
    pub fn store_arguments(&self) -> (out: Vec<Argument>)
        ensures store_arguments(self@, out@),
    {
        let s = &self.selected.selection;
        vec![literal("storage"), literal("domain"), literal("ensure-store"), literal(s.domain.as_str()),
            literal(s.source.as_str()), literal(s.source.as_str()), literal("--mode"), literal(s.mode.as_str()),
            literal("--system"), literal(s.system.as_str()), literal("--root"), literal(s.root.as_str()),
            literal("--kernel"), literal(s.kernel.as_str()), literal("--device"), literal(s.device.as_str()),
            literal("--blob-device"), literal(s.blob_device.as_str()), literal("--mountpoint"), literal(s.mountpoint.as_str()),
            literal("--view-backend"), literal(s.backend.as_str()), literal("--generation"),
            Argument::Generation(s.generation), literal("--require-retained")]
    }

    /// Resolve the root from this generation only, never by an unscoped source lookup.
    pub fn root_arguments(&self) -> (out: Vec<Argument>)
        ensures out.len() == 9 && text(out[0], "storage"@) && text(out[1], "pool"@)
            && text(out[2], "resolve-root"@) && text(out[3], "--domain"@)
            && text(out[4], self@.domain@ + "-runtime-store"@) && text(out[5], "--root-source"@)
            && text(out[6], self@.root@) && text(out[7], "--generation"@)
            && out[8] is Generation && out[8]->Generation_0 == self@.generation,
    {
        let s = &self.selected.selection;
        let namespace = runtime_namespace(s.domain.as_str());
        vec![literal("storage"), literal("pool"), literal("resolve-root"), literal("--domain"),
            literal(namespace.as_str()), literal("--root-source"), literal(s.root.as_str()),
            literal("--generation"), Argument::Generation(s.generation)]
    }
}

/// Boot identities which must never be supplied independently by the setup adapter.
#[derive(Clone, Copy)]
pub enum BootField {
    /// Qubes kernel property.
    Kernel,
    /// Whole-system identity used by readiness checks.
    System,
    /// Original root seed identity.
    Root,
    /// Shared software source identity.
    Store,
}

/// Fixed property or feature key for one selected boot identity.
pub open spec fn key(field: BootField) -> Seq<char> {
    match field { BootField::Kernel => "kernel"@, BootField::System => "airlock-system-source"@,
        BootField::Root => "airlock-root-source"@, BootField::Store => "airlock-store-source"@ }
}

/// Original selected value of a boot property or feature.
pub open spec fn value(s: Selection, field: BootField) -> Seq<char> {
    match field { BootField::Kernel => s.kernel@, BootField::System => s.system@,
        BootField::Root => s.root@, BootField::Store => s.source@ }
}

impl<'a> SetupRequest<'a> {
    /// Select the fixed upstream interface for this boot field.
    pub fn setting_program(&self, field: BootField) -> (out: &str)
        ensures out@ == match field { BootField::Kernel => "qvm-prefs"@,
            BootField::System | BootField::Root | BootField::Store => "qvm-features"@ },
    {
        match field { BootField::Kernel => "qvm-prefs",
            BootField::System | BootField::Root | BootField::Store => "qvm-features" }
    }

    /// Construct only the fixed VM/key/value tuple for this selected boot field.
    pub fn setting_arguments(&self, field: BootField) -> (out: Vec<String>)
        ensures out.len() == 3 && out[0]@ == self@.domain@
            && out[1]@ == key(field) && out[2]@ == value(self@, field),
    {
        let s = &self.selected.selection;
        let (key, value) = match field {
            BootField::Kernel => ("kernel", s.kernel.as_str()),
            BootField::System => ("airlock-system-source", s.system.as_str()),
            BootField::Root => ("airlock-root-source", s.root.as_str()),
            BootField::Store => ("airlock-store-source", s.source.as_str()),
        };
        vec![s.domain.as_str().to_string(), key.to_string(), value.to_string()]
    }

    /// Root import requires a named halted VM and evidence for this exact selected root.
    pub fn root<'b>(&'b self, artifact: &'b RootArtifact, power: &Power) -> (out: Option<RootRequest<'b>>)
        ensures out.is_some() == (power.name@ == self@.domain@ && power.state@ is Halted
            && artifact.name@ == self@.domain@ && artifact.source@ == self@.root@
            && artifact.image@.len() > 0 && artifact.hash@.len() > 0),
            out.is_some() ==> out.unwrap()@ == (self@, *artifact),
    {
        let s = &self.selected.selection;
        if power.name != s.domain || !power.state.is_halted() || artifact.name != s.domain
            || artifact.source != s.root || artifact.image.is_empty() || artifact.hash.is_empty() { return None; }
        Some(RootRequest { selected: self.selected, artifact })
    }
}

/// Root import bound to selected generation evidence; external observation and artifact authentication remain trusted.
pub struct RootRequest<'a> {
    // Selected VM and root source retained until dispatch.
    selected: &'a SelectedGeneration,
    // Authenticated result of the generation-specific root resolver.
    artifact: &'a RootArtifact,
}

impl<'a> View for RootRequest<'a> {
    type V = (Selection, RootArtifact);

    closed spec fn view(&self) -> Self::V { (self.selected@, *self.artifact) }
}

impl<'a> RootRequest<'a> {
    /// Select exactly this VM's root volume and resolved image.
    pub fn arguments(&self) -> (out: Vec<String>)
        ensures out.len() == 3 && out[0]@ == "import"@
            && out[1]@ == self@.0.domain@ + ":root"@ && out[2]@ == self@.1.image@,
    {
        let mut volume = self.selected.selection.domain.as_str().to_string();
        volume.push_str(":root");
        vec!["import".to_string(), volume, self.artifact.image.as_str().to_string()]
    }
}

}
