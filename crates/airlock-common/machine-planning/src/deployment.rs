//! Select deployment roots and reconnectable GUI clients from decoded configuration.
//! Configuration strings are canonical values, not hashes or caller-selected change flags.
//! Reference interpretation and named graph execution share the existing verified kernel.

use crate::names::owned_contains;
use crate::topology::{self, Declaration, Defaults, InvalidTopology, NamedGraph, Reference};
use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// Interpret a stable deployment-planning observation; unsupported states require a new snapshot.
/// Paused and transitional domains cannot safely be treated as stopped or as restartable roots.
pub fn running(state: &str) -> (result: Option<bool>)
    ensures
        result.is_some() == (state@ == "Running"@ || state@ == "Halted"@),
        result == Some(true) <==> state@ == "Running"@,
        result == Some(false) <==> state@ == "Halted"@,
{
    if state == "Running" { Some(true) }
    else if state == "Halted" { Some(false) }
    else { None }
}

/// One VM's configuration values before and after deployment, keyed by exact identity.
pub struct Change {
    /// VM identity shared with the decoded relationship declarations.
    pub name: String,
    /// Canonical baseline domain value; absent only for a newly declared VM.
    pub before: Option<String>,
    /// Canonical candidate domain value; absent only for a removed VM.
    pub after: Option<String>,
    /// Candidate autostart boolean; absent and non-boolean values decode to None.
    pub autostart: Option<bool>,
    /// Baseline reconnect feature string; only the exact value "1" opts in.
    pub reconnect: Option<String>,
}

/// Equality of complete configuration values, preserving absence as a distinct case.
pub open spec fn same(before: Option<String>, after: Option<String>) -> bool {
    match (before, after) {
        (Some(a), Some(b)) => a@ == b@,
        (None, None) => true,
        _ => false,
    }
}

/// A candidate declaration needs configuration when it is new, changed, or defaults changed.
pub open spec fn changed(c: Change, globals_changed: bool) -> bool {
    c.after.is_some() && (globals_changed || !same(c.before, c.after))
}

/// Removal is an impact seed, independently of retirement authorization.
pub open spec fn removed(c: Change) -> bool { c.before.is_some() && c.after.is_none() }

/// Only candidate VMs already running or explicitly autostarting are startup roots.
pub open spec fn startup(c: Change, running: Seq<String>) -> bool {
    c.after.is_some() && (crate::names::owned_member(running, c.name@) || c.autostart == Some(true))
}

/// A non-display provider relationship, including device backends.
pub open spec fn hard(d: Declaration, defaults: Defaults, target: Seq<char>) -> bool {
    (exists|k: int| 0 <= k < 6 && k != 2
        && #[trigger] topology::provider(topology::effective(topology::setting(d, k), topology::fallback(defaults, k)), d.name@, target))
    || (exists|k: int| 0 <= k < d.devices.len()
        && topology::provider(#[trigger] d.devices[k], d.name@, target))
}

/// GUI opt-in preserves an unchanged client, never the GUI provider itself.
/// Hard dependents remain subject to ordinary affected-VM propagation.
pub open spec fn eligible(c: Change, d: Declaration, defaults: Defaults, gui_present: bool) -> bool {
    &&& c.name@ == d.name@
    &&& c.name@ != "sys-gui"@
    &&& c.before.is_some() && c.after.is_some() && same(c.before, c.after)
    &&& match c.reconnect { Some(value) => value@ == "1"@, None => false }
    &&& topology::provider(topology::effective(d.guivm, defaults.guivm), d.name@, "sys-gui"@)
    &&& gui_present
    &&& !hard(d, defaults, "sys-gui"@)
}

/// Compare canonical values rather than trusting a caller's changed/unchanged flag.
fn equal(before: &Option<String>, after: &Option<String>) -> (equal: bool)
    ensures equal == same(*before, *after),
{
    match (before, after) {
        (Some(a), Some(b)) => a.as_str() == b.as_str(),
        (None, None) => true,
        _ => false,
    }
}

/// Check every non-display reference, retaining the interpreter's malformed-value rejection.
fn hard_dependency(d: &Declaration, defaults: &Defaults, target: &str)
    -> (result: Result<bool, InvalidTopology>)
    ensures result.is_ok() ==> result.unwrap() == hard(*d, *defaults, target@),
{
    let mut found = false;
    let mut k = 0;
    while k < 6
        invariant 0 <= k <= 6,
            found == (exists|j: int| 0 <= j < k && j != 2
                && #[trigger] topology::provider(topology::effective(topology::setting(*d, j), topology::fallback(*defaults, j)), d.name@, target@)),
        decreases 6 - k,
    {
        if k != 2 {
            let provider = topology::resolve(d.setting(k), defaults.setting(k), d.name.as_str())?;
            if let Some(name) = provider { found = found || name.as_str() == target; }
        }
        k += 1;
    }

    let mut k = 0;
    while k < d.devices.len()
        invariant 0 <= k <= d.devices.len(),
            found == ((exists|j: int| 0 <= j < 6 && j != 2
                && #[trigger] topology::provider(topology::effective(topology::setting(*d, j), topology::fallback(*defaults, j)), d.name@, target@))
                || (exists|j: int| 0 <= j < k && topology::provider(#[trigger] d.devices[j], d.name@, target@))),
        decreases d.devices.len() - k,
    {
        let provider = topology::resolve(&d.devices[k], &Reference::Disabled, d.name.as_str())?;
        if let Some(name) = provider { found = found || name.as_str() == target; }
        k += 1;
    }
    Ok(found)
}

/// Decide GUI eligibility with exact configuration, opt-in, provider and hard-edge checks.
pub fn gui_eligible(c: &Change, d: &Declaration, defaults: &Defaults, gui_present: bool)
    -> (result: Result<bool, InvalidTopology>)
    ensures result.is_ok() ==> result.unwrap() == eligible(*c, *d, *defaults, gui_present),
{
    // Validate all hard relationships even for clients which do not opt in.
    let hard = hard_dependency(d, defaults, "sys-gui")?;
    if c.name.as_str() != d.name.as_str() || c.name.as_str() == "sys-gui"
        || c.before.is_none() || c.after.is_none() || !equal(&c.before, &c.after)
        || !gui_present || hard { return Ok(false); }
    let reconnect = match &c.reconnect { Some(value) => value.as_str() == "1", None => false };
    if !reconnect { return Ok(false); }

    match topology::resolve(&d.guivm, &defaults.guivm, d.name.as_str())? {
        Some(provider) => Ok(provider.as_str() == "sys-gui"),
        None => Ok(false),
    }
}

/// Initial selections; construction and every copied identity are verified together.
/// These sets propose effects and grant no execution authority.
struct Seeds {
    // Exact changed/new candidate declarations.
    changed: Vec<String>,
    // Exact removed baseline declarations.
    removed: Vec<String>,
    // Union of changed and removed declarations before impact expansion.
    impact: Vec<String>,
    // Candidate running/autostart roots before dependency expansion.
    startup: Vec<String>,
    // Eligible clients whose soft display edge can be omitted for impact only.
    omit_display: Vec<String>,
}

/// Named root sets exposed to contracts without exposing mutable execution fields.
pub ghost struct Selection {
    /// Exact changed candidate names.
    pub changed: Seq<String>,
    /// Exact removed baseline names.
    pub removed: Seq<String>,
    /// Initial affected names before reverse closure.
    pub impact: Seq<String>,
    /// Initial startup names before forward closure.
    pub startup: Seq<String>,
    /// Eligible GUI clients before affected-set filtering.
    pub omit_display: Seq<String>,
}

impl View for Seeds {
    type V = Selection;

    closed spec fn view(&self) -> Selection {
        Selection { changed: self.changed@, removed: self.removed@, impact: self.impact@,
            startup: self.startup@, omit_display: self.omit_display@ }
    }
}

/// Select exactly the rows justified by one lifecycle rule, copying their own names.
fn select_kind(rows: &[Change], globals: bool, running: &[String], kind: usize) -> (names: Vec<String>)
    ensures forall|target: Seq<char>| crate::names::owned_member(names@, target)
        == (exists|j: int| 0 <= j < rows.len() && rows[j].name@ == target
            && #[trigger] reason(rows[j], globals, running@, kind as int)),
{
    let mut names: Vec<String> = Vec::new();
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(),
            forall|target: Seq<char>| crate::names::owned_member(names@, target)
                == (exists|j: int| 0 <= j < i && rows[j].name@ == target
                    && #[trigger] reason(rows[j], globals, running@, kind as int)),
        decreases rows.len() - i,
    {
        let c = &rows[i];
        let changed = c.after.is_some() && (globals || !equal(&c.before, &c.after));
        let removed = c.before.is_some() && c.after.is_none();
        let startup = c.after.is_some()
            && (owned_contains(running, c.name.as_str()) || c.autostart == Some(true));
        let selected = if kind == 0 { changed } else if kind == 1 { removed }
            else if kind == 2 { changed || removed } else { startup };
        let ghost previous = names@;
        if selected {
            let name = c.name.as_str().to_string();
            proof { topology::member_push(names@, name); }
            names.push(name);
        }
        assert forall|target: Seq<char>| crate::names::owned_member(names@, target)
            == (exists|j: int| 0 <= j < i + 1 && rows[j].name@ == target
                && #[trigger] reason(rows[j], globals, running@, kind as int)) by {
            if exists|j: int| 0 <= j < i + 1 && rows[j].name@ == target && reason(rows[j], globals, running@, kind as int) {
                let j = choose|j: int| 0 <= j < i + 1 && rows[j].name@ == target && reason(rows[j], globals, running@, kind as int);
                if j < i { assert(crate::names::owned_member(previous, target)); }
            }
        }
        i += 1;
    }
    names
}

/// A name qualifies through its own baseline declaration and its own configuration row.
pub open spec fn gui_through(rows: Seq<Change>, before: Seq<Declaration>, defaults: Defaults,
    gui_present: bool, limit: int, name: Seq<char>) -> bool {
    exists|i: int, j: int| 0 <= i < limit && i < rows.len() && 0 <= j < before.len()
        && rows[i].name@ == name && before[j].name@ == name
        && #[trigger] eligible(rows[i], before[j], defaults, gui_present)
}

/// Full GUI eligibility set, defined independently of the selection loop.
pub open spec fn gui_reason(rows: Seq<Change>, before: Seq<Declaration>, defaults: Defaults,
    gui_present: bool, name: Seq<char>) -> bool {
    gui_through(rows, before, defaults, gui_present, rows.len() as int, name)
}

/// Named declarations must be unique so a lookup cannot conceal conflicting relationships.
pub open spec fn unique(before: Seq<Declaration>) -> bool {
    forall|i: int, j: int| 0 <= i < j < before.len() ==> #[trigger] before[i].name@ != #[trigger] before[j].name@
}

/// Select every eligible GUI client without allowing index/name substitution.
fn select_gui(rows: &[Change], before: &[Declaration], defaults: &Defaults, gui_present: bool)
    -> (result: Result<Vec<String>, InvalidTopology>)
    requires unique(before@),
    ensures result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@, target)
        == gui_reason(rows@, before@, *defaults, gui_present, target),
{
    let mut names: Vec<String> = Vec::new();
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(), unique(before@),
            forall|target: Seq<char>| crate::names::owned_member(names@, target)
                == gui_through(rows@, before@, *defaults, gui_present, i as int, target),
        decreases rows.len() - i,
    {
        let c = &rows[i];
        let at = topology::lookup_domain(before, c.name.as_str());
        let accepted = if let Some(j) = at { gui_eligible(c, &before[j], defaults, gui_present)? } else { false };
        assert(accepted == (exists|j: int| 0 <= j < before.len() && before[j].name@ == c.name@
            && #[trigger] eligible(*c, before[j], *defaults, gui_present))) by {
            if at.is_some() {
                let j = at.unwrap();
                assert forall|k: int| 0 <= k < before.len() && before[k].name@ == c.name@
                    implies k == j by {
                    if k < j { assert(before[k].name@ != before[j as int].name@); }
                    if j < k { assert(before[j as int].name@ != before[k].name@); }
                }
            }
        }
        let ghost previous = names@;
        if accepted {
            let name = c.name.as_str().to_string();
            proof { topology::member_push(names@, name); }
            names.push(name);
        }
        assert forall|target: Seq<char>| crate::names::owned_member(names@, target)
            == gui_through(rows@, before@, *defaults, gui_present, i + 1, target) by {
            if gui_through(rows@, before@, *defaults, gui_present, i + 1, target) {
                let a = choose|a: int| 0 <= a < i + 1 && rows[a].name@ == target
                    && exists|b: int| 0 <= b < before.len() && before[b].name@ == target
                        && #[trigger] eligible(rows[a], before[b], *defaults, gui_present);
                if a < i { assert(crate::names::owned_member(previous, target)); }
            }
            if crate::names::owned_member(previous, target) {
                let a = choose|a: int| 0 <= a < i && rows[a].name@ == target
                    && exists|b: int| 0 <= b < before.len() && before[b].name@ == target
                        && #[trigger] eligible(rows[a], before[b], *defaults, gui_present);
                assert(gui_through(rows@, before@, *defaults, gui_present, i + 1, target));
            }
        }
        i += 1;
    }
    Ok(names)
}

/// Presence of a configuration value under its own VM identity.
pub open spec fn row_present(rows: Seq<Change>, name: Seq<char>) -> bool {
    exists|j: int| 0 <= j < rows.len() && #[trigger] rows[j].name@ == name
}

/// Mathematical input coverage, independent of collection positions.
pub open spec fn covered(rows: Seq<Change>, before: Seq<Declaration>, after: Seq<Declaration>) -> bool {
    &&& forall|i: int| 0 <= i < rows.len() ==> topology::guest(#[trigger] rows[i].name@)
        && (rows[i].before.is_some() || rows[i].after.is_some())
        && (rows[i].before.is_some() == topology::declared(before, rows[i].name@))
        && (rows[i].after.is_some() == topology::declared(after, rows[i].name@))
    &&& forall|i: int, j: int| 0 <= i < j < rows.len() ==> #[trigger] rows[i].name@ != #[trigger] rows[j].name@
    &&& forall|i: int| 0 <= i < before.len() ==> #[trigger] row_present(rows, before[i].name@)
    &&& forall|i: int| 0 <= i < after.len() ==> #[trigger] row_present(rows, after[i].name@)
}

/// Locate a configuration row by name rather than accepting an externally supplied index.
fn lookup(rows: &[Change], name: &str) -> (found: bool)
    ensures found == row_present(rows@, name@),
{
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(), forall|j: int| 0 <= j < i ==> #[trigger] rows[j].name@ != name@,
        decreases rows.len() - i,
    {
        if rows[i].name.as_str() == name { return true; }
        i += 1;
    }
    false
}

/// Reject ambiguous configuration identities before evaluating per-row presence.
fn unique_rows(rows: &[Change]) -> (accepted: bool)
    ensures accepted == (forall|i: int, j: int| 0 <= i < j < rows.len() ==> #[trigger] rows[i].name@ != #[trigger] rows[j].name@),
{
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(),
            forall|a: int, b: int| 0 <= a < i && a < b < rows.len() ==> #[trigger] rows[a].name@ != #[trigger] rows[b].name@,
        decreases rows.len() - i,
    {
        let mut j = i + 1;
        while j < rows.len()
            invariant 0 <= i < rows.len(), i < j <= rows.len(),
                forall|b: int| i < b < j ==> rows[i as int].name@ != #[trigger] rows[b].name@,
            decreases rows.len() - j,
        {
            if rows[i].name.as_str() == rows[j].name.as_str() { return false; }
            j += 1;
        }
        i += 1;
    }
    true
}

/// Reject missing, duplicate, phantom or presence-inconsistent configuration identities.
fn check(rows: &[Change], before: &[Declaration], after: &[Declaration]) -> (accepted: bool)
    ensures accepted == covered(rows@, before@, after@),
{
    if !unique_rows(rows) { return false; }
    let mut i = 0;
    while i < rows.len()
        invariant 0 <= i <= rows.len(),
            forall|a: int| 0 <= a < i ==> topology::guest(#[trigger] rows[a].name@)
                && (rows[a].before.is_some() || rows[a].after.is_some())
                && (rows[a].before.is_some() == topology::declared(before@, rows[a].name@))
                && (rows[a].after.is_some() == topology::declared(after@, rows[a].name@)),
            forall|a: int, b: int| 0 <= a < b < rows.len() ==> #[trigger] rows[a].name@ != #[trigger] rows[b].name@,
        decreases rows.len() - i,
    {
        let row = &rows[i];
        if !topology::valid_guest(row.name.as_str()) || (row.before.is_none() && row.after.is_none())
            || row.before.is_some() != topology::lookup_domain(before, row.name.as_str()).is_some()
            || row.after.is_some() != topology::lookup_domain(after, row.name.as_str()).is_some() { return false; }
        i += 1;
    }

    let mut i = 0;
    while i < before.len()
        invariant 0 <= i <= before.len(),
            forall|a: int| 0 <= a < rows.len() ==> topology::guest(#[trigger] rows[a].name@)
                && (rows[a].before.is_some() || rows[a].after.is_some())
                && (rows[a].before.is_some() == topology::declared(before@, rows[a].name@))
                && (rows[a].after.is_some() == topology::declared(after@, rows[a].name@)),
            forall|a: int, b: int| 0 <= a < b < rows.len() ==> #[trigger] rows[a].name@ != #[trigger] rows[b].name@,
            forall|a: int| 0 <= a < i ==> #[trigger] row_present(rows@, before[a].name@),
        decreases before.len() - i,
    {
        if !lookup(rows, before[i].name.as_str()) { return false; }
        i += 1;
    }
    let mut i = 0;
    while i < after.len()
        invariant 0 <= i <= after.len(),
            forall|a: int| 0 <= a < rows.len() ==> topology::guest(#[trigger] rows[a].name@)
                && (rows[a].before.is_some() || rows[a].after.is_some())
                && (rows[a].before.is_some() == topology::declared(before@, rows[a].name@))
                && (rows[a].after.is_some() == topology::declared(after@, rows[a].name@)),
            forall|a: int, b: int| 0 <= a < b < rows.len() ==> #[trigger] rows[a].name@ != #[trigger] rows[b].name@,
            forall|a: int| 0 <= a < before.len() ==> #[trigger] row_present(rows@, before[a].name@),
            forall|a: int| 0 <= a < i ==> #[trigger] row_present(rows@, after[a].name@),
        decreases after.len() - i,
    {
        if !lookup(rows, after[i].name.as_str()) { return false; }
        i += 1;
    }
    true
}

/// Exact selection reason, defined independently of the iteration and output representation.
pub open spec fn reason(c: Change, globals: bool, running: Seq<String>, kind: int) -> bool {
    if kind == 0 { changed(c, globals) } else if kind == 1 { removed(c) }
    else if kind == 2 { changed(c, globals) || removed(c) } else { startup(c, running) }
}

/// Membership in a declarative root set; independent of the executable output vector.
pub open spec fn root_rule(rows: Seq<Change>, globals: bool, running: Seq<String>, kind: int, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < rows.len() && rows[i].name@ == name && #[trigger] reason(rows[i], globals, running, kind)
}

impl Seeds {
    /// Check identity coverage and derive lifecycle roots and soft-display eligibility.
    fn new(rows: &[Change], before: &[Declaration], after: &[Declaration], defaults: &Defaults,
        before_globals: &str, after_globals: &str, running: &[String]) -> (result: Result<Self, InvalidTopology>)
        requires unique(before@),
        ensures
            result.is_ok() ==> covered(rows@, before@, after@),
            result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@.changed, target)
                == (exists|j: int| 0 <= j < rows.len() && rows[j].name@ == target && reason(rows[j], before_globals@ != after_globals@, running@, 0)),
            result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@.removed, target)
                == (exists|j: int| 0 <= j < rows.len() && rows[j].name@ == target && reason(rows[j], before_globals@ != after_globals@, running@, 1)),
            result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@.impact, target)
                == (exists|j: int| 0 <= j < rows.len() && rows[j].name@ == target && reason(rows[j], before_globals@ != after_globals@, running@, 2)),
            result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@.startup, target)
                == (exists|j: int| 0 <= j < rows.len() && rows[j].name@ == target && reason(rows[j], before_globals@ != after_globals@, running@, 3)),
            result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@.omit_display, target)
                == gui_reason(rows@, before@, *defaults, topology::declared(after@, "sys-gui"@), target),
    {
        if !check(rows, before, after) { return Err(InvalidTopology::Identity); }
        let globals = before_globals != after_globals;
        let changed = select_kind(rows, globals, running, 0);
        let removed = select_kind(rows, globals, running, 1);
        let impact = select_kind(rows, globals, running, 2);
        let startup = select_kind(rows, globals, running, 3);
        let gui_present = topology::lookup_domain(after, "sys-gui").is_some();
        let omit_display = select_gui(rows, before, defaults, gui_present)?;
        Ok(Self { changed, removed, impact, startup, omit_display })
    }

    /// Borrow changed declarations without permitting callers to fabricate validated seeds.
    fn changed(&self) -> (names: &[String]) ensures names@ == self@.changed, { &self.changed }

    /// Borrow removed declarations; explicit retirement permission is checked separately.
    fn removed(&self) -> (names: &[String]) ensures names@ == self@.removed, { &self.removed }

}

/// Root membership agrees exactly with independent change, startup and GUI predicates.
pub open spec fn correct(s: Selection, rows: Seq<Change>, before: Seq<Declaration>,
    after: Seq<Declaration>, defaults: Defaults, globals: bool, running: Seq<String>) -> bool {
    &&& covered(rows, before, after)
    &&& forall|name: Seq<char>| crate::names::owned_member(s.changed, name)
        == root_rule(rows, globals, running, 0, name)
    &&& forall|name: Seq<char>| crate::names::owned_member(s.removed, name)
        == root_rule(rows, globals, running, 1, name)
    &&& forall|name: Seq<char>| crate::names::owned_member(s.impact, name)
        == (crate::names::owned_member(s.changed, name) || crate::names::owned_member(s.removed, name))
    &&& forall|name: Seq<char>| crate::names::owned_member(s.startup, name)
        == root_rule(rows, globals, running, 3, name)
    &&& forall|name: Seq<char>| crate::names::owned_member(s.omit_display, name)
        == gui_reason(rows, before, defaults, topology::declared(after, "sys-gui"@), name)
}

/// Equivalent decoded row values, independent of their position or String allocation.
pub open spec fn same_change(a: Change, b: Change) -> bool {
    a.name@ == b.name@ && same(a.before, b.before) && same(a.after, b.after)
        && a.autostart == b.autostart && same(a.reconnect, b.reconnect)
}

/// Every effective configuration row occurs in the other input, irrespective of iteration order.
pub open spec fn rows_in(left: Seq<Change>, right: Seq<Change>) -> bool {
    forall|i: int| #![trigger left[i]] 0 <= i < left.len() ==> exists|j: int| 0 <= j < right.len()
        && #[trigger] same_change(left[i], right[j])
}

/// Equivalent per-name inputs have the same declarative selection rules.
proof fn same_rules(a: Change, b: Change, globals: bool, left: Seq<String>, right: Seq<String>,
    d: Declaration, defaults: Defaults, gui_present: bool)
    requires same_change(a, b),
        forall|name: Seq<char>| crate::names::owned_member(left, name) == crate::names::owned_member(right, name),
    ensures
        forall|kind: int| #[trigger] reason(a, globals, left, kind) == reason(b, globals, right, kind),
        eligible(a, d, defaults, gui_present) == eligible(b, d, defaults, gui_present),
{
}

/// Iteration order and running-set enumeration cannot change the selected VM sets.
/// Complete plan serialization and external snapshot accuracy are separate properties.
pub proof fn deterministic(a: Selection, b: Selection, left: Seq<Change>, right: Seq<Change>,
    before: Seq<Declaration>, after: Seq<Declaration>, defaults: Defaults, globals: bool,
    running_left: Seq<String>, running_right: Seq<String>)
    requires
        correct(a, left, before, after, defaults, globals, running_left),
        correct(b, right, before, after, defaults, globals, running_right),
        rows_in(left, right), rows_in(right, left),
        forall|name: Seq<char>| crate::names::owned_member(running_left, name) == crate::names::owned_member(running_right, name),
    ensures forall|name: Seq<char>|
        #[trigger] crate::names::owned_member(a.changed, name) == crate::names::owned_member(b.changed, name)
        && crate::names::owned_member(a.removed, name) == crate::names::owned_member(b.removed, name)
        && crate::names::owned_member(a.impact, name) == crate::names::owned_member(b.impact, name)
        && crate::names::owned_member(a.startup, name) == crate::names::owned_member(b.startup, name)
        && crate::names::owned_member(a.omit_display, name) == crate::names::owned_member(b.omit_display, name),
{
    rules_in(left, right, before, defaults, topology::declared(after, "sys-gui"@), globals, running_left, running_right);
    rules_in(right, left, before, defaults, topology::declared(after, "sys-gui"@), globals, running_right, running_left);
}

/// Transport a policy witness through an equivalent named configuration row.
proof fn rules_in(left: Seq<Change>, right: Seq<Change>, before: Seq<Declaration>, defaults: Defaults,
    gui_present: bool, globals: bool, running_left: Seq<String>, running_right: Seq<String>)
    requires rows_in(left, right),
        forall|name: Seq<char>| crate::names::owned_member(running_left, name) == crate::names::owned_member(running_right, name),
    ensures
        forall|name: Seq<char>, kind: int| #[trigger] root_rule(left, globals, running_left, kind, name)
            ==> root_rule(right, globals, running_right, kind, name),
        forall|name: Seq<char>| #[trigger] gui_reason(left, before, defaults, gui_present, name)
            ==> gui_reason(right, before, defaults, gui_present, name),
{
    assert forall|i: int, kind: int| 0 <= i < left.len() && #[trigger] reason(left[i], globals, running_left, kind)
        implies exists|j: int| 0 <= j < right.len() && right[j].name@ == left[i].name@
            && #[trigger] reason(right[j], globals, running_right, kind) by {
        let j = choose|j: int| 0 <= j < right.len() && #[trigger] same_change(left[i], right[j]);
        assert(same(left[i].before, left[i].after) == same(right[j].before, right[j].after));
        assert(reason(left[i], globals, running_left, kind) == reason(right[j], globals, running_right, kind));
    }
    assert forall|name: Seq<char>| #[trigger] gui_reason(left, before, defaults, gui_present, name)
        implies gui_reason(right, before, defaults, gui_present, name) by {
        assert(gui_through(left, before, defaults, gui_present, left.len() as int, name));
        let i = choose|i: int| 0 <= i < left.len() && left[i].name@ == name
            && exists|d: int| 0 <= d < before.len() && before[d].name@ == name
                && #[trigger] eligible(left[i], before[d], defaults, gui_present);
        let d = choose|d: int| 0 <= d < before.len() && before[d].name@ == name
            && #[trigger] eligible(left[i], before[d], defaults, gui_present);
        let j = choose|j: int| 0 <= j < right.len() && #[trigger] same_change(left[i], right[j]);
        same_rules(left[i], right[j], globals, running_left, running_right, before[d], defaults, gui_present);
        assert(eligible(right[j], before[d], defaults, gui_present));
    }
}

/// Only the display participation bit differs from the original declaration.
pub open spec fn projection(d: Declaration, omit: bool) -> Declaration {
    Declaration { omit_display: omit, ..d }
}

/// Apply checked GUI eligibility without modifying any identity or other reference.
fn project(domains: Vec<Declaration>, omissions: &[String]) -> (projected: Vec<Declaration>)
    ensures projected.len() == domains.len(),
        forall|i: int| 0 <= i < projected.len() ==> #[trigger] projected[i]
            == projection(domains[i], crate::names::owned_member(omissions@, domains[i].name@)),
{
    let ghost original = domains@;
    let mut projected = domains;
    let mut i = 0;
    while i < projected.len()
        invariant projected.len() == original.len(), 0 <= i <= projected.len(),
            forall|j: int| 0 <= j < i ==> #[trigger] projected[j]
                == projection(original[j], crate::names::owned_member(omissions@, original[j].name@)),
            forall|j: int| i <= j < projected.len() ==> #[trigger] projected[j] == original[j],
        decreases projected.len() - i,
    {
        let omit = owned_contains(omissions, projected[i].name.as_str());
        projected[i].omit_display = omit;
        i += 1;
    }
    projected
}

/// Decoded graph inputs and the exact machine-wide values which affect change selection.
pub struct Configuration {
    /// Proposed lexical identities, checked against the declarations by NamedGraph.
    pub names: Vec<String>,
    /// Complete decoded references with no caller-selected GUI omissions.
    pub domains: Vec<Declaration>,
    /// Effective host reference defaults retained for verified interpretation.
    pub defaults: Defaults,
    /// Canonical pair of kernel definitions and host preferences; other global fields do not seed restarts.
    pub globals: String,
}

/// Reject pre-applied display omissions before interpreting original topology inputs.
fn unprojected(domains: &[Declaration]) -> (accepted: bool)
    ensures accepted == (forall|i: int| 0 <= i < domains.len() ==> !#[trigger] domains[i].omit_display),
{
    let mut i = 0;
    while i < domains.len()
        invariant 0 <= i <= domains.len(), forall|j: int| 0 <= j < i ==> !#[trigger] domains[j].omit_display,
        decreases domains.len() - i,
    {
        if domains[i].omit_display { return false; }
        i += 1;
    }
    true
}

/// Checked roots and their graphs cannot be replaced independently after construction.
/// Methods always pass these exact names to the existing verified named planner.
///
/// ```compile_fail
/// use airlock_machine_planning::deployment::SeedPlan;
/// fn expand_authority(plan: &mut SeedPlan) {
///     plan.seeds.startup.push("unrelated-vm".into());
/// }
/// ```
pub struct SeedPlan {
    // Immutable root sets derived from configuration values.
    seeds: Seeds,
    // Proof-only provenance for the cached closure; no runtime graph is needed after expansion.
    _impact: Ghost<topology::NamedState>,
    // Full baseline graph, retaining display edges for shutdown ordering.
    shutdown: NamedGraph,
    // Full candidate graph used for dependency-expanded startup.
    startup: NamedGraph,
    // Reverse closure computed once from the checked roots and immutable impact graph.
    affected: Vec<String>,
    // Exact caller-supplied Running set, retained for shutdown and reconnect selection.
    running: Vec<String>,
}

/// Contract view of one immutable planning boundary.
pub ghost struct PlanningState {
    /// Checked initial VM sets.
    pub seeds: Selection,
    /// Impact graph with verified display omission.
    pub impact: topology::NamedState,
    /// Baseline graph for shutdown ordering.
    pub shutdown: topology::NamedState,
    /// Candidate graph for startup ordering.
    pub startup: topology::NamedState,
    /// Cached named impact closure, tied to the graph by the type invariant.
    pub affected: Seq<String>,
    /// Explicit running-VM observation.
    pub running: Seq<String>,
}

impl View for SeedPlan {
    type V = PlanningState;

    closed spec fn view(&self) -> PlanningState {
        PlanningState { seeds: self.seeds@, impact: self._impact@,
            shutdown: self.shutdown@, startup: self.startup@, affected: self.affected@, running: self.running@ }
    }
}

/// Named impact closure of exactly this plan's selected roots.
pub open spec fn impacted(s: PlanningState, affected: Seq<String>) -> bool {
    exists|mask: Seq<bool>| mask.len() == s.impact.names.len()
        && crate::graph::closure(s.impact.graph,
            crate::graph::selected(topology::membership(s.impact.names, s.seeds.impact)),
            crate::graph::selected(mask), true)
        && forall|name: Seq<char>| crate::names::owned_member(affected, name) == topology::masked(s.impact.names, mask, name)
}

impl SeedPlan {
    /// Retain the exact cached closure through every later lifecycle selection.
    #[verifier::type_invariant]
    closed spec fn well_formed(self) -> bool { impacted(self@, self@.affected) }

    /// Bind complete configuration rows to baseline/candidate graphs and derive all roots.
    /// Proposed name tables are checked by NamedGraph, not trusted as index assignments.
    pub fn new(before: Configuration, after: Configuration, impact_names: Vec<String>,
        rows: &[Change], running: Vec<String>) -> (result: Result<Self, InvalidTopology>)
        ensures result.is_ok() ==> correct(result.unwrap()@.seeds, rows@, before.domains@, after.domains@,
                before.defaults, before.globals@ != after.globals@, running@)
            && result.unwrap()@.running == running@
            && result.unwrap()@.shutdown.origin.domains == before.domains@
            && result.unwrap()@.shutdown.origin.defaults == before.defaults
            && result.unwrap()@.startup.origin.domains == after.domains@
            && result.unwrap()@.startup.origin.defaults == after.defaults
            && result.unwrap()@.impact.origin.defaults == before.defaults
            && result.unwrap()@.impact.origin.extra == result.unwrap()@.seeds.impact
            && result.unwrap()@.impact.origin.domains.len() == before.domains.len()
            && (forall|i: int| 0 <= i < before.domains.len() ==> #[trigger] result.unwrap()@.impact.origin.domains[i]
                == projection(before.domains[i], crate::names::owned_member(result.unwrap()@.seeds.omit_display, before.domains[i].name@))),
    {
        if !unprojected(&before.domains) || !unprojected(&after.domains) { return Err(InvalidTopology::Identity); }
        let shutdown = NamedGraph::new(before.names, &before.domains, &before.defaults, &[])?;
        let startup = NamedGraph::new(after.names, &after.domains, &after.defaults, &[])?;
        let seeds = Seeds::new(rows, &before.domains, &after.domains, &before.defaults, before.globals.as_str(), after.globals.as_str(), &running)?;
        assert forall|name: Seq<char>| crate::names::owned_member(seeds@.impact, name)
            == (crate::names::owned_member(seeds@.changed, name) || crate::names::owned_member(seeds@.removed, name)) by {
            if crate::names::owned_member(seeds@.impact, name) {
                let j = choose|j: int| 0 <= j < rows.len() && rows[j].name@ == name
                    && (changed(rows[j], before.globals@ != after.globals@) || removed(rows[j]));
            }
            if crate::names::owned_member(seeds@.changed, name) {
                let j = choose|j: int| 0 <= j < rows.len() && rows[j].name@ == name
                    && changed(rows[j], before.globals@ != after.globals@);
            }
            if crate::names::owned_member(seeds@.removed, name) {
                let j = choose|j: int| 0 <= j < rows.len() && rows[j].name@ == name && #[trigger] removed(rows[j]);
            }
        }
        let projected = project(before.domains, &seeds.omit_display);
        let impact = NamedGraph::new(impact_names, &projected, &before.defaults, &seeds.impact)?;
        assert(seeds@.impact == seeds.impact@);
        let affected = impact.expand(&seeds.impact, true);
        let ghost mask = choose|mask: Seq<bool>| mask.len() == impact@.names.len()
            && crate::graph::closure(impact@.graph,
                crate::graph::selected(topology::membership(impact@.names, seeds.impact@)),
                crate::graph::selected(mask), true)
            && forall|name: Seq<char>| crate::names::owned_member(affected@, name) == topology::masked(impact@.names, mask, name);
        let ghost impact_state = impact@;
        let ghost state = PlanningState { seeds: seeds@, impact: impact_state, shutdown: shutdown@,
            startup: startup@, affected: affected@, running: running@ };
        assert(state.seeds.impact == seeds.impact@);
        assert(state.impact == impact@);
        assert(mask.len() == state.impact.names.len());
        assert(crate::graph::closure(state.impact.graph,
            crate::graph::selected(topology::membership(state.impact.names, state.seeds.impact)),
            crate::graph::selected(mask), true));
        assert(impacted(state, affected@));
        let result = Self { seeds, _impact: Ghost(impact_state), shutdown, startup, affected, running };
        assert(impacted(result@, result@.affected));
        Ok(result)
    }

    /// Borrow the checked change set for staging and plan metadata.
    pub fn changed(&self) -> (names: &[String]) ensures names@ == self@.seeds.changed, { self.seeds.changed() }

    /// Borrow the checked removal set for existing retirement policy.
    pub fn removed(&self) -> (names: &[String]) ensures names@ == self@.seeds.removed, { self.seeds.removed() }

    /// Expand exactly the selected impact roots through the checked baseline projection.
    pub fn affected(&self) -> (names: &[String])
        ensures names@ == self@.affected, impacted(self@, names@),
    {
        proof { use_type_invariant(self); }
        self.affected.as_slice()
    }

    /// Start exactly the selected roots and their candidate dependencies in dependency order.
    pub fn start(&self) -> (result: Option<Vec<String>>)
        ensures result.is_none() == !(exists|order: Seq<usize>| crate::graph::full_order(self@.startup.graph, order)),
            result.is_some() ==> exists|vertices: Seq<usize>|
                crate::graph::closure(self@.startup.graph,
                    crate::graph::selected(topology::membership(self@.startup.names, self@.seeds.startup)),
                    crate::graph::listed(vertices), false)
                && vertices.no_duplicates() && crate::graph::respects(self@.startup.graph, vertices, false)
                && #[trigger] crate::graph::names_match(self@.startup.names, vertices, result.unwrap()@),
    { self.startup.startup(&self.seeds.startup) }

    /// Stop affected running VMs using full baseline reverse ordering.
    pub fn stop(&self) -> (result: Option<Vec<String>>)
        ensures result.is_none() == !(exists|order: Seq<usize>| crate::graph::full_order(self@.shutdown.graph, order)),
            result.is_some() ==> exists|affected: Seq<String>, vertices: Seq<usize>|
                #[trigger] impacted(self@, affected)
                && vertices.no_duplicates() && crate::graph::respects(self@.shutdown.graph, vertices, true)
                && (forall|v: usize| v < self@.shutdown.names.len() ==> #[trigger] vertices.contains(v)
                    == (crate::names::owned_member(affected, self@.shutdown.names[v as int]@)
                        && crate::names::owned_member(self@.running, self@.shutdown.names[v as int]@)))
                && #[trigger] crate::graph::names_match(self@.shutdown.names, vertices, result.unwrap()@),
    {
        let affected = self.affected();
        self.shutdown.shutdown(affected, &self.running)
    }

    /// Reconnect exactly the running eligible clients outside the impact closure when GUI is affected.
    /// This proposes GUI interruption effects; the existing approval gate checks them separately.
    pub fn reconnect(&self) -> (names: Vec<String>)
        ensures exists|affected: Seq<String>| impacted(self@, affected)
            && forall|name: Seq<char>| crate::names::owned_member(names@, name)
                == (crate::names::owned_member(affected, "sys-gui"@)
                    && crate::names::owned_member(self@.running, name)
                    && crate::names::owned_member(self@.seeds.omit_display, name)
                    && !crate::names::owned_member(affected, name)),
    {
        let affected = self.affected();
        let mut names: Vec<String> = Vec::new();
        let gui_affected = owned_contains(affected, "sys-gui");
        let mut i = 0;
        while i < self.running.len()
            invariant 0 <= i <= self.running.len(), impacted(self@, affected@),
                gui_affected == crate::names::owned_member(affected@, "sys-gui"@),
                forall|name: Seq<char>| crate::names::owned_member(names@, name)
                    == (gui_affected && crate::names::owned_member(self@.seeds.omit_display, name)
                        && !crate::names::owned_member(affected@, name)
                        && exists|j: int| 0 <= j < i && #[trigger] self.running[j]@ == name),
            decreases self.running.len() - i,
        {
            let ghost previous = names@;
            if gui_affected && owned_contains(&self.seeds.omit_display, self.running[i].as_str())
                && !owned_contains(affected, self.running[i].as_str()) {
                let name = self.running[i].as_str().to_string();
                proof { topology::member_push(names@, name); }
                names.push(name);
            }
            assert forall|name: Seq<char>| crate::names::owned_member(names@, name)
                == (gui_affected && crate::names::owned_member(self@.seeds.omit_display, name)
                    && !crate::names::owned_member(affected@, name)
                    && exists|j: int| 0 <= j < i + 1 && #[trigger] self.running[j]@ == name) by {
                if exists|j: int| 0 <= j < i + 1 && self.running[j]@ == name {
                    let j = choose|j: int| 0 <= j < i + 1 && self.running[j]@ == name;
                    if j < i && gui_affected && crate::names::owned_member(self@.seeds.omit_display, name)
                        && !crate::names::owned_member(affected@, name) {
                        assert(crate::names::owned_member(previous, name));
                    }
                }
            }
            i += 1;
        }
        names
    }
}

}
