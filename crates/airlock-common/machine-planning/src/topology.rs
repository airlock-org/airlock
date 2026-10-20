//! Interpret decoded machine references and preserve VM identities across graph planning.
//! JSON/Rusix decoding supplies explicit reference variants, never resolved indices.
//! Checked canonical tables and opaque graphs prevent name/index/mask substitutions.

use crate::graph::{self, Edge, Graph, GraphState};
use vstd::prelude::*;
use vstd::string::StrSliceExecFns;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// A decoded setting, preserving omission separately from an explicit reset.
pub enum Reference {
    /// Inherit the corresponding host default.
    Omitted,
    /// An explicit null; no provider is required.
    Disabled,
    /// Exact wire spelling, including empty strings and dom0 before interpretation.
    Named(
        /// Provider spelling before empty, dom0, self and guest-name checks.
        String,
    ),
    /// A wire value with an unsupported type or malformed device assignment container.
    Invalid,
    /// A device class was present but did not contain an assignment array.
    InvalidAssignments,
}

/// Host defaults for the six dependency-bearing VM settings.
pub struct Defaults {
    /// Network provider inherited by an omitted netvm.
    pub netvm: Reference,
    /// Template inherited by an omitted template.
    pub template: Reference,
    /// Display provider inherited by an omitted guivm.
    pub guivm: Reference,
    /// Audio provider inherited by an omitted audiovm.
    pub audiovm: Reference,
    /// Both dispvm and default_dispvm inherit this single Qubes host preference.
    pub dispvm: Reference,
}

/// Dependency-bearing fields of one decoded VM declaration.
pub struct Declaration {
    /// Declared VM identity, validated again by the graph constructor.
    pub name: String,
    /// Network override, retaining omission and reset semantics.
    pub netvm: Reference,
    /// Template override, retaining omission and reset semantics.
    pub template: Reference,
    /// Display override; ignored only under the caller's separate GUI-preservation policy.
    pub guivm: Reference,
    /// Audio override.
    pub audiovm: Reference,
    /// Disposable-VM override.
    pub dispvm: Reference,
    /// Default disposable-VM override; shares the host default with dispvm.
    pub default_dispvm: Reference,
    /// Backend references from all decoded device assignments; they never inherit a default.
    pub devices: Vec<Reference>,
    /// Caller-selected omission of display relationships, not permission to execute an effect.
    pub omit_display: bool,
}

/// Decoded topology cannot be interpreted without changing its meaning.
#[derive(Debug)]
pub enum InvalidTopology {
    /// An effective reference has an invalid type or VM spelling.
    Reference,
    /// A device class does not contain the required assignment array.
    DeviceAssignments,
    /// Declarations or the proposed canonical name table are invalid or inconsistent.
    Identity,
}

/// ASCII letter permitted at the beginning of a Qubes name.
pub open spec fn letter(c: char) -> bool {
    ('A' <= c && c <= 'Z') || ('a' <= c && c <= 'z')
}

/// Qubes guest identity grammar, excluding the privileged dom0 domain.
pub open spec fn guest(name: Seq<char>) -> bool {
    1 <= name.len() <= 31 && letter(name[0]) && name != "dom0"@
        && forall|i: int| 0 <= i < name.len() ==> letter(#[trigger] name[i])
            || ('0' <= name[i] && name[i] <= '9') || name[i] == '_' || name[i] == '.' || name[i] == '-'
}

/// Use the integer range contract for character comparisons supported by the pinned prover.
fn in_range(c: char, lower: char, upper: char) -> (inside: bool)
    ensures inside == (lower <= c && c <= upper),
{
    let code = c as u32;
    ((lower as u32)..=(upper as u32)).contains(&code)
}

/// Validate spelling at the verified boundary as well as the ordinary typed wire boundary.
pub fn valid_guest(name: &str) -> (valid: bool)
    ensures valid == guest(name@),
{
    let length = name.unicode_len();
    if length == 0 || length > 31 || name == "dom0" { return false; }
    let first = name.get_char(0);
    if !(in_range(first, 'A', 'Z') || in_range(first, 'a', 'z')) { return false; }
    let mut i = 0;
    while i < length
        invariant
            length == name@.len(), 1 <= length <= 31, name@ != "dom0"@, letter(name@[0]),
            0 <= i <= length,
            forall|j: int| 0 <= j < i ==> letter(#[trigger] name@[j])
                || ('0' <= name@[j] && name@[j] <= '9') || name@[j] == '_' || name@[j] == '.' || name@[j] == '-',
        decreases length - i,
    {
        let c = name.get_char(i);
        if !(in_range(c, 'A', 'Z') || in_range(c, 'a', 'z') || in_range(c, '0', '9')
            || c == '_' || c == '.' || c == '-') { return false; }
        i += 1;
    }
    true
}

/// Only an omitted setting inherits; explicit null, empty or malformed overrides do not.
pub open spec fn effective(local: Reference, default: Reference) -> Reference {
    if local is Omitted { default } else { local }
}

/// Effective values which can be interpreted without inventing a provider.
pub open spec fn reference_valid(value: Reference, owner: Seq<char>) -> bool {
    match value {
        Reference::Invalid | Reference::InvalidAssignments => false,
        Reference::Named(name) => name@ == ""@ || name@ == "dom0"@ || name@ == owner || guest(name@),
        _ => true,
    }
}

/// A required provider is a nonempty, non-dom0, non-self effective reference.
pub open spec fn provider(value: Reference, owner: Seq<char>, target: Seq<char>) -> bool {
    match value {
        Reference::Named(name) => name@ != ""@ && name@ != "dom0"@ && name@ != owner && name@ == target,
        _ => false,
    }
}

/// Resolve one setting, rejecting malformed effective values while preserving exact identity.
pub fn resolve(local: &Reference, default: &Reference, owner: &str)
    -> (result: Result<Option<String>, InvalidTopology>)
    ensures
        result.is_ok() == reference_valid(effective(*local, *default), owner@),
        result.is_ok() ==> match result.unwrap() {
            Some(name) => guest(name@) && provider(effective(*local, *default), owner@, name@),
            None => forall|target: Seq<char>| !provider(effective(*local, *default), owner@, target),
        },
        result.is_ok() ==> forall|target: Seq<char>| provider(effective(*local, *default), owner@, target)
            == (result.unwrap().is_some() && result.unwrap().unwrap()@ == target),
{
    let value = if matches!(local, Reference::Omitted) { default } else { local };
    match value {
        Reference::Invalid => Err(InvalidTopology::Reference),
        Reference::InvalidAssignments => Err(InvalidTopology::DeviceAssignments),
        Reference::Named(name) => {
            if name.as_str() == "" || name.as_str() == "dom0" || name.as_str() == owner { return Ok(None); }
            if !valid_guest(name.as_str()) { return Err(InvalidTopology::Reference); }
            Ok(Some(name.as_str().to_string()))
        },
        _ => Ok(None),
    }
}

/// The six dependency settings have a fixed semantic position, independent of map iteration.
pub open spec fn setting(d: Declaration, index: int) -> Reference {
    if index == 0 { d.netvm } else if index == 1 { d.template } else if index == 2 { d.guivm }
    else if index == 3 { d.audiovm } else if index == 4 { d.dispvm } else { d.default_dispvm }
}

/// Both disposable settings inherit the same host preference.
pub open spec fn fallback(d: Defaults, index: int) -> Reference {
    if index == 0 { d.netvm } else if index == 1 { d.template } else if index == 2 { d.guivm }
    else if index == 3 { d.audiovm } else { d.dispvm }
}

impl Declaration {
    /// Select the actual setting corresponding to the specification's dependency kind.
    pub(crate) fn setting(&self, index: usize) -> (value: &Reference)
        requires index < 6,
        ensures *value == setting(*self, index as int),
    {
        match index { 0 => &self.netvm, 1 => &self.template, 2 => &self.guivm,
            3 => &self.audiovm, 4 => &self.dispvm, _ => &self.default_dispvm }
    }
}

impl Defaults {
    /// Select the host default, including the shared disposable-VM default.
    pub(crate) fn setting(&self, index: usize) -> (value: &Reference)
        requires index < 6,
        ensures *value == fallback(*self, index as int),
    {
        match index { 0 => &self.netvm, 1 => &self.template, 2 => &self.guivm,
            3 => &self.audiovm, _ => &self.dispvm }
    }
}

/// Whether a setting participates in the caller's selected relationship projection.
pub open spec fn enabled(d: Declaration, index: int) -> bool { index != 2 || !d.omit_display }

/// Interpretable active settings among the first kinds and device records.
pub open spec fn valid_through(d: Declaration, defaults: Defaults, kinds: int, devices: int) -> bool {
    &&& forall|k: int| 0 <= k < kinds && enabled(d, k) ==>
        #[trigger] reference_valid(effective(setting(d, k), fallback(defaults, k)), d.name@)
    &&& forall|k: int| 0 <= k < devices ==> reference_valid(#[trigger] d.devices[k], d.name@)
}

/// Required providers among the first setting kinds and device records.
pub open spec fn depends_through(d: Declaration, defaults: Defaults, kinds: int, devices: int, target: Seq<char>) -> bool {
    (exists|k: int| 0 <= k < kinds && enabled(d, k)
        && #[trigger] provider(effective(setting(d, k), fallback(defaults, k)), d.name@, target))
    || (exists|k: int| 0 <= k < devices && provider(#[trigger] d.devices[k], d.name@, target))
}

/// Independent reference semantics: the union of effective property providers and device backends.
pub open spec fn depends(d: Declaration, defaults: Defaults, target: Seq<char>) -> bool {
    depends_through(d, defaults, 6, d.devices.len() as int, target)
}

/// All participating values, rather than unused host defaults, must be valid.
pub open spec fn interpretable(d: Declaration, defaults: Defaults) -> bool {
    valid_through(d, defaults, 6, d.devices.len() as int)
}

/// One additional setting contributes exactly its effective provider, when enabled.
proof fn kind_step(d: Declaration, defaults: Defaults, k: int)
    requires 0 <= k < 6,
    ensures forall|target: Seq<char>| depends_through(d, defaults, k + 1, 0, target)
        == (depends_through(d, defaults, k, 0, target)
            || (enabled(d, k) && #[trigger] provider(effective(setting(d, k), fallback(defaults, k)), d.name@, target))),
{
    assert forall|target: Seq<char>| depends_through(d, defaults, k + 1, 0, target)
        == (depends_through(d, defaults, k, 0, target)
            || (enabled(d, k) && #[trigger] provider(effective(setting(d, k), fallback(defaults, k)), d.name@, target))) by {
        if depends_through(d, defaults, k + 1, 0, target) {
            let at = choose|at: int| 0 <= at < k + 1 && enabled(d, at)
                && provider(effective(setting(d, at), fallback(defaults, at)), d.name@, target);
            if at < k { assert(depends_through(d, defaults, k, 0, target)); }
        }
    }
}

/// One additional assignment contributes its backend, without inheriting any default.
proof fn device_step(d: Declaration, defaults: Defaults, k: int)
    requires 0 <= k < d.devices.len(),
    ensures forall|target: Seq<char>| depends_through(d, defaults, 6, k + 1, target)
        == (depends_through(d, defaults, 6, k, target) || provider(d.devices[k], d.name@, target)),
{
    assert forall|target: Seq<char>| depends_through(d, defaults, 6, k + 1, target)
        == (depends_through(d, defaults, 6, k, target) || provider(d.devices[k], d.name@, target)) by {
        if depends_through(d, defaults, 6, k + 1, target) && !depends_through(d, defaults, 6, k, target) {
            let at = choose|at: int| 0 <= at < k + 1 && provider(d.devices[at], d.name@, target);
            assert(at == k);
        }
    }
}

/// Adding a resolved provider changes name membership by precisely that spelling.
pub(crate) proof fn member_push(names: Seq<String>, name: String)
    ensures forall|target: Seq<char>| #[trigger] crate::names::owned_member(names.push(name), target)
        == (crate::names::owned_member(names, target) || name@ == target),
{
    assert forall|target: Seq<char>| #[trigger] crate::names::owned_member(names.push(name), target)
        == (crate::names::owned_member(names, target) || name@ == target) by {
        if #[trigger] crate::names::owned_member(names.push(name), target) {
            let at = choose|at: int| 0 <= at < names.push(name).len() && names.push(name)[at]@ == target;
            if at < names.len() { assert(names.push(name)[at] == names[at]); assert(names[at]@ == target); assert(crate::names::owned_member(names, target)); }
        }
        if crate::names::owned_member(names, target) {
            let at = choose|at: int| 0 <= at < names.len() && names[at]@ == target;
            assert(names.push(name)[at]@ == target);
        }
        if name@ == target { assert(names.push(name)[names.len() as int]@ == target); }
    }
}

/// Interpret one VM without losing defaults, overrides, optional values or external providers.
pub fn interpret(d: &Declaration, defaults: &Defaults) -> (result: Result<Vec<String>, InvalidTopology>)
    ensures
        result.is_ok() == interpretable(*d, *defaults),
        result.is_ok() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@, target)
            == depends(*d, *defaults, target),
        result.is_ok() ==> forall|i: int| 0 <= i < result.unwrap().len() ==> guest(#[trigger] result.unwrap()[i]@),
{
    let mut names: Vec<String> = Vec::new();
    let mut k = 0;
    while k < 6
        invariant
            0 <= k <= 6, valid_through(*d, *defaults, k as int, 0),
            forall|target: Seq<char>| crate::names::owned_member(names@, target)
                == depends_through(*d, *defaults, k as int, 0, target),
            forall|i: int| 0 <= i < names.len() ==> guest(#[trigger] names[i]@),
        decreases 6 - k,
    {
        proof { kind_step(*d, *defaults, k as int); }
        if k != 2 || !d.omit_display {
            match resolve(d.setting(k), defaults.setting(k), d.name.as_str()) {
                Ok(Some(name)) => { proof { member_push(names@, name); } names.push(name); },
                Ok(None) => {},
                Err(error) => {
                    assert(!reference_valid(effective(setting(*d, k as int), fallback(*defaults, k as int)), d.name@));
                    proof { if interpretable(*d, *defaults) {
                        assert(valid_through(*d, *defaults, 6, d.devices.len() as int));
                        assert(enabled(*d, k as int));
                        assert(reference_valid(effective(setting(*d, k as int), fallback(*defaults, k as int)), d.name@));
                    }
                    assert(!interpretable(*d, *defaults)); }
                    return Err(error);
                },
            }
        }
        k += 1;
    }

    let mut k = 0;
    while k < d.devices.len()
        invariant
            0 <= k <= d.devices.len(), valid_through(*d, *defaults, 6, k as int),
            forall|target: Seq<char>| crate::names::owned_member(names@, target)
                == depends_through(*d, *defaults, 6, k as int, target),
            forall|i: int| 0 <= i < names.len() ==> guest(#[trigger] names[i]@),
        decreases d.devices.len() - k,
    {
        proof { device_step(*d, *defaults, k as int); }
        match resolve(&d.devices[k], &Reference::Disabled, d.name.as_str()) {
            Ok(Some(name)) => { proof { member_push(names@, name); } names.push(name); },
            Ok(None) => {},
            Err(error) => { return Err(error); },
        }
        k += 1;
    }
    Ok(names)
}

/// A lexical comparison witness: equal prefix followed by an earlier character or prefix end.
pub open spec fn lex_at(left: Seq<char>, right: Seq<char>, k: int) -> bool {
    0 <= k <= left.len() && k < right.len()
        && (forall|p: int| 0 <= p < k ==> #[trigger] left[p] == right[p])
        && (k == left.len() || left[k] < right[k])
}

/// Lexical order of names, independent of their allocation or input enumeration.
pub open spec fn lex_less(left: Seq<char>, right: Seq<char>) -> bool {
    exists|k: int| #[trigger] lex_at(left, right, k)
}

/// Compare exact names in lexical order; accepted guest names contain ASCII only.
fn lexical(left: &str, right: &str) -> (less: bool)
    ensures less == lex_less(left@, right@),
{
    let a = left.unicode_len();
    let b = right.unicode_len();
    let mut k = 0;
    while k < a && k < b
        invariant
            a == left@.len(), b == right@.len(), 0 <= k <= a, 0 <= k <= b,
            forall|p: int| 0 <= p < k ==> #[trigger] left@[p] == right@[p],
        decreases a - k,
    {
        let l = left.get_char(k);
        let r = right.get_char(k);
        if l != r {
            assert(lex_less(left@, right@) == (l < r)) by {
                if l < r { assert(lex_at(left@, right@, k as int)); }
                if lex_less(left@, right@) {
                    let at = choose|at: int| lex_at(left@, right@, at);
                    assert(0 <= at <= a && at < b);
                    if at < k { assert(left@[at] == right@[at]); }
                    if at > k { assert(left@[k as int] == right@[k as int]); }
                }
            }
            return l < r;
        }
        assert forall|p: int| 0 <= p < k + 1 implies #[trigger] left@[p] == right@[p] by { }
        k += 1;
    }
    assert(lex_less(left@, right@) == (a < b)) by {
        if a < b { assert(k == a); assert(lex_at(left@, right@, a as int)); }
        if lex_less(left@, right@) {
            let at = choose|at: int| lex_at(left@, right@, at);
            assert(0 <= at <= a && at < b);
            if at < k { assert(left@[at] == right@[at]); }
        }
    }
    a < b
}

/// Adjacent lexical comparisons imply all earlier names precede a newly admitted name.
proof fn lexical_transitive(left: Seq<char>, middle: Seq<char>, right: Seq<char>)
    requires lex_less(left, middle), lex_less(middle, right),
    ensures lex_less(left, right),
{
    let a = choose|a: int| lex_at(left, middle, a);
    let b = choose|b: int| lex_at(middle, right, b);
    let at = if a < b { a } else { b };
    assert forall|p: int| 0 <= p < at implies #[trigger] left[p] == right[p] by { }
    if a < b {
        assert(middle[a] == right[a]);
    } else if b < a {
        assert(left[b] == middle[b]);
    }
    assert(lex_at(left, right, at));
}

/// Name lookup compares string contents and rejects absent identities.
fn lookup(names: &[String], target: &str) -> (found: Option<usize>)
    ensures
        found.is_some() == crate::names::owned_member(names@, target@),
        found.is_some() ==> found.unwrap() < names.len() && names[found.unwrap() as int]@ == target@,
{
    let mut i = 0;
    while i < names.len()
        invariant 0 <= i <= names.len(), forall|j: int| 0 <= j < i ==> #[trigger] names[j]@ != target@,
        decreases names.len() - i,
    {
        if names[i].as_str() == target { return Some(i); }
        i += 1;
    }
    None
}

/// Membership among declarations is by exact VM identity, not array position.
pub open spec fn declared(domains: Seq<Declaration>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < domains.len() && #[trigger] domains[i].name@ == name
}

/// Locate a declaration without accepting an externally supplied index.
pub(crate) fn lookup_domain(domains: &[Declaration], target: &str) -> (found: Option<usize>)
    ensures
        found.is_some() == declared(domains@, target@),
        found.is_some() ==> found.unwrap() < domains.len() && domains[found.unwrap() as int].name@ == target@,
{
    let mut i = 0;
    while i < domains.len()
        invariant 0 <= i <= domains.len(), forall|j: int| 0 <= j < i ==> #[trigger] domains[j].name@ != target@,
        decreases domains.len() - i,
    {
        if domains[i].name.as_str() == target { return Some(i); }
        i += 1;
    }
    None
}

/// Exactly one lexical table for the declared universe, with no ambiguous identity rows.
pub open spec fn canonical(names: Seq<String>, domains: Seq<Declaration>, extra: Seq<String>) -> bool {
    &&& forall|i: int| 0 <= i < names.len() ==> guest(#[trigger] names[i]@)
    &&& forall|i: int, j: int| 0 <= i < j < names.len() ==> lex_less(#[trigger] names[i]@, #[trigger] names[j]@)
    &&& forall|i: int| 0 <= i < domains.len() ==> guest(#[trigger] domains[i].name@)
        && crate::names::owned_member(names, domains[i].name@)
    &&& forall|i: int, j: int| 0 <= i < j < domains.len() ==> #[trigger] domains[i].name@ != #[trigger] domains[j].name@
    &&& forall|i: int| 0 <= i < extra.len() ==> guest(#[trigger] extra[i]@) && crate::names::owned_member(names, extra[i]@)
    &&& forall|i: int| 0 <= i < names.len() ==> declared(domains, #[trigger] names[i]@)
        || crate::names::owned_member(extra, names[i]@)
}

/// Check a proposed canonical table rather than trusting the caller's map enumeration.
fn check_names(names: &[String], domains: &[Declaration], extra: &[String]) -> (accepted: bool)
    ensures accepted == canonical(names@, domains@, extra@),
{
    let mut i = 0;
    while i < names.len()
        invariant
            0 <= i <= names.len(),
            forall|a: int| 0 <= a < i ==> guest(#[trigger] names[a]@),
            forall|a: int, b: int| 0 <= a < b < i ==> lex_less(#[trigger] names[a]@, #[trigger] names[b]@),
        decreases names.len() - i,
    {
        if !valid_guest(names[i].as_str()) { return false; }
        if i > 0 && !lexical(names[i - 1].as_str(), names[i].as_str()) { return false; }
        assert forall|a: int| 0 <= a < i implies lex_less(#[trigger] names[a]@, names[i as int]@) by {
            if a < i - 1 { lexical_transitive(names[a]@, names[i - 1]@, names[i as int]@); }
        }
        i += 1;
    }

    let mut i = 0;
    while i < domains.len()
        invariant
            0 <= i <= domains.len(),
            forall|a: int| 0 <= a < names.len() ==> guest(#[trigger] names[a]@),
            forall|a: int, b: int| 0 <= a < b < names.len() ==> lex_less(#[trigger] names[a]@, #[trigger] names[b]@),
            forall|a: int| 0 <= a < i ==> guest(#[trigger] domains[a].name@) && crate::names::owned_member(names@, domains[a].name@),
            forall|a: int, b: int| 0 <= a < i && a < b < domains.len() ==> #[trigger] domains[a].name@ != #[trigger] domains[b].name@,
        decreases domains.len() - i,
    {
        if !valid_guest(domains[i].name.as_str()) || lookup(names, domains[i].name.as_str()).is_none() { return false; }
        let mut j = i + 1;
        while j < domains.len()
            invariant
                0 <= i < domains.len(), i < j <= domains.len(),
                forall|b: int| i < b < j ==> domains[i as int].name@ != #[trigger] domains[b].name@,
            decreases domains.len() - j,
        {
            if domains[i].name.as_str() == domains[j].name.as_str() { return false; }
            j += 1;
        }
        i += 1;
    }

    let mut i = 0;
    while i < extra.len()
        invariant
            0 <= i <= extra.len(),
            forall|a: int| 0 <= a < names.len() ==> guest(#[trigger] names[a]@),
            forall|a: int, b: int| 0 <= a < b < names.len() ==> lex_less(#[trigger] names[a]@, #[trigger] names[b]@),
            forall|a: int| 0 <= a < domains.len() ==> guest(#[trigger] domains[a].name@) && crate::names::owned_member(names@, domains[a].name@),
            forall|a: int, b: int| 0 <= a < b < domains.len() ==> #[trigger] domains[a].name@ != #[trigger] domains[b].name@,
            forall|a: int| 0 <= a < i ==> guest(#[trigger] extra[a]@) && crate::names::owned_member(names@, extra[a]@),
        decreases extra.len() - i,
    {
        if !valid_guest(extra[i].as_str()) || lookup(names, extra[i].as_str()).is_none() { return false; }
        i += 1;
    }

    let mut i = 0;
    while i < names.len()
        invariant
            0 <= i <= names.len(),
            forall|a: int| 0 <= a < names.len() ==> guest(#[trigger] names[a]@),
            forall|a: int, b: int| 0 <= a < b < names.len() ==> lex_less(#[trigger] names[a]@, #[trigger] names[b]@),
            forall|a: int| 0 <= a < domains.len() ==> guest(#[trigger] domains[a].name@) && crate::names::owned_member(names@, domains[a].name@),
            forall|a: int, b: int| 0 <= a < b < domains.len() ==> #[trigger] domains[a].name@ != #[trigger] domains[b].name@,
            forall|a: int| 0 <= a < extra.len() ==> guest(#[trigger] extra[a]@) && crate::names::owned_member(names@, extra[a]@),
            forall|a: int| 0 <= a < i ==> declared(domains@, #[trigger] names[a]@) || crate::names::owned_member(extra@, names[a]@),
        decreases names.len() - i,
    {
        if lookup_domain(domains, names[i].as_str()).is_none() && lookup(extra, names[i].as_str()).is_none() { return false; }
        i += 1;
    }
    true
}

/// Effective named relation, including external providers not declared in the universe.
pub open spec fn relationship(domains: Seq<Declaration>, defaults: Defaults, owner: Seq<char>, target: Seq<char>) -> bool {
    exists|i: int| 0 <= i < domains.len() && domains[i].name@ == owner && #[trigger] depends(domains[i], defaults, target)
}

/// All declarations have interpretable effective values.
pub open spec fn all_interpretable(domains: Seq<Declaration>, defaults: Defaults) -> bool {
    forall|i: int| 0 <= i < domains.len() ==> #[trigger] interpretable(domains[i], defaults)
}

/// Pure provenance retained only for checking the constructor and planning contracts.
pub ghost struct Origin {
    /// Original decoded declarations, independent of their enumeration order.
    pub domains: Seq<Declaration>,
    /// Exact inherited host values used during interpretation.
    pub defaults: Defaults,
    /// Additional vertices for affected-VM propagation; these grant no execution authority.
    pub extra: Seq<String>,
}

/// Mathematical interpretation and graph belonging to one immutable name table.
pub ghost struct NamedState {
    /// Canonical identities represented by graph indices.
    pub names: Seq<String>,
    /// Actual graph consumed by the verified planning algorithms.
    pub graph: GraphState,
    /// Decoded inputs to which every edge is bound.
    pub origin: Origin,
}

/// A graph represents exactly the effective internal relationships of the original declarations.
pub open spec fn faithful(s: NamedState) -> bool {
    &&& canonical(s.names, s.origin.domains, s.origin.extra)
    &&& all_interpretable(s.origin.domains, s.origin.defaults)
    &&& s.graph.count == s.names.len()
    &&& graph::valid(s.graph)
    &&& forall|a: usize, b: usize| a < s.names.len() && b < s.names.len() ==>
        #[trigger] graph::dependency(s.graph, a, b)
            == relationship(s.origin.domains, s.origin.defaults, s.names[a as int]@, s.names[b as int]@)
}

/// Opaque graph which owns reference interpretation and every name/index conversion.
/// Callers provide decoded declarations and proposed canonical names, never raw edges or indices.
///
/// ```compile_fail
/// use airlock_machine_planning::topology::NamedGraph;
/// fn substitute(graph: &mut NamedGraph) {
///     graph.names.swap(0, 1);
/// }
/// ```
pub struct NamedGraph {
    // Canonical identity table checked against the declarations and impact seeds.
    names: Vec<String>,
    // The same graph executed by the existing verified planning kernel.
    graph: Graph,
    // Proof-only provenance; erased from ordinary Cargo builds.
    _origin: Ghost<Origin>,
}

impl View for NamedGraph {
    type V = NamedState;

    /// Expose immutable identities, relationships and the planner graph to contracts.
    closed spec fn view(&self) -> NamedState {
        NamedState { names: self.names@, graph: self.graph@, origin: self._origin@ }
    }
}

/// Appending one indexed relationship changes the mathematical graph by exactly that edge.
proof fn edge_push(count: usize, edges: Seq<Edge>, edge: Edge)
    ensures forall|a: usize, b: usize| #[trigger] graph::dependency(GraphState { count, edges: edges.push(edge) }, a, b)
        == (graph::dependency(GraphState { count, edges }, a, b) || (a == edge.dependent && b == edge.dependency)),
{
    assert forall|a: usize, b: usize| #[trigger] graph::dependency(GraphState { count, edges: edges.push(edge) }, a, b)
        == (graph::dependency(GraphState { count, edges }, a, b) || (a == edge.dependent && b == edge.dependency)) by {
        if graph::dependency(GraphState { count, edges: edges.push(edge) }, a, b) {
            let at = choose|at: int| 0 <= at < edges.push(edge).len()
                && edges.push(edge)[at].dependent == a && edges.push(edge)[at].dependency == b;
            if at < edges.len() {
                assert(edges.push(edge)[at] == edges[at]);
                assert(edges[at].dependent == a && edges[at].dependency == b);
                let g = GraphState { count, edges };
                assert(g.edges[at].dependent == a && g.edges[at].dependency == b);
                assert(graph::dependency(g, a, b));
            }
        }
        if graph::dependency(GraphState { count, edges }, a, b) {
            let at = choose|at: int| 0 <= at < edges.len() && edges[at].dependent == a && edges[at].dependency == b;
            assert(edges.push(edge)[at].dependent == a && edges.push(edge)[at].dependency == b);
        }
        if a == edge.dependent && b == edge.dependency {
            assert(edges.push(edge)[edges.len() as int].dependent == a && edges.push(edge)[edges.len() as int].dependency == b);
        }
    }
}

impl NamedGraph {
    /// Carry the exact source relation and identity table through every later planning call.
    #[verifier::type_invariant]
    closed spec fn well_formed(self) -> bool { faithful(self@) }

    /// Interpret decoded relationships and check a complete canonical identity table.
    /// External providers remain in the named relation but do not acquire implicit vertices.
    pub fn new(names: Vec<String>, domains: &[Declaration], defaults: &Defaults, extra: &[String])
        -> (result: Result<Self, InvalidTopology>)
        ensures
            result.is_ok() == (canonical(names@, domains@, extra@) && all_interpretable(domains@, *defaults)),
            result.is_ok() ==> faithful(result.unwrap()@)
                && result.unwrap()@.names == names@
                && result.unwrap()@.origin == (Origin { domains: domains@, defaults: *defaults, extra: extra@ }),
    {
        if !check_names(&names, domains, extra) { return Err(InvalidTopology::Identity); }

        let mut resolved: Vec<Vec<String>> = Vec::new();
        let mut k = 0;
        while k < domains.len()
            invariant
                canonical(names@, domains@, extra@),
                0 <= k <= domains.len(), resolved.len() == k,
                forall|d: int| 0 <= d < k ==> #[trigger] interpretable(domains[d], *defaults),
                forall|d: int, target: Seq<char>| 0 <= d < k ==>
                    #[trigger] crate::names::owned_member(resolved[d]@, target) == depends(domains[d], *defaults, target),
            decreases domains.len() - k,
        {
            let row = interpret(&domains[k], defaults)?;
            resolved.push(row);
            k += 1;
        }

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut i = 0;
        while i < names.len()
            invariant
                canonical(names@, domains@, extra@), all_interpretable(domains@, *defaults),
                resolved.len() == domains.len(),
                forall|d: int, target: Seq<char>| 0 <= d < domains.len() ==>
                    #[trigger] crate::names::owned_member(resolved[d]@, target) == depends(domains[d], *defaults, target),
                0 <= i <= names.len(), rows.len() == i,
                forall|a: int, target: Seq<char>| 0 <= a < i ==>
                    #[trigger] crate::names::owned_member(rows[a]@, target)
                        == relationship(domains@, *defaults, names[a]@, target),
            decreases names.len() - i,
        {
            let mut row: Vec<String> = Vec::new();
            if let Some(d) = lookup_domain(domains, names[i].as_str()) {
                let mut p = 0;
                while p < resolved[d].len()
                    invariant
                        d < domains.len(), resolved.len() == domains.len(),
                        0 <= p <= resolved[d as int].len(), row.len() == p,
                        forall|j: int| 0 <= j < p ==> #[trigger] row[j]@ == resolved[d as int][j]@,
                    decreases resolved[d as int].len() - p,
                {
                    row.push(resolved[d][p].as_str().to_string());
                    p += 1;
                }
                assert forall|target: Seq<char>| crate::names::owned_member(row@, target)
                    == relationship(domains@, *defaults, names[i as int]@, target) by {
                    if crate::names::owned_member(row@, target) {
                        let p = choose|p: int| 0 <= p < row.len() && row[p]@ == target;
                        assert(crate::names::owned_member(resolved[d as int]@, target));
                        assert(depends(domains[d as int], *defaults, target));
                    }
                    if relationship(domains@, *defaults, names[i as int]@, target) {
                        let other = choose|other: int| 0 <= other < domains.len() && domains[other].name@ == names[i as int]@
                            && depends(domains[other], *defaults, target);
                        assert(other == d);
                        assert(crate::names::owned_member(resolved[d as int]@, target));
                        let p = choose|p: int| 0 <= p < resolved[d as int].len() && resolved[d as int][p]@ == target;
                        assert(row[p]@ == target);
                    }
                }
            }
            rows.push(row);
            i += 1;
        }

        let mut edges: Vec<Edge> = Vec::new();
        let mut i = 0;
        while i < names.len()
            invariant
                0 <= i <= names.len(), rows.len() == names.len(),
                forall|a: int, target: Seq<char>| 0 <= a < names.len() ==>
                    #[trigger] crate::names::owned_member(rows[a]@, target)
                        == relationship(domains@, *defaults, names[a]@, target),
                graph::valid(GraphState { count: names.len(), edges: edges@ }),
                forall|a: usize, b: usize| a < names.len() && b < names.len() ==>
                    #[trigger] graph::dependency(GraphState { count: names.len(), edges: edges@ }, a, b)
                        == (a < i && relationship(domains@, *defaults, names[a as int]@, names[b as int]@)),
            decreases names.len() - i,
        {
            let mut j = 0;
            while j < names.len()
                invariant
                    0 <= i < names.len(), 0 <= j <= names.len(), rows.len() == names.len(),
                    forall|a: int, target: Seq<char>| 0 <= a < names.len() ==>
                        #[trigger] crate::names::owned_member(rows[a]@, target)
                            == relationship(domains@, *defaults, names[a]@, target),
                    graph::valid(GraphState { count: names.len(), edges: edges@ }),
                    forall|a: usize, b: usize| a < names.len() && b < names.len() ==>
                        #[trigger] graph::dependency(GraphState { count: names.len(), edges: edges@ }, a, b)
                            == ((a < i || (a == i && b < j)) && relationship(domains@, *defaults, names[a as int]@, names[b as int]@)),
                decreases names.len() - j,
            {
                if crate::names::owned_contains(&rows[i], names[j].as_str()) {
                    let edge = Edge { dependent: i, dependency: j };
                    proof { edge_push(names.len(), edges@, edge); }
                    let ghost before = edges@;
                    edges.push(edge);
                    assert forall|e: int| 0 <= e < edges.len() implies #[trigger] graph::valid_edge(edges[e], names.len()) by {
                        if e < before.len() {
                            let g = GraphState { count: names.len(), edges: before };
                            assert(graph::valid_edge(g.edges[e], g.count));
                            assert(edges[e] == before[e]);
                        } else { assert(edges[e] == edge); }
                    }
                }
                j += 1;
            }
            i += 1;
        }
        let graph = match Graph::new(names.len(), edges) {
            Some(graph) => graph,
            None => { assert(false); return Err(InvalidTopology::Identity); },
        };
        Ok(Self { names, graph, _origin: Ghost(Origin { domains: domains@, defaults: *defaults, extra: extra@ }) })
    }
}

/// Membership mask is defined by VM names, rather than supplied graph positions.
pub open spec fn membership(names: Seq<String>, members: Seq<String>) -> Seq<bool> {
    Seq::new(names.len(), |i: int| crate::names::owned_member(members, names[i]@))
}

/// Names selected by a mask over this exact canonical table.
pub open spec fn masked(names: Seq<String>, mask: Seq<bool>, target: Seq<char>) -> bool {
    exists|i: int| 0 <= i < names.len() && i < mask.len() && mask[i] && #[trigger] names[i]@ == target
}

/// Named output retains a kernel schedule and its exact original VM identities.
pub open spec fn named_schedule(s: NamedState, members: Seq<String>, targets: Seq<String>, reverse: bool) -> bool {
    exists|vertices: Seq<usize>| graph::schedule(s.graph, membership(s.names, members), vertices, reverse)
        && #[trigger] graph::names_match(s.names, vertices, targets)
}

impl NamedGraph {
    /// Encode named inputs in this graph's checked identity space; external names have no bit.
    fn mask(&self, members: &[String]) -> (mask: Vec<bool>)
        ensures mask@ == membership(self@.names, members@),
    {
        let mut mask: Vec<bool> = Vec::new();
        let mut i = 0;
        while i < self.names.len()
            invariant
                0 <= i <= self.names.len(), mask.len() == i,
                forall|j: int| 0 <= j < i ==> #[trigger] mask[j] == crate::names::owned_member(members@, self.names[j]@),
            decreases self.names.len() - i,
        {
            mask.push(crate::names::owned_contains(members, self.names[i].as_str()));
            i += 1;
        }
        assert(mask@ =~= membership(self@.names, members@));
        mask
    }

    /// Decode only a correctly sized mask; every selected bit yields precisely its own VM name.
    fn decode_mask(&self, mask: &[bool]) -> (result: Option<Vec<String>>)
        ensures
            result.is_some() == (mask.len() == self@.names.len()),
            result.is_some() ==> forall|target: Seq<char>| crate::names::owned_member(result.unwrap()@, target)
                == masked(self@.names, mask@, target),
    {
        if mask.len() != self.names.len() { return None; }
        let mut names: Vec<String> = Vec::new();
        let mut i = 0;
        while i < self.names.len()
            invariant
                mask.len() == self.names.len(), 0 <= i <= self.names.len(),
                forall|target: Seq<char>| crate::names::owned_member(names@, target)
                    == (exists|j: int| 0 <= j < i && mask[j] && #[trigger] self.names[j]@ == target),
            decreases self.names.len() - i,
        {
            let ghost previous = names@;
            if mask[i] {
                let name = self.names[i].as_str().to_string();
                proof { member_push(names@, name); }
                names.push(name);
            }
            assert forall|target: Seq<char>| crate::names::owned_member(names@, target)
                == (exists|j: int| 0 <= j < i + 1 && mask[j] && #[trigger] self.names[j]@ == target) by {
                if exists|j: int| 0 <= j < i + 1 && mask[j] && self.names[j]@ == target {
                    let j = choose|j: int| 0 <= j < i + 1 && mask[j] && self.names[j]@ == target;
                    if j < i { assert(crate::names::owned_member(previous, target)); }
                }
            }
            i += 1;
        }
        Some(names)
    }

    /// Compute named affected/dependency closure through the graph bound to the decoded topology.
    pub fn expand(&self, seeds: &[String], reverse: bool) -> (targets: Vec<String>)
        ensures
            faithful(self@),
            exists|mask: Seq<bool>| mask.len() == self@.names.len()
                && graph::closure(self@.graph, graph::selected(membership(self@.names, seeds@)), graph::selected(mask), reverse)
                && forall|target: Seq<char>| crate::names::owned_member(targets@, target) == masked(self@.names, mask, target),
    {
        proof { use_type_invariant(self); }
        let mask = self.graph.expand(self.mask(seeds), reverse).unwrap();
        self.decode_mask(&mask).unwrap()
    }

    /// Order a named subset without accepting independently supplied indices or output names.
    pub fn ordered(&self, members: &[String], reverse: bool) -> (result: Option<Vec<String>>)
        ensures
            faithful(self@),
            result.is_none() == !(exists|order: Seq<usize>| graph::full_order(self@.graph, order)),
            result.is_some() ==> named_schedule(self@, members@, result.unwrap()@, reverse),
    {
        proof { use_type_invariant(self); }
        let order = self.graph.schedule(self.mask(members), reverse)?;
        let targets = graph::name_targets(&self.names, &order).unwrap();
        assert(graph::schedule(self@.graph, membership(self@.names, members@), order@, reverse));
        Some(targets)
    }

    /// Compute exact named startup effects in dependency order from this immutable topology.
    pub fn startup(&self, seeds: &[String]) -> (result: Option<Vec<String>>)
        ensures
            faithful(self@),
            result.is_none() == !(exists|order: Seq<usize>| graph::full_order(self@.graph, order)),
            result.is_some() ==> exists|vertices: Seq<usize>|
                graph::closure(self@.graph, graph::selected(membership(self@.names, seeds@)), graph::listed(vertices), false)
                && vertices.no_duplicates() && graph::respects(self@.graph, vertices, false)
                && #[trigger] graph::names_match(self@.names, vertices, result.unwrap()@),
    {
        proof { use_type_invariant(self); }
        let order = self.graph.startup(self.mask(seeds))?;
        let targets = graph::name_targets(&self.names, &order).unwrap();
        assert(graph::names_match(self@.names, order@, targets@));
        Some(targets)
    }

    /// Compute exact named affected/running shutdowns in reverse dependency order.
    pub fn shutdown(&self, affected: &[String], running: &[String]) -> (result: Option<Vec<String>>)
        ensures
            faithful(self@),
            result.is_none() == !(exists|order: Seq<usize>| graph::full_order(self@.graph, order)),
            result.is_some() ==> exists|vertices: Seq<usize>|
                vertices.no_duplicates() && graph::respects(self@.graph, vertices, true)
                && (forall|v: usize| v < self@.names.len() ==> #[trigger] vertices.contains(v)
                    == (crate::names::owned_member(affected@, self@.names[v as int]@)
                        && crate::names::owned_member(running@, self@.names[v as int]@)))
                && #[trigger] graph::names_match(self@.names, vertices, result.unwrap()@),
    {
        proof { use_type_invariant(self); }
        let order = self.graph.shutdown(self.mask(affected), self.mask(running))?;
        let targets = graph::name_targets(&self.names, &order).unwrap();
        assert(graph::names_match(self@.names, order@, targets@));
        Some(targets)
    }
}

/// Lexical order cannot order a spelling before itself.
proof fn lexical_irreflexive(name: Seq<char>)
    ensures !lex_less(name, name),
{
    if lex_less(name, name) {
        let at = choose|at: int| lex_at(name, name, at);
        assert(false);
    }
}

/// Two unequal lexical order witnesses cannot point in opposite directions.
proof fn lexical_asymmetric(left: Seq<char>, right: Seq<char>)
    requires lex_less(left, right),
    ensures !lex_less(right, left),
{
    if lex_less(right, left) {
        let a = choose|a: int| lex_at(left, right, a);
        let b = choose|b: int| lex_at(right, left, b);
        if a < b { assert(left[a] == right[a]); }
        else if b < a { assert(left[b] == right[b]); }
        else { assert(a < left.len() && b < right.len()); }
    }
}

/// A lexical table's content order, independent of String allocation identities.
pub open spec fn sorted(names: Seq<Seq<char>>) -> bool {
    forall|i: int, j: int| 0 <= i < j < names.len() ==> #[trigger] lex_less(names[i], names[j])
}

/// Sorted sequences with the same members have the same prefix and cannot differ in length.
proof fn sorted_prefix(left: Seq<Seq<char>>, right: Seq<Seq<char>>, at: int)
    requires
        sorted(left), sorted(right), left.to_set() == right.to_set(),
        0 <= at <= left.len(), at <= right.len(),
        forall|i: int| 0 <= i < at ==> #[trigger] left[i] == right[i],
    ensures left.len() <= right.len(), forall|i: int| 0 <= i < left.len() ==> #[trigger] left[i] == right[i],
    decreases left.len() - at,
{
    left.to_set_ensures();
    right.to_set_ensures();
    if at < left.len() {
        assert(left.to_set().contains(left[at]));
        assert(right.contains(left[at]));
        let j = choose|j: int| 0 <= j < right.len() && right[j] == left[at];
        if j < at {
            assert(left[j] == left[at]);
            assert(lex_less(left[j], left[at]));
            lexical_irreflexive(left[at]);
        }
        assert(at < right.len());
        if left[at] != right[at] {
            assert(right.to_set().contains(right[at]));
            assert(left.contains(right[at]));
            let k = choose|k: int| 0 <= k < left.len() && left[k] == right[at];
            if k < at {
                assert(right[k] == right[at]);
                assert(lex_less(right[k], right[at]));
                lexical_irreflexive(right[at]);
            }
            assert(j > at && k > at);
            assert(lex_less(left[at], right[at]));
            assert(lex_less(right[at], left[at]));
            lexical_asymmetric(left[at], right[at]);
        }
        sorted_prefix(left, right, at + 1);
    }
}

/// Canonical table contents are unique for a given identity universe.
pub proof fn canonical_unique(left: Seq<Seq<char>>, right: Seq<Seq<char>>)
    requires sorted(left), sorted(right), left.to_set() == right.to_set(),
    ensures left == right,
{
    sorted_prefix(left, right, 0);
    sorted_prefix(right, left, 0);
    assert(left =~= right);
}

/// Canonical names contain exactly declarations and explicit impact seeds.
proof fn universe(s: NamedState)
    requires faithful(s),
    ensures forall|target: Seq<char>| crate::names::owned_member(s.names, target)
        == (declared(s.origin.domains, target) || crate::names::owned_member(s.origin.extra, target)),
{
    assert forall|target: Seq<char>| crate::names::owned_member(s.names, target)
        == (declared(s.origin.domains, target) || crate::names::owned_member(s.origin.extra, target)) by {
        if crate::names::owned_member(s.names, target) {
            let i = choose|i: int| 0 <= i < s.names.len() && s.names[i]@ == target;
            assert(declared(s.origin.domains, s.names[i]@) || crate::names::owned_member(s.origin.extra, s.names[i]@));
        }
        if declared(s.origin.domains, target) {
            let d = choose|d: int| 0 <= d < s.origin.domains.len() && s.origin.domains[d].name@ == target;
            assert(crate::names::owned_member(s.names, s.origin.domains[d].name@));
        }
        if crate::names::owned_member(s.origin.extra, target) {
            let i = choose|i: int| 0 <= i < s.origin.extra.len() && s.origin.extra[i]@ == target;
            assert(crate::names::owned_member(s.names, s.origin.extra[i]@));
        }
    }
}

/// Equivalent decoded configurations have the same identity universe and effective relation.
pub open spec fn equivalent_topologies(left: Origin, right: Origin) -> bool {
    &&& forall|name: Seq<char>| (declared(left.domains, name) || crate::names::owned_member(left.extra, name))
        == (declared(right.domains, name) || crate::names::owned_member(right.extra, name))
    &&& forall|owner: Seq<char>, target: Seq<char>| #[trigger] relationship(left.domains, left.defaults, owner, target)
        == relationship(right.domains, right.defaults, owner, target)
}

/// Valid graphs cannot have edges outside their vertex space.
proof fn outside_graph(graph: GraphState, a: usize, b: usize)
    requires graph::valid(graph), a >= graph.count || b >= graph.count,
    ensures !graph::dependency(graph, a, b),
{
    if graph::dependency(graph, a, b) {
        let e = choose|e: int| 0 <= e < graph.edges.len() && graph.edges[e].dependent == a && graph.edges[e].dependency == b;
        assert(graph::valid_edge(graph.edges[e], graph.count));
    }
}

/// Relate content-set membership to the executable owned string list.
proof fn contents_members(names: Seq<String>)
    ensures forall|name: Seq<char>| names.map(|i, n: String| n@).to_set().contains(name)
        == crate::names::owned_member(names, name),
{
    let contents = names.map(|i, n: String| n@);
    contents.to_set_ensures();
    assert forall|name: Seq<char>| contents.to_set().contains(name)
        == crate::names::owned_member(names, name) by {
        if contents.contains(name) {
            let i = choose|i: int| 0 <= i < contents.len() && contents[i] == name;
            assert(names[i]@ == name);
        }
        if crate::names::owned_member(names, name) {
            let i = choose|i: int| 0 <= i < names.len() && names[i]@ == name;
            assert(contents[i] == name);
        }
    }
}

/// Equivalent effective topologies yield the same canonical names and equivalent planner graphs.
/// This theorem covers declaration/device enumeration and duplicate relationship edges.
pub proof fn interpretation_deterministic(left: NamedState, right: NamedState)
    requires faithful(left), faithful(right), equivalent_topologies(left.origin, right.origin),
    ensures
        left.names.map(|i, name: String| name@) == right.names.map(|i, name: String| name@),
        graph::equivalent(left.graph, right.graph),
{
    universe(left);
    universe(right);
    contents_members(left.names);
    contents_members(right.names);
    let a = left.names.map(|i, name: String| name@);
    let b = right.names.map(|i, name: String| name@);
    assert(sorted(a));
    assert(sorted(b));
    assert(a.to_set() =~= b.to_set()) by {
        assert forall|name: Seq<char>| a.to_set().contains(name) == b.to_set().contains(name) by {
            assert(a.to_set().contains(name) == crate::names::owned_member(left.names, name));
            assert(b.to_set().contains(name) == crate::names::owned_member(right.names, name));
        }
    }
    canonical_unique(a, b);
    assert forall|d: usize, p: usize| #[trigger] graph::dependency(left.graph, d, p)
        == graph::dependency(right.graph, d, p) by {
        if d < left.graph.count && p < left.graph.count {
            assert(a[d as int] == b[d as int] && a[p as int] == b[p as int]);
        } else {
            outside_graph(left.graph, d, p);
            outside_graph(right.graph, d, p);
        }
    }
}

}
