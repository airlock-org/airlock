//! Select concrete Qubes configuration requests from an immutable, checked machine plan.
//! Decoding configuration and observing Qubes remain adapters; target and argument
//! selection, scope confinement and the executing-context gate run this verified Rust.
//!
//! ## VM configuration requests
//!
//! The planner records `configure` explicitly because setup reconciles every desired VM,
//! including declarations unchanged since the baseline. The complete approval projection
//! includes these names. Missing legacy fields decode to an empty list; a nonempty desired
//! configuration cannot acquire configuration authority from that empty list.
//!
//! `AuthorizedPlan::configuration` constructs an opaque `ConfigurationPlan` before
//! publication. Its verified constructor accepts exactly the checked VM namespace,
//! rejecting missing, extra and duplicate definitions and substituted helper paths.
//! The authenticated release adapter decodes classes, labels, templates and declared
//! settings. The plan retains those values immutably with the complete plan binding.
//! Every create, property, feature and service request checks the actual executing
//! `DurableOperation`, both publication acknowledgments and the journal's current attempt,
//! operation, authority and plan binding. Requests borrow that gate through dispatch.
//! Their dispatch methods accept no independent mutating VM name or key/value.
//!
//! The executable Verus contracts prove exact Qubes argv, not just target membership.
//! Setting requests also require a power observation naming the retained VM and an
//! explicitly halted classification before a mutation. An already-current value can be
//! left alone while running. Matching NetVM/GuiVM values still require an explicit
//! Qubes override. Observation truth/completeness, XML interpretation, decoding, iteration
//! and absence of a concurrent administrative restart remain adapter obligations.
//!
//! The setup protocol separates global preparation, Rust VM reconciliation and completion.
//! Boot/manual setup uses the same Rust reconciliation with its existing local policy;
//! it cannot reuse a deployment runner to dispatch raw changes. Older admitted setup
//! programs use an explicit legacy batch request after their recognized unsupported-option
//! response; unknown versions and uncertain protocol outcomes are refused. Fixed per-VM
//! backend/device requests bind the target, program and topology; they do **not** verify the native helper's storage,
//! derived boot metadata or device behaviour. Global policy, template import, label and
//! kernel preparation are likewise still trusted batch adapters. Template import can
//! create/start VMs outside the ordinary domain namespace. These are not covered by
//! the common VM configuration request theorem.
//!
//! Setup reports aggregate preparation, per-VM helper and Rust change receipts, preserving
//! restart/unsupported evidence. Run identity, missing/failed child evidence and repeated
//! finalization are checked by ordinary Rust tests. These informational reports are
//! separate from the protected durable journal and grant no authority.

use crate::{
    effects::{self, Targets},
    operation::{DurableOperation, ExecutionContext, PlanBinding},
};
use vstd::prelude::*;

verus! {

/// One declared Qubes setting after wire-value normalization.
pub struct Setting {
    /// Upstream property, feature or service identity.
    pub name: String,
    /// Exact value decoded from the admitted configuration.
    pub value: String,
}

/// VM configuration decoded from the admitted release, before intent publication.
pub struct Definition {
    /// Declared guest identity; decoding validates the Qubes name grammar.
    pub name: String,
    /// Qubes class used only when this VM does not exist.
    pub class: String,
    /// Declared/default label used when creating this VM.
    pub label: String,
    /// Explicit creation template, absent when none is declared.
    pub template: Option<String>,
    /// Properties in their required reconciliation order.
    pub properties: Vec<Setting>,
    /// Exact declared feature values.
    pub features: Vec<Setting>,
    /// Service settings normalized to upstream on/off values.
    pub services: Vec<Setting>,
    /// Admitted setup identity retained for boot/manual and older protocol compatibility.
    pub helper: Option<Helper>,
}

/// Exact admitted helper and immutable topology file, captured before publication.
pub struct Helper {
    /// Program recorded by the admitted candidate, not a caller-selected executable.
    pub program: String,
    /// Candidate topology file authenticated by its retained release fingerprint.
    pub topology: String,
}

/// Compatibility helper modes; native deployment uses typed storage and device requests.
#[derive(Clone, Copy)]
pub enum HelperKind {
    /// Nix storage/backend reconciliation, without VM creation or declared common settings.
    Backend,
}

/// Configuration operation families have distinct fixed Qubes tools.
#[derive(Clone, Copy)]
pub enum Kind {
    /// Existing upstream VM property.
    Property,
    /// Existing upstream VM feature.
    Feature,
    /// Existing upstream service enablement.
    Service,
}

/// Scope, immutable definitions and full candidate/baseline/plan binding.
pub ghost struct ConfigurationState {
    /// Targets admitted by the original complete plan check.
    pub scope: effects::Scope,
    /// Definitions retained without replacement after publication.
    pub definitions: Seq<Definition>,
    /// Admitted setup program and authenticated topology used by batch adapters.
    pub setup: Helper,
    /// Complete original execution-plan digest.
    pub binding: Seq<char>,
}

/// A per-VM adapter retains the same admitted program and topology as its plan.
pub open spec fn same_helper(definition: Definition, setup: Helper) -> bool {
    definition.helper.is_some() && definition.helper.unwrap().program@ == setup.program@
        && definition.helper.unwrap().topology@ == setup.topology@
}

/// Every retained definition belongs to the checked plan and original approval.
pub open spec fn valid(s: ConfigurationState) -> bool {
    &&& effects::valid(s.scope)
    &&& forall|i: int| 0 <= i < s.definitions.len() ==>
        #[trigger] effects::planned(s.scope, s.definitions[i].name@)
            && same_helper(s.definitions[i], s.setup)
}

/// Lift planned membership into original human scope without trusting an adapter flag.
proof fn planned_allowed(s: ConfigurationState, index: int)
    requires valid(s), s.scope.reviewed, 0 <= index < s.definitions.len(),
    ensures s.scope.allowed.contains(s.definitions[index].name@),
{
    let name = s.definitions[index].name@;
    assert(effects::planned(s.scope, name));
    let j = choose|j: int| 0 <= j < s.scope.targets.len() && #[trigger] s.scope.targets[j]@ == name;
}

/// Immutable configuration authority; only AuthorizedPlan can create this value.
pub struct ConfigurationPlan {
    // Original checked reconciliation scope, including its approval provenance.
    scope: Targets,
    // Definitions captured from the admitted release before publication.
    definitions: Vec<Definition>,
    // Exact admitted compatibility program and immutable topology input.
    setup: Helper,
    // Cannot be exchanged for another plan when selecting a request.
    binding: PlanBinding,
}

impl View for ConfigurationPlan {
    type V = ConfigurationState;

    closed spec fn view(&self) -> ConfigurationState {
        ConfigurationState { scope: self.scope@, definitions: self.definitions@, setup: self.setup, binding: self.binding@ }
    }
}

/// Definition membership compares VM names by content.
pub open spec fn defined(defs: Seq<Definition>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < defs.len() && #[trigger] defs[i].name@ == name
}

/// Search the complete definition list rather than trusting caller-provided coverage.
fn has_definition(defs: &[Definition], name: &str) -> (found: bool)
    ensures found == defined(defs@, name@),
{
    let mut i = 0;
    while i < defs.len()
        invariant 0 <= i <= defs.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] defs[j].name@ != name@,
        decreases defs.len() - i,
    {
        if defs[i].name.as_str() == name { return true; }
        i += 1;
    }
    false
}

impl ConfigurationPlan {
    /// Refuse extra, duplicate or missing definitions before any privileged execution.
    pub(crate) fn new(scope: Targets, binding: PlanBinding, definitions: Vec<Definition>, setup: Helper) -> (result: Option<Self>)
        ensures
            result.is_some() ==> result.unwrap()@.binding == binding@,
            result.is_some() ==> result.unwrap()@.scope == scope@,
            result.is_some() ==> result.unwrap()@.definitions == definitions@ && result.unwrap()@.setup == setup,
            result.is_some() && effects::valid(scope@) ==> valid(result.unwrap()@),
            result.is_some() ==> forall|name: Seq<char>| #[trigger] defined(definitions@, name)
                == effects::planned(scope@, name),
    {
        let mut i = 0;
        while i < definitions.len()
            invariant 0 <= i <= definitions.len(),
                forall|j: int| 0 <= j < i ==> #[trigger] effects::planned(scope@, definitions[j].name@),
                forall|j: int| 0 <= j < i ==> #[trigger] same_helper(definitions[j], setup),
            decreases definitions.len() - i,
        {
            if let Some(helper) = &definitions[i].helper {
                if helper.program.as_str() != setup.program.as_str() || helper.topology.as_str() != setup.topology.as_str() { return None; }
            } else { return None; }
            assert(same_helper(definitions[i as int], setup));
            if !scope.contains(definitions[i].name.as_str())
                || has_definition(&definitions.as_slice()[0..i], definitions[i].name.as_str()) { return None; }
            i += 1;
        }

        let names = scope.names();
        let mut j = 0;
        while j < names.len()
            invariant 0 <= j <= names.len(), names@ == scope@.targets,
                forall|i: int| 0 <= i < definitions.len() ==> #[trigger] effects::planned(scope@, definitions[i].name@),
                forall|i: int| 0 <= i < definitions.len() ==> #[trigger] same_helper(definitions[i], setup),
                forall|k: int| 0 <= k < j ==> #[trigger] defined(definitions@, names[k]@),
            decreases names.len() - j,
        {
            if !has_definition(definitions.as_slice(), names[j].as_str()) { return None; }
            j += 1;
        }

        proof {
            assert forall|name: Seq<char>| #[trigger] defined(definitions@, name)
                == effects::planned(scope@, name) by {
                if defined(definitions@, name) {
                    let i = choose|i: int| 0 <= i < definitions.len() && #[trigger] definitions[i].name@ == name;
                } else if effects::planned(scope@, name) {
                    let i = choose|i: int| 0 <= i < names.len() && #[trigger] names[i]@ == name;
                }
            }
        }

        Some(Self { scope, definitions, setup, binding })
    }

    /// Read a retained definition for observation; this grants no mutation permission.
    pub fn definitions(&self) -> (definitions: &[Definition])
        ensures definitions@ == self@.definitions,
    { self.definitions.as_slice() }

    /// Admit a fixed batch phase for the same complete, checked topology namespace.
    pub fn batch<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        phase: SetupPhase) -> (result: Result<BatchRequest<'a>, Denial>)
        ensures result.is_ok() ==> crate::operation::executing(durable, context) && context.plan@ == self@.binding,
            result.is_ok() ==> result.unwrap()@ == (self@, phase),
    {
        if !durable.allows_effect(context) || context.plan.as_str() != self.binding.as_str() { return Err(Denial::Context); }
        Ok(BatchRequest { plan: self, phase, _durable: durable })
    }

    /// VM creation requires a checked definition and the exact executing durable context.
    pub fn create<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        index: usize, exists: bool) -> (result: Result<Option<CreateRequest<'a>>, Denial>)
        ensures
            result.is_ok() ==> crate::operation::executing(durable, context) && context.plan@ == self@.binding,
            result.is_ok() ==> index < self@.definitions.len(),
            result.is_ok() ==> result.unwrap().is_some() == !exists,
            result.is_ok() && result.unwrap().is_some() ==> result.unwrap().unwrap()@ == self@.definitions[index as int],
            result.is_ok() && valid(self@) && self@.scope.reviewed ==>
                self@.scope.allowed.contains(self@.definitions[index as int].name@),
    {
        if !durable.allows_effect(context) || context.plan.as_str() != self.binding.as_str() {
            return Err(Denial::Context);
        }
        if index >= self.definitions.len() { return Err(Denial::Index); }
        proof { if valid(self@) && self@.scope.reviewed { planned_allowed(self@, index as int); } }
        if !self.scope.contains(self.definitions[index].name.as_str()) { return Err(Denial::Context); }
        if exists { return Ok(None); }
        Ok(Some(CreateRequest { definition: &self.definitions[index], _durable: durable }))
    }

    /// Restrict the compatibility helper to one checked VM and a fixed operation mode.
    pub fn helper<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        index: usize, kind: HelperKind) -> (result: Result<HelperRequest<'a>, Denial>)
        ensures result.is_ok() ==> crate::operation::executing(durable, context) && context.plan@ == self@.binding,
            result.is_ok() ==> index < self@.definitions.len(),
            result.is_ok() ==> result.unwrap()@.0 == self@.definitions[index as int],
            result.is_ok() ==> result.unwrap()@.1 == kind,
            result.is_ok() && valid(self@) && self@.scope.reviewed ==>
                self@.scope.allowed.contains(self@.definitions[index as int].name@),
    {
        if !durable.allows_effect(context) || context.plan.as_str() != self.binding.as_str() { return Err(Denial::Context); }
        if index >= self.definitions.len() || self.definitions[index].helper.is_none() { return Err(Denial::Index); }
        proof { if valid(self@) && self@.scope.reviewed { planned_allowed(self@, index as int); } }
        if !self.scope.contains(self.definitions[index].name.as_str()) { return Err(Denial::Context); }
        Ok(HelperRequest { definition: &self.definitions[index], kind, _durable: durable })
    }

    /// Compare observed values, retaining explicit NetVM/GuiVM overrides even when equal.
    pub fn setting<'a>(&'a self, durable: &'a DurableOperation, context: &ExecutionContext,
        selection: Selection, observation: &SettingObservation)
        -> (result: Result<Option<SettingRequest<'a>>, Denial>)
        ensures
            result.is_ok() ==> crate::operation::executing(durable, context) && context.plan@ == self@.binding,
            result.is_ok() ==> selection.domain < self@.definitions.len(),
            result.is_ok() ==> observation.name@ == self@.definitions[selection.domain as int].name@,
            result.is_ok() ==> selection.setting < settings(self@.definitions[selection.domain as int], selection.kind).len(),
            result.is_ok() && result.unwrap().is_some() ==> observation.power@ is Halted,
            result.is_ok() && result.unwrap().is_some() ==> result.unwrap().unwrap()@.target == self@.definitions[selection.domain as int].name@,
            result.is_ok() && result.unwrap().is_some() ==> result.unwrap().unwrap()@.kind == selection.kind,
            result.is_ok() && result.unwrap().is_some() ==> result.unwrap().unwrap()@.setting == settings(self@.definitions[selection.domain as int], selection.kind)[selection.setting as int],
            result.is_ok() && valid(self@) && self@.scope.reviewed ==>
                self@.scope.allowed.contains(self@.definitions[selection.domain as int].name@),
    {
        if !durable.allows_effect(context) || context.plan.as_str() != self.binding.as_str() {
            return Err(Denial::Context);
        }
        if selection.domain >= self.definitions.len() { return Err(Denial::Index); }
        proof { if valid(self@) && self@.scope.reviewed { planned_allowed(self@, selection.domain as int); } }
        let domain = &self.definitions[selection.domain];
        if !self.scope.contains(domain.name.as_str()) { return Err(Denial::Context); }
        if observation.name != domain.name.as_str() { return Err(Denial::Observation); }
        let values = match selection.kind { Kind::Property => &domain.properties,
            Kind::Feature => &domain.features, Kind::Service => &domain.services };
        if selection.setting >= values.len() { return Err(Denial::Index); }
        let setting = &values[selection.setting];
        let inherited = matches!(selection.kind, Kind::Property)
            && (setting.name.as_str() == "netvm" || setting.name.as_str() == "guivm") && !observation.explicit;
        if !inherited && value_matches(setting.value.as_str(), observation.current) { return Ok(None); }
        if !observation.power.is_halted() { return Err(Denial::Running); }
        Ok(Some(SettingRequest { target: domain.name.as_str(), setting, kind: selection.kind, _durable: durable }))
    }
}

/// Invalid indices or stale/nonexecuting authority never mean successful convergence.
#[derive(Debug)]
pub enum Denial {
    /// Observation names another VM and cannot justify this request.
    Observation,
    /// A required mutation must wait for an observed halted VM.
    Running,
    /// Attempt, operation, authority, plan or durable state differs.
    Context,
    /// A requested definition or setting is absent from the retained plan.
    Index,
}

/// Qubes evidence stays associated with the exact VM whose setting is requested.
pub struct SettingObservation<'a> {
    /// VM name in the observed power row, not an independently asserted target.
    pub name: &'a str,
    /// Current queried value; an unset feature may have no value.
    pub current: Option<&'a str>,
    /// Whether a matching connection value is an explicit override in Qubes XML.
    pub explicit: bool,
    /// Verified classification of that VM's decoded raw STATE field.
    pub power: crate::inventory::PowerState,
}

/// Setting coordinates are selectors, never unchecked command arguments.
pub struct Selection {
    /// Retained definition index.
    pub domain: usize,
    /// The fixed tool family.
    pub kind: Kind,
    /// Retained setting index in that family's list.
    pub setting: usize,
}

/// Mathematical setting family in a decoded VM definition.
pub open spec fn settings(d: Definition, kind: Kind) -> Seq<Setting> {
    match kind { Kind::Property => d.properties@, Kind::Feature => d.features@, Kind::Service => d.services@ }
}

/// Exact equality, with upstream spellings of explicitly disabled references equivalent.
pub open spec fn equal_value(desired: Seq<char>, current: Option<&str>) -> bool {
    match current { Some(value) => desired == value@
        || (desired == "none"@ && (value@ == ""@ || value@ == "None"@)), None => false }
}

/// Compare observation contents without making failed or missing observations authority.
pub fn value_matches(desired: &str, current: Option<&str>) -> (equal: bool)
    ensures equal == equal_value(desired@, current),
{
    match current {
        Some(value) => {
            proof { reveal_strlit(""); if value@.len() == 0 { assert(value@ =~= ""@); } }
            desired == value || (desired == "none" && (value.is_empty() || value == "None"))
        },
        None => false,
    }
}

/// Creation borrows its exact definition and durable gate until the adapter consumes it.
/// Callers cannot fabricate this permission from an ordinary decoded definition.
///
/// ```compile_fail
/// use airlock_deployment_core::{configuration::{CreateRequest, Definition}, operation::DurableOperation};
/// /// Attempt direct construction without the checked configuration selector.
/// fn bypass<'a>(definition: &'a Definition, durable: &'a DurableOperation) -> CreateRequest<'a> {
///     CreateRequest { definition, _durable: durable }
/// }
/// ```
pub struct CreateRequest<'a> {
    // Definition selected from the immutable checked configuration.
    definition: &'a Definition,
    // Prevents interruption/completion while the request is outstanding.
    _durable: &'a DurableOperation,
}

impl<'a> View for CreateRequest<'a> {
    type V = Definition;

    closed spec fn view(&self) -> Definition { *self.definition }
}

impl<'a> CreateRequest<'a> {
    /// Construct the exact upstream create argv, including only the declared template.
    pub fn arguments(&self) -> (args: Vec<String>)
        ensures args.len() == if self@.template.is_some() { 7usize } else { 5usize },
            args[0]@ == "--class"@ && args[1]@ == self@.class@
                && args[2]@ == "--label"@ && args[3]@ == self@.label@,
            self@.template.is_some() ==> args[4]@ == "--template"@ && args[5]@ == self@.template.unwrap()@ && args[6]@ == self@.name@,
            self@.template.is_none() ==> args[4]@ == self@.name@,
    { create_arguments(self.definition) }
}

/// Encode creation from the same definition for deployment and explicit boot/manual setup.
pub fn create_arguments(definition: &Definition) -> (args: Vec<String>)
    ensures args.len() == if definition.template.is_some() { 7usize } else { 5usize },
        args[0]@ == "--class"@ && args[1]@ == definition.class@
            && args[2]@ == "--label"@ && args[3]@ == definition.label@,
        definition.template.is_some() ==> args[4]@ == "--template"@ && args[5]@ == definition.template.unwrap()@ && args[6]@ == definition.name@,
        definition.template.is_none() ==> args[4]@ == definition.name@,
{
    let mut args = vec!["--class".to_string(), definition.class.as_str().to_string(),
        "--label".to_string(), definition.label.as_str().to_string()];
    if let Some(template) = &definition.template {
        args.push("--template".to_string());
        args.push(template.as_str().to_string());
    }
    args.push(definition.name.as_str().to_string());
    args
}

/// Exact target, family and setting passed to a Qubes mutation adapter.
pub ghost struct SettingState {
    /// Sole VM positional target.
    pub target: Seq<char>,
    /// Tool family selected by the plan.
    pub kind: Kind,
    /// Exact declared key/value, without independent adapter parameters.
    pub setting: Setting,
}

/// A setting request cannot replace its target, key or value after validation.
pub struct SettingRequest<'a> {
    // Name borrowed from the retained definition.
    target: &'a str,
    // Key and value borrowed from the selected setting list.
    setting: &'a Setting,
    // Determines the sole permitted upstream executable.
    kind: Kind,
    // Borrows the executing gate through dispatch.
    _durable: &'a DurableOperation,
}

impl<'a> View for SettingRequest<'a> {
    type V = SettingState;

    closed spec fn view(&self) -> SettingState {
        SettingState { target: self.target@, kind: self.kind, setting: *self.setting }
    }
}

impl<'a> SettingRequest<'a> {
    /// Choose the fixed upstream executable from the retained setting family.
    pub fn program(&self) -> (program: &str)
        ensures program@ == match self@.kind { Kind::Property => "qvm-prefs"@,
            Kind::Feature => "qvm-features"@, Kind::Service => "qvm-service"@ },
    {
        match self.kind { Kind::Property => "qvm-prefs", Kind::Feature => "qvm-features", Kind::Service => "qvm-service" }
    }

    /// Pass exactly the checked VM, declared key and declared value to the adapter.
    pub fn arguments(&self) -> (args: Vec<String>)
        ensures args.len() == 3 && args[0]@ == self@.target
            && args[1]@ == self@.setting.name@ && args[2]@ == self@.setting.value@,
    {
        vec![self.target.to_string(), self.setting.name.as_str().to_string(),
            self.setting.value.as_str().to_string()]
    }
}

/// Compatibility helper request constrained to one definition and exact admitted paths.
pub struct HelperRequest<'a> {
    // Immutable selected definition, including the admitted helper identities.
    definition: &'a Definition,
    // Fixed operation mode; global/full-topology mode is unavailable here.
    kind: HelperKind,
    // Keeps the deployment executing through helper dispatch.
    _durable: &'a DurableOperation,
}

impl<'a> View for HelperRequest<'a> {
    type V = (Definition, HelperKind);

    closed spec fn view(&self) -> Self::V { (*self.definition, self.kind) }
}

impl<'a> HelperRequest<'a> {
    /// Borrow the exact admitted executable; absence yields no executable.
    pub fn program(&self) -> (program: Option<&str>)
        ensures program.is_some() == self@.0.helper.is_some(),
            program.is_some() ==> program.unwrap()@ == self@.0.helper.unwrap().program@,
    {
        self.definition.helper.as_ref().map(|helper: &Helper| -> (s: &str)
            ensures s@ == helper.program@,
        { helper.program.as_str() })
    }

    /// Construct a fixed, single-VM helper invocation from the selected definition.
    pub fn arguments(&self) -> (args: Vec<String>)
        ensures self@.0.helper.is_some() ==> args.len() == 5
            && args[0]@ == "--apply"@ && args[1]@ == "--topology"@
            && args[2]@ == self@.0.helper.unwrap().topology@
            && args[3]@ == match self@.1 { HelperKind::Backend => "--backend-only"@ }
            && args[4]@ == self@.0.name@,
    {
        let mut args = Vec::new();
        if let Some(helper) = &self.definition.helper {
            args.push("--apply".to_string()); args.push("--topology".to_string());
            args.push(helper.topology.as_str().to_string());
            args.push(match self.kind { HelperKind::Backend => "--backend-only".to_string() });
            args.push(self.definition.name.as_str().to_string());
        }
        args
    }
}

/// Fixed setup phases retain compatibility with older admitted release helpers.
#[derive(Clone, Copy)]
pub enum SetupPhase {
    /// Read-only capability query; an unsupported old helper can return an error.
    Protocol,
    /// Global preparation before VM reconciliation.
    Prepare,
    /// Domain-backed defaults and completion receipts after VM reconciliation.
    Finish,
    /// Explicit legacy batch adapter; all declared VM reconciliation targets are checked.
    Legacy,
}

/// A batch helper request covers the complete checked configuration namespace.
pub struct BatchRequest<'a> {
    // Immutable definitions, admitted helper paths and checked VM scope.
    plan: &'a ConfigurationPlan,
    // Fixed entry point, rather than arbitrary independently supplied arguments.
    phase: SetupPhase,
    // Keeps the operation executing throughout the delegated external adapter.
    _durable: &'a DurableOperation,
}

impl<'a> View for BatchRequest<'a> {
    type V = (ConfigurationState, SetupPhase);

    closed spec fn view(&self) -> Self::V { (self.plan@, self.phase) }
}

impl<'a> BatchRequest<'a> {
    /// Select exactly the admitted executable captured with this plan.
    pub fn program(&self) -> (program: &str)
        ensures program@ == self@.0.setup.program@,
    { self.plan.setup.program.as_str() }

    /// Encode only the fixed protocol/global/legacy entry modes for this topology.
    pub fn arguments(&self) -> (args: Vec<String>)
        ensures self@.1 is Protocol ==> args.len() == 1 && args[0]@ == "--protocol"@,
            !(self@.1 is Protocol) ==> args.len() == (if self@.1 is Legacy { 3usize } else { 4usize })
                && args[0]@ == "--apply"@ && args[1]@ == "--topology"@ && args[2]@ == self@.0.setup.topology@,
            self@.1 is Prepare ==> args[3]@ == "--prepare-only"@,
            self@.1 is Finish ==> args[3]@ == "--finish-only"@,
    {
        let mut args = Vec::new();
        if matches!(self.phase, SetupPhase::Protocol) {
            args.push("--protocol".to_string());
        } else {
            args.push("--apply".to_string()); args.push("--topology".to_string());
            args.push(self.plan.setup.topology.as_str().to_string());
            match self.phase { SetupPhase::Prepare => args.push("--prepare-only".to_string()),
                SetupPhase::Finish => args.push("--finish-only".to_string()), _ => {} }
        }
        args
    }
}

}
