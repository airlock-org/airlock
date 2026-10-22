//! Logical reclamation consumes permissions tied to the original pool, resource category and trace.
//! Numeric buckets accelerate exact comparisons; collisions never supply deletion authority.

#[cfg(verus_keep_ghost)]
use super::inventory::advance;

use super::tracing::{Resource, Trace};
use std::collections::HashMap;
use vstd::prelude::*;

verus! {

broadcast use {vstd::seq_lib::group_seq_properties, vstd::std_specs::hash::group_hash_axioms};

/// Distinct physical metadata locations can depend on the same logical object.
pub enum Target {
    /// Logical object record, protected by the exact object basename.
    ObjectMetadata(
        /// Basename without the metadata filename's JSON extension.
        String,
    ),
    /// Canonical fragment-location record, including the shared tar terminator.
    FragmentMetadata(
        /// Basename without the metadata filename's JSON extension.
        String,
    ),
    /// Immutable software view, protected by its full canonical directory identity.
    View(
        /// Exact directory, including the original pool prefix.
        String,
    ),
    /// Immutable boot disk artifact, protected by its original content digest.
    Root(
        /// Exact artifact directory basename.
        String,
    ),
}

/// Independent projection: both metadata records require the object; other categories stay distinct.
pub open spec fn subject(t: Target) -> Resource {
    match t {
        Target::ObjectMetadata(n) | Target::FragmentMetadata(n) => Resource::Object(n),
        Target::View(n) => Resource::View(n),
        Target::Root(n) => Resource::Root(n),
    }
}

/// Full original identity; no normalization or digest truncation is used for permission checks.
pub open spec fn identity(r: Resource) -> Seq<char> {
    match r { Resource::Object(n) | Resource::View(n) | Resource::Root(n) | Resource::Segment(n) => n@ }
}

// The mathematical bucket recurrence supports efficient indexing, not an assumption of hash uniqueness.
spec fn bucket_bytes(bytes: Seq<u8>) -> u64
    decreases bytes.len(),
{
    if bytes.len() == 0 { 0 }
    else { ((bucket_bytes(bytes.drop_last()) as int * 257 + bytes.last() as int) % 4294967296) as u64 }
}

spec fn key(r: Resource) -> u64 { bucket_bytes(vstd::utf8::encode_utf8(identity(r))) }

fn bucket(name: &str) -> (out: u64)
    ensures out == bucket_bytes(vstd::utf8::encode_utf8(name@)), out < 4294967296,
{
    let bytes = name.as_bytes();
    let mut value: u64 = 0;
    let mut i = 0;
    while i < bytes.len()
        invariant 0 <= i <= bytes.len(), value < 4294967296,
            bytes@ == vstd::utf8::encode_utf8(name@), value == bucket_bytes(bytes@.take(i as int)),
        decreases bytes.len() - i,
    {
        value = (value * 257 + bytes[i] as u64) % 4294967296;
        proof {
            assert(bytes@.take(i as int + 1).drop_last() == bytes@.take(i as int));
            assert(bytes@.take(i as int + 1).last() == bytes@[i as int]);
        }
        i += 1;
    }
    value
}

fn name(r: &Resource) -> (out: &str) ensures out@ == identity(*r),
{
    match r { Resource::Object(n) | Resource::View(n) | Resource::Root(n) | Resource::Segment(n) => n.as_str() }
}

fn copy(r: &Resource) -> (out: Resource) ensures super::tracing::same(out, *r),
{
    match r {
        Resource::Object(n) => Resource::Object(n.as_str().to_string()),
        Resource::View(n) => Resource::View(n.as_str().to_string()),
        Resource::Root(n) => Resource::Root(n.as_str().to_string()),
        Resource::Segment(n) => Resource::Segment(n.as_str().to_string()),
    }
}

fn matches(left: &Resource, right: &Resource) -> (out: bool)
    ensures out == super::tracing::same(*left, *right),
{
    match (left, right) {
        (Resource::Object(a), Resource::Object(b)) | (Resource::View(a), Resource::View(b))
        | (Resource::Root(a), Resource::Root(b)) | (Resource::Segment(a), Resource::Segment(b)) => *a == *b,
        _ => false,
    }
}

spec fn indexed(entries: Map<u64, Vec<Resource>>, r: Resource) -> bool {
    entries.contains_key(key(r)) && super::tracing::contains(entries[key(r)]@, r)
}

fn add(entries: &mut HashMap<u64, Vec<Resource>>, r: &Resource)
    ensures forall|p: Resource| #[trigger] indexed(final(entries)@, p)
        == (indexed(old(entries)@, p) || super::tracing::same(*r, p)),
{
    let ghost before = entries@;
    let k = bucket(name(r));
    let mut values = entries.remove(&k).unwrap_or_default();
    let ghost previous = values@;
    values.push(copy(r));
    entries.insert(k, values);
    proof {
        assert forall|p: Resource| #[trigger] indexed(entries@, p)
            == (indexed(before, p) || super::tracing::same(*r, p)) by {
            if super::tracing::same(*r, p) {
                assert(identity(*r) == identity(p));
                assert(key(p) == k);
                assert(super::tracing::same(entries@[k][previous.len() as int], p));
            }
            if key(p) == k {
                if indexed(entries@, p) {
                    let j = choose|j: int| 0 <= j < entries@[k].len() && super::tracing::same(entries@[k][j], p);
                    if j < previous.len() { assert(super::tracing::same(previous[j], p)); }
                    else { assert(j == previous.len()); }
                }
                if indexed(before, p) {
                    let j = choose|j: int| 0 <= j < before[k].len() && super::tracing::same(before[k][j], p);
                    assert(super::tracing::same(entries@[k][j], p));
                }
            }
        }
    }
}

/// Immutable exact index over every category in the original resource trace.
pub struct ResourceIndex<'a> {
    // Pool coordinate retained by every resulting deletion request.
    pool: String,
    // Original complete logical requirements, borrowed through the lifetime of every permission.
    trace: &'a Trace,
    // Numeric buckets contain full category/name pairs; no collision can erase a requirement.
    entries: HashMap<u64, Vec<Resource>>,
}

impl<'a> View for ResourceIndex<'a> {
    type V = Seq<Resource>;

    closed spec fn view(&self) -> Self::V { self.trace@ }
}

impl<'a> ResourceIndex<'a> {
    /// Original pool shared by source tracing, candidate discovery and dispatch.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.pool@.len() > 0 && forall|r: Resource| #[trigger] indexed(self.entries@, r) == super::tracing::contains(self.trace@, r)
    }

    /// Index every supplied requirement; unsupported empty identities refuse the entire pass.
    pub fn new(pool: String, trace: &'a Trace) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == trace@ && out.unwrap().pool_name() == pool@,
    {
        if pool.is_empty() { return None; }
        let references = trace.references();
        let mut entries = HashMap::new();
        let mut i = 0;
        while i < references.len()
            invariant 0 <= i <= references.len(), pool@.len() > 0, references@ == trace@,
                forall|r: Resource| #[trigger] indexed(entries@, r) == super::tracing::contains(references@.take(i as int), r),
            decreases references.len() - i,
        {
            if name(&references[i]).is_empty() { return None; }
            add(&mut entries, &references[i]);
            proof {
                assert forall|r: Resource| #[trigger] indexed(entries@, r) == super::tracing::contains(references@.take(i as int + 1), r) by {
                    advance(references@, i as int, |p: Resource| super::tracing::same(p, r));
                }
            }
            i += 1;
        }
        Some(Self { pool, trace, entries })
    }

    /// Authorize exactly a nonempty target whose logical resource is absent from the original trace.
    pub fn reclaim(&self, target: Target) -> (out: Option<DeleteRequest<'a>>)
        ensures out.is_some() == (identity(subject(target)).len() > 0 && !super::tracing::contains(self@, subject(target))),
            out.is_some() ==> out.unwrap()@ == target && out.unwrap().pool_name() == self.pool_name()
                && out.unwrap().reference_set() == self@,
    {
        proof { use_type_invariant(self); }
        let resource = match &target {
            Target::ObjectMetadata(n) | Target::FragmentMetadata(n) => Resource::Object(n.as_str().to_string()),
            Target::View(n) => Resource::View(n.as_str().to_string()),
            Target::Root(n) => Resource::Root(n.as_str().to_string()),
        };
        if name(&resource).is_empty() { return None; }
        let k = bucket(name(&resource));
        if let Some(values) = self.entries.get(&k) {
            let mut i = 0;
            while i < values.len()
                invariant 0 <= i <= values.len(), self.valid(), self.entries@.contains_key(k), values@ == self.entries@[k]@,
                    k == key(resource), super::tracing::same(resource, subject(target)),
                    identity(resource).len() > 0,
                    forall|j: int| 0 <= j < i ==> !super::tracing::same(#[trigger] values[j], resource),
                decreases values.len() - i,
            {
                if matches(&values[i], &resource) {
                    proof { assert(indexed(self.entries@, resource)); }
                    return None;
                }
                i += 1;
            }
        }
        proof { assert(!indexed(self.entries@, resource)); }
        Some(DeleteRequest { pool: self.pool.as_str().to_string(), target, _trace: self.trace })
    }
}

/// Consumed permission for one exact logical cleanup target, borrowing the original protection trace.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::logical::{DeleteRequest, Target};
/// /// A filesystem adapter cannot replace the permission's target.
/// fn substitute(request: &mut DeleteRequest<'_>) { request.target = Target::Root("other".into()); }
/// ```
///
/// ```compile_fail,E0502
/// use airlock_deployment_core::retention::{logical::{ResourceIndex, Target}, tracing::Trace};
/// /// New protection cannot be added while a permission still uses the old snapshot.
/// fn extend(trace: &mut Trace) {
///     let index = ResourceIndex::new("/pool".into(), trace).unwrap();
///     let request = index.reclaim(Target::Root("root".into())).unwrap();
///     trace.include_root("root".into());
///     let target = request.target();
///     let _ = target;
/// }
/// ```
pub struct DeleteRequest<'a> {
    // Original pool, never resupplied independently to the deleting adapter.
    pool: String,
    // Exact category and identity selected against the original trace.
    target: Target,
    // Borrowing excludes extending this trace while any resulting permission remains live.
    _trace: &'a Trace,
}

impl<'a> View for DeleteRequest<'a> {
    type V = Target;

    closed spec fn view(&self) -> Self::V { self.target }
}

impl<'a> DeleteRequest<'a> {
    /// Original pool forwarded unchanged to filesystem operations.
    pub closed spec fn pool_name(&self) -> Seq<char> { self.pool@ }

    /// Original protection snapshot against which absence was established.
    pub closed spec fn reference_set(&self) -> Seq<Resource> { self._trace@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self.pool@.len() > 0 && identity(subject(self.target)).len() > 0
            && !super::tracing::contains(self._trace@, subject(self.target))
    }

    /// Preserve the original checked pool identity.
    pub fn pool(&self) -> (out: &str) ensures out@ == self.pool_name(),
    { self.pool.as_str() }

    /// Expose only an immutable reference to the exact original target and resource category.
    pub fn target(&self) -> (out: &Target) ensures *out == self@,
    { &self.target }
}

}

#[cfg(test)]
mod tests {
    #[test]
    fn known_collision_uses_one_bucket_for_two_distinct_valid_digest_names() {
        let first = format!("{prefix}ae0ea", prefix = "a".repeat(59));
        let second = format!("{prefix}ba6ab", prefix = "a".repeat(59));
        assert_ne!(first, second);
        assert_eq!(super::bucket(&first), super::bucket(&second));
    }
}
