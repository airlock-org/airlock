//! Trace immutable software references without losing identities or resource categories.
//! Decoding and choosing source records are adapter duties. Each successful extension
//! preserves exactly the prior requirements and every reference in its supplied source.

#[cfg(verus_keep_ghost)]
use super::inventory::{advance, exists_in};

use vstd::prelude::*;

verus! {

broadcast use vstd::seq_lib::group_seq_properties;

/// References carried by one retained generation, decoded before cleanup starts.
pub struct Manifest {
    /// Complete Nix object basenames, excluding the shared tar terminator.
    pub objects: Vec<String>,
    /// Canonical directory containing this generation's immutable view.
    pub view: String,
    /// Immutable root artifact digest, absent only for a store-only generation.
    pub root: Option<String>,
}

/// Complete decoded logical inputs for one tracing pass.
pub struct Sources {
    /// Every retained or recoverable generation's complete manifest references.
    pub manifests: Vec<Manifest>,
    /// Objects owned by unfinished transactions, including work without a generation manifest.
    pub pending_objects: Vec<String>,
}

/// Logical requirements are the union of manifest references and pending object identities.
pub open spec fn sources_require(s: Sources, r: Resource) -> bool {
    exists_in(s.manifests@, |m: Manifest| manifest_requires(m, r))
        || match r { Resource::Object(n) => exists_in(s.pending_objects@, |p: String| p@ == n@), _ => false }
}

/// All decoded sources must be interpretable before their aggregate may be used.
pub open spec fn valid_sources(s: Sources) -> bool {
    (forall|i: int| 0 <= i < s.manifests.len() ==> valid_manifest(#[trigger] s.manifests[i]))
        && (forall|i: int| 0 <= i < s.pending_objects.len() ==> #[trigger] s.pending_objects[i]@.len() > 0)
}

/// One physical range pinned by a canonical view or an active consumer.
pub struct Extent {
    /// Exact segment identity; offsets never identify a different segment.
    pub segment: String,
    /// First physical byte in the segment.
    pub offset: u64,
    /// Positive length, with no overflow at the exclusive end.
    pub length: u64,
}

/// Different storage namespaces remain distinct even when their names coincide.
pub enum Resource {
    /// Admitted Nix object metadata and its canonical bytes.
    Object(
        /// Exact pool object basename.
        String,
    ),
    /// Immutable view directory, including its metadata image and blob map.
    View(
        /// Exact canonical directory.
        String,
    ),
    /// Immutable root disk artifact directory.
    Root(
        /// Exact content digest.
        String,
    ),
    /// Physical segment file containing one or more required ranges.
    Segment(
        /// Exact segment identifier.
        String,
    ),
}

/// Equality preserves both the resource category and its full identity.
pub open spec fn same(a: Resource, b: Resource) -> bool {
    match (a, b) {
        (Resource::Object(x), Resource::Object(y))
        | (Resource::View(x), Resource::View(y))
        | (Resource::Root(x), Resource::Root(y))
        | (Resource::Segment(x), Resource::Segment(y)) => x@ == y@,
        _ => false,
    }
}

/// Set membership is independent of incidental order and duplicate references.
pub open spec fn contains(rs: Seq<Resource>, r: Resource) -> bool {
    exists|i: int| 0 <= i < rs.len() && same(#[trigger] rs[i], r)
}

/// Every named object, the shared terminator, the view, and any root are required.
pub open spec fn manifest_requires(m: Manifest, r: Resource) -> bool {
    match r {
        Resource::Object(name) => name@ == ".tar-eof"@
            || exists|i: int| 0 <= i < m.objects.len() && #[trigger] m.objects[i]@ == name@,
        Resource::View(name) => name@ == m.view@,
        Resource::Root(name) => m.root.is_some() && name@ == m.root.unwrap()@,
        Resource::Segment(_) => false,
    }
}

/// Pinning any valid range requires retaining its entire segment file.
pub open spec fn extent_requires(es: Seq<Extent>, r: Resource) -> bool {
    match r {
        Resource::Segment(name) => exists|i: int| 0 <= i < es.len() && #[trigger] es[i].segment@ == name@,
        _ => false,
    }
}

/// Empty references cannot stand for an omitted protected resource.
pub open spec fn valid_manifest(m: Manifest) -> bool {
    m.view@.len() > 0 && m.objects.len() > 0
        && (m.root.is_some() ==> m.root.unwrap()@.len() > 0)
        && forall|i: int| 0 <= i < m.objects.len() ==> #[trigger] m.objects[i]@.len() > 0
}

/// Invalid or overflowing ranges refuse the whole extension before it changes the trace.
pub open spec fn valid_extents(es: Seq<Extent>) -> bool {
    forall|i: int| 0 <= i < es.len() ==> #[trigger] es[i].segment@.len() > 0
        && es[i].length > 0 && es[i].offset + es[i].length <= u64::MAX
}

proof fn push_membership(rs: Seq<Resource>, p: Resource)
    ensures forall|r: Resource| #[trigger] contains(rs.push(p), r) == (contains(rs, r) || same(p, r)),
{
    assert forall|r: Resource| #[trigger] contains(rs.push(p), r) == (contains(rs, r) || same(p, r)) by {
        if contains(rs.push(p), r) {
            let i = choose|i: int| 0 <= i < rs.push(p).len() && same(rs.push(p)[i], r);
            if i < rs.len() { assert(same(rs[i], r)); }
            else { assert(i == rs.len()); }
        }
        if contains(rs, r) {
            let i = choose|i: int| 0 <= i < rs.len() && same(rs[i], r);
            assert(same(rs.push(p)[i], r));
        }
        if same(p, r) { assert(same(rs.push(p)[rs.len() as int], r)); }
    }
}

/// Resource trace with private reference storage; checked extensions preserve prior requirements.
///
/// ```compile_fail,E0616
/// use airlock_deployment_core::retention::tracing::Trace;
/// /// Cleanup cannot replace traced references with an empty inventory.
/// fn discard(trace: &mut Trace) { trace.references.clear(); }
/// ```
pub struct Trace {
    // Every reference contributed by successful source extensions; duplicates are harmless.
    references: Vec<Resource>,
}

impl View for Trace {
    type V = Seq<Resource>;

    closed spec fn view(&self) -> Self::V { self.references@ }
}

impl Trace {
    /// Begin an empty pass before any retained source has been interpreted.
    pub fn new() -> (out: Self) ensures out@.len() == 0,
    { Self { references: Vec::new() } }

    /// Trace every supplied manifest and pending object; invalid inputs refuse the entire pass.
    pub fn from_sources(s: &Sources) -> (out: Option<Self>)
        ensures out.is_some() == valid_sources(*s),
            out.is_some() ==> forall|r: Resource| #[trigger] contains(out.unwrap()@, r) == sources_require(*s, r),
    {
        let mut trace = Self::new();
        let mut i = 0;
        while i < s.manifests.len()
            invariant 0 <= i <= s.manifests.len(),
                forall|j: int| 0 <= j < i ==> valid_manifest(#[trigger] s.manifests[j]),
                forall|r: Resource| #[trigger] contains(trace@, r)
                    == exists_in(s.manifests@.take(i as int), |m: Manifest| manifest_requires(m, r)),
            decreases s.manifests.len() - i,
        {
            if !trace.include_manifest(&s.manifests[i]) { return None; }
            proof {
                assert forall|r: Resource| #[trigger] contains(trace@, r)
                    == exists_in(s.manifests@.take(i as int + 1), |m: Manifest| manifest_requires(m, r))
                by { advance(s.manifests@, i as int, |m: Manifest| manifest_requires(m, r)); }
            }
            i += 1;
        }
        let mut i = 0;
        while i < s.pending_objects.len()
            invariant 0 <= i <= s.pending_objects.len(),
                forall|j: int| 0 <= j < s.manifests.len() ==> valid_manifest(#[trigger] s.manifests[j]),
                forall|j: int| 0 <= j < i ==> #[trigger] s.pending_objects[j]@.len() > 0,
                forall|r: Resource| #[trigger] contains(trace@, r)
                    == (exists_in(s.manifests@, |m: Manifest| manifest_requires(m, r))
                        || match r { Resource::Object(n) => exists_in(s.pending_objects@.take(i as int), |p: String| p@ == n@), _ => false }),
            decreases s.pending_objects.len() - i,
        {
            if !trace.include_object(s.pending_objects[i].as_str().to_string()) { return None; }
            proof {
                assert forall|r: Resource| #[trigger] contains(trace@, r)
                    == (exists_in(s.manifests@, |m: Manifest| manifest_requires(m, r))
                        || match r { Resource::Object(n) => exists_in(s.pending_objects@.take(i as int + 1), |p: String| p@ == n@), _ => false })
                by { if let Resource::Object(n) = r { advance(s.pending_objects@, i as int, |p: String| p@ == n@); } }
            }
            i += 1;
        }
        Some(trace)
    }

    fn add(&mut self, r: Resource)
        ensures forall|p: Resource| #[trigger] contains(final(self)@, p) == (contains(old(self)@, p) || same(r, p)),
    {
        let ghost before = self.references@;
        self.references.push(r);
        proof { push_membership(before, r); }
    }

    /// Preserve every prior requirement and every reference in this manifest, or refuse unchanged.
    pub fn include_manifest(&mut self, m: &Manifest) -> (ok: bool)
        ensures ok == valid_manifest(*m),
            !ok ==> final(self)@ == old(self)@,
            ok ==> forall|r: Resource| #[trigger] contains(final(self)@, r)
                == (contains(old(self)@, r) || manifest_requires(*m, r)),
    {
        if m.view.is_empty() || m.objects.is_empty() { return false; }
        let valid_root = match &m.root { Some(root) => !root.is_empty(), None => true };
        if !valid_root { return false; }
        let mut i = 0;
        while i < m.objects.len()
            invariant 0 <= i <= m.objects.len(), self@ == old(self)@,
                forall|j: int| 0 <= j < i ==> #[trigger] m.objects[j]@.len() > 0,
            decreases m.objects.len() - i,
        {
            if m.objects[i].is_empty() { return false; }
            i += 1;
        }

        let ghost before = self@;
        let mut i = 0;
        while i < m.objects.len()
            invariant 0 <= i <= m.objects.len(), valid_manifest(*m),
                forall|r: Resource| #[trigger] contains(self@, r) == (contains(before, r)
                    || match r { Resource::Object(n) => exists|j: int| 0 <= j < i && #[trigger] m.objects[j]@ == n@, _ => false }),
            decreases m.objects.len() - i,
        {
            self.add(Resource::Object(m.objects[i].as_str().to_string()));
            proof {
                assert forall|r: Resource| #[trigger] contains(self@, r) == (contains(before, r)
                    || match r { Resource::Object(n) => exists|j: int| 0 <= j < i + 1 && #[trigger] m.objects[j]@ == n@, _ => false }) by {
                    if let Resource::Object(n) = r {
                        if exists|j: int| 0 <= j < i + 1 && #[trigger] m.objects[j]@ == n@ {
                            let j = choose|j: int| 0 <= j < i + 1 && m.objects[j]@ == n@;
                            if j < i { assert(exists|k: int| 0 <= k < i && #[trigger] m.objects[k]@ == n@); }
                            else { assert(j == i); }
                        }
                    }
                }
            }
            i += 1;
        }
        self.add(Resource::Object(".tar-eof".to_string()));
        self.add(Resource::View(m.view.as_str().to_string()));
        if let Some(root) = &m.root { self.add(Resource::Root(root.as_str().to_string())); }
        true
    }

    /// Preserve a pending transaction's admitted object without inventing view or root references.
    pub fn include_object(&mut self, name: String) -> (ok: bool)
        ensures ok == (name@.len() > 0), !ok ==> final(self)@ == old(self)@,
            ok ==> forall|r: Resource| #[trigger] contains(final(self)@, r)
                == (contains(old(self)@, r) || same(Resource::Object(name), r)),
    {
        if name.is_empty() { return false; }
        self.add(Resource::Object(name));
        true
    }

    /// Retain a surviving manifest's root even when it has no usable store view.
    pub fn include_root(&mut self, name: String) -> (ok: bool)
        ensures ok == (name@.len() > 0), !ok ==> final(self)@ == old(self)@,
            ok ==> forall|r: Resource| #[trigger] contains(final(self)@, r)
                == (contains(old(self)@, r) || same(Resource::Root(name), r)),
    {
        if name.is_empty() { return false; }
        self.add(Resource::Root(name));
        true
    }

    /// Preserve all physical segment identities, including old locations pinned by running VMs.
    pub fn include_extents(&mut self, es: &[Extent]) -> (ok: bool)
        ensures ok == valid_extents(es@), !ok ==> final(self)@ == old(self)@,
            ok ==> forall|r: Resource| #[trigger] contains(final(self)@, r)
                == (contains(old(self)@, r) || extent_requires(es@, r)),
    {
        let mut i = 0;
        while i < es.len()
            invariant 0 <= i <= es.len(), self@ == old(self)@,
                forall|j: int| 0 <= j < i ==> #[trigger] es[j].segment@.len() > 0
                    && es[j].length > 0 && es[j].offset + es[j].length <= u64::MAX,
            decreases es.len() - i,
        {
            if es[i].segment.is_empty() || es[i].length == 0 || es[i].offset > u64::MAX - es[i].length { return false; }
            i += 1;
        }

        let ghost before = self@;
        let mut i = 0;
        while i < es.len()
            invariant 0 <= i <= es.len(), valid_extents(es@),
                forall|r: Resource| #[trigger] contains(self@, r)
                    == (contains(before, r) || extent_requires(es@.take(i as int), r)),
            decreases es.len() - i,
        {
            self.add(Resource::Segment(es[i].segment.as_str().to_string()));
            proof {
                assert forall|r: Resource| #[trigger] contains(self@, r)
                    == (contains(before, r) || extent_requires(es@.take(i as int + 1), r)) by {
                    if let Resource::Segment(n) = r {
                        if extent_requires(es@.take(i as int + 1), r) {
                            let j = choose|j: int| 0 <= j < i + 1 && es[j].segment@ == n@;
                            if j < i { assert(es@.take(i as int)[j].segment@ == n@); }
                            else { assert(j == i); }
                        }
                        if extent_requires(es@.take(i as int), r) {
                            let j = choose|j: int| 0 <= j < i && es[j].segment@ == n@;
                            assert(es@.take(i as int + 1)[j].segment@ == n@);
                        }
                        if es[i as int].segment@ == n@ { assert(es@.take(i as int + 1)[i as int].segment@ == n@); }
                    }
                }
            }
            i += 1;
        }
        true
    }

    /// Read the full traced set for indexed filesystem lookups; no mutable access is exposed.
    pub fn references(&self) -> (out: &[Resource]) ensures out@ == self@,
    { &self.references }
}

}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}
