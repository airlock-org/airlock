//! Immutable store projection and preservation of writable VM storage.
//!
//! Generation admission and Qubes observation are external evidence. These decisions
//! do not grant deployment authority: reviewed callers still need their durable
//! storage request, while image construction and maintenance have separate policies.
//! JSON encoding, physical paths, Qubes locking and faithful execution remain adapter
//! responsibilities. Preservation concerns backing identities and definitions, not
//! a proof that the external storage driver preserves every byte. The preservation
//! snapshot belongs to one attempt; it is not a durable ownership ledger across
//! crashes. A later attempt observes backing identities again through Qubes.

use crate::inventory::PowerState;
use vstd::prelude::*;

verus! {

/// Only immutable software slots can occur in an attachment request.
#[derive(Clone, Copy)]
pub enum Slot {
    /// Guest system metadata, normally exposed as xvde.
    Store,
    /// Guest system external data, normally exposed as xvdf.
    StoreData,
    /// Persistent builder base metadata, normally exposed as xvdg.
    BuilderStore,
    /// Persistent builder base external data, normally exposed as xvdh.
    BuilderData,
}

impl Slot {
    /// Decode supported immutable slots; writable and unknown slots are rejected.
    pub fn parse(name: &str) -> (out: Option<Self>)
        ensures out.is_some() == (name@ == "store"@ || name@ == "store-data"@
            || name@ == "builder-store"@ || name@ == "builder-data"@),
            out.is_some() ==> slot_name(out.unwrap()) == name@,
    {
        if name == "store" { Some(Self::Store) }
        else if name == "store-data" { Some(Self::StoreData) }
        else if name == "builder-store" { Some(Self::BuilderStore) }
        else if name == "builder-data" { Some(Self::BuilderData) }
        else { None }
    }

    /// Supply the exact Qubes volume key without an independently supplied name.
    pub fn name(&self) -> (out: &str)
        ensures out@ == slot_name(*self),
    {
        match self {
            Self::Store => "store",
            Self::StoreData => "store-data",
            Self::BuilderStore => "builder-store",
            Self::BuilderData => "builder-data",
        }
    }
}

/// Mathematical names of the four permitted immutable volume slots.
pub open spec fn slot_name(slot: Slot) -> Seq<char> {
    match slot {
        Slot::Store => "store"@,
        Slot::StoreData => "store-data"@,
        Slot::BuilderStore => "builder-store"@,
        Slot::BuilderData => "builder-data"@,
    }
}

/// Physical backing identity within an upstream Qubes pool.
pub struct Backing {
    /// Pool containing the volume.
    pub pool: String,
    /// Driver-defined identity within that pool.
    pub vid: String,
}

/// Exact backing identity comparison, independent of frontend or VM naming.
pub open spec fn same_backing(a: Backing, b: Backing) -> bool {
    a.pool@ == b.pool@ && a.vid@ == b.vid@
}

fn backing_equal(a: &Backing, b: &Backing) -> (yes: bool)
    ensures yes == same_backing(*a, *b),
{ a.pool == b.pool && a.vid == b.vid }

/// Observed writable state; a canonical record retains all additional driver fields.
pub struct WritableVolume {
    /// Current pool and backing object.
    pub backing: Backing,
    /// The guest can write this volume.
    pub writable: bool,
    /// Changes survive VM shutdown.
    pub persistent: bool,
    /// A snapshot-derived volume cannot establish independent private ownership.
    pub snap_on_start: bool,
    /// A source volume would make ownership dependent on another backing object.
    pub source: Option<Backing>,
    /// Complete observed volume encoded by the adapter, including its persisted definition.
    pub record: String,
}

/// Independent persistent writable backing required for both root and private volumes.
pub open spec fn independent(v: WritableVolume) -> bool {
    v.backing.pool@.len() > 0 && v.backing.vid@.len() > 0
        && v.writable && v.persistent && !v.snap_on_start && v.source.is_none()
}

fn is_independent(v: &WritableVolume) -> (yes: bool)
    ensures yes == independent(*v),
{
    !v.backing.pool.is_empty() && !v.backing.vid.is_empty()
        && v.writable && v.persistent && !v.snap_on_start && v.source.is_none()
}

/// Neither writable backing is shared by another local volume or another VM.
pub open spec fn isolated(root: WritableVolume, private: WritableVolume, references: Seq<Backing>) -> bool {
    independent(root) && independent(private) && !same_backing(root.backing, private.backing)
        && forall|i: int| 0 <= i < references.len() ==>
            !same_backing(root.backing, #[trigger] references[i])
                && !same_backing(private.backing, references[i])
}

/// Check all supplied backing references, including source-volume aliases.
pub fn isolation(root: &WritableVolume, private: &WritableVolume, references: &[Backing]) -> (yes: bool)
    ensures yes == isolated(*root, *private, references@),
{
    if !is_independent(root) || !is_independent(private) || backing_equal(&root.backing, &private.backing) {
        return false;
    }
    let mut i = 0;
    while i < references.len()
        invariant 0 <= i <= references.len(), independent(*root), independent(*private),
            !same_backing(root.backing, private.backing),
            forall|j: int| 0 <= j < i ==> !same_backing(root.backing, #[trigger] references[j])
                && !same_backing(private.backing, references[j]),
        decreases references.len() - i,
    {
        if backing_equal(&root.backing, &references[i]) || backing_equal(&private.backing, &references[i]) {
            return false;
        }
        i += 1;
    }
    true
}

/// Decoded generation evidence. Loading and authenticating a committed manifest is external.
pub struct Generation {
    /// Namespace of the generation, checked against the intended VM.
    pub domain: String,
    /// Positive generation number within this namespace.
    pub number: u64,
    /// Immutable software closure identity.
    pub closure_hash: String,
    /// Manifest associated with this generation.
    pub manifest: String,
    /// Metadata image consumed by the immutable storage driver.
    pub metadata: String,
    /// Logical length of the metadata image.
    pub metadata_size: u64,
    /// Map selecting the external data visible to this generation.
    pub data: String,
    /// Logical external-data address space.
    pub data_size: u64,
}

/// Complete semantic volume definition passed to the wire adapter.
pub struct Definition {
    /// Permitted immutable Qubes slot; cannot name root or private.
    pub slot: Slot,
    /// Pool containing the immutable view.
    pub pool: String,
    /// Generation namespace; builder bases have their own namespace.
    pub domain: String,
    /// Positive generation selected for this view.
    pub generation: u64,
    /// Closure identity shared by both members of the pair.
    pub closure_hash: String,
    /// Manifest shared by both members of the pair.
    pub manifest: String,
    /// Metadata image or external-data map according to the slot.
    pub path: String,
    /// Image length or external-data address-space size.
    pub size: u64,
    /// Requested guest write access, always false for an admitted attachment.
    pub writable: bool,
    /// Immutable views retain their identity after VM shutdown.
    pub persistent: bool,
    /// Immutable views never request Qubes snapshot derivation.
    pub snap_on_start: bool,
}

/// Meaning of projecting one committed software generation into its two volume roles.
pub open spec fn projects(d: Definition, g: Generation, pool: Seq<char>, metadata: bool) -> bool {
    d.pool@ == pool && d.domain@ == g.domain@ && d.generation == g.number
        && d.closure_hash@ == g.closure_hash@ && d.manifest@ == g.manifest@
        && !d.writable && d.persistent && !d.snap_on_start
        && if metadata { d.slot is Store && d.path@ == g.metadata@ && d.size == g.metadata_size }
            else { d.slot is StoreData && d.path@ == g.data@ && d.size == g.data_size }
}

/// Checked system-generation projection; possession does not grant execution authority.
pub struct StorePair {
    // Exact generation whose fields are projected without later substitutions.
    generation: Generation,
    // Pool selected by the existing storage policy.
    pool: String,
}

impl View for StorePair {
    type V = (Generation, Seq<char>);

    closed spec fn view(&self) -> Self::V { (self.generation, self.pool@) }
}

impl StorePair {
    /// Refuse empty identities, invalid sizes and a generation belonging to another VM.
    pub fn new(target: &str, pool: String, generation: Generation) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap()@ == (generation, pool@)
            && generation.domain@ == target@ && target@.len() > 0 && pool@.len() > 0
            && generation.number > 0 && generation.metadata_size > 0 && generation.data_size > 0,
    {
        if target.is_empty() || pool.is_empty() || generation.domain.as_str() != target
            || generation.number == 0 || generation.metadata_size == 0 || generation.data_size == 0
            || generation.closure_hash.is_empty() || generation.manifest.is_empty()
            || generation.metadata.is_empty() || generation.data.is_empty() { return None; }
        Some(Self { generation, pool })
    }

    /// Produce exactly the metadata and data definitions for this generation, in boot order.
    pub fn definitions(&self) -> (out: Vec<Definition>)
        ensures out.len() == 2 && projects(out[0], self@.0, self@.1, true)
            && projects(out[1], self@.0, self@.1, false),
    {
        let g = &self.generation;
        vec![
            Definition { slot: Slot::Store, pool: self.pool.as_str().to_string(), domain: g.domain.as_str().to_string(),
                generation: g.number, closure_hash: g.closure_hash.as_str().to_string(), manifest: g.manifest.as_str().to_string(),
                path: g.metadata.as_str().to_string(), size: g.metadata_size, writable: false, persistent: true, snap_on_start: false },
            Definition { slot: Slot::StoreData, pool: self.pool.as_str().to_string(), domain: g.domain.as_str().to_string(),
                generation: g.number, closure_hash: g.closure_hash.as_str().to_string(), manifest: g.manifest.as_str().to_string(),
                path: g.data.as_str().to_string(), size: g.data_size, writable: false, persistent: true, snap_on_start: false },
        ]
    }
}

/// Every attachment is one coherent immutable pair, with no extra or writable effects.
pub open spec fn pair(target: Seq<char>, defs: Seq<Definition>) -> bool {
    defs.len() == 2 && defs[0].pool@.len() > 0 && defs[0].generation > 0
        && defs[0].domain@ == defs[1].domain@ && defs[0].pool@ == defs[1].pool@
        && defs[0].generation == defs[1].generation
        && defs[0].closure_hash@ == defs[1].closure_hash@ && defs[0].manifest@ == defs[1].manifest@
        && defs[0].closure_hash@.len() > 0 && defs[0].manifest@.len() > 0
        && (forall|i: int| 0 <= i < defs.len() ==> !defs[i].writable
            && defs[i].persistent && !defs[i].snap_on_start && #[trigger] defs[i].size > 0 && defs[i].path@.len() > 0)
        && ((defs[0].slot is Store && defs[1].slot is StoreData && defs[0].domain@ == target)
            || (defs[0].slot is BuilderStore && defs[1].slot is BuilderData
                && (target == "usr-assistant"@ || target == "sys-ctrl"@)
                && defs[0].domain@ == target + "-builder"@))
}

fn valid_pair(target: &str, defs: &[Definition]) -> (yes: bool)
    ensures yes == pair(target@, defs@),
{
    if defs.len() != 2 { return false; }
    let a = &defs[0];
    let b = &defs[1];
    if a.pool.is_empty() || a.generation == 0 || a.domain != b.domain || a.pool != b.pool
        || a.generation != b.generation || a.closure_hash != b.closure_hash || a.manifest != b.manifest
        || a.closure_hash.is_empty() || a.manifest.is_empty() { return false; }
    // Instantiate both members of the quantified specification before rejecting a bad pair.
    assert(pair(target@, defs@) ==> defs[0].size > 0 && defs[1].size > 0);
    if a.writable || !a.persistent || a.snap_on_start || a.size == 0 || a.path.is_empty()
        || b.writable || !b.persistent || b.snap_on_start || b.size == 0 || b.path.is_empty() {
        return false;
    }
    assert forall|j: int| 0 <= j < defs.len() implies !defs[j].writable && defs[j].persistent
        && !defs[j].snap_on_start && #[trigger] defs[j].size > 0 && defs[j].path@.len() > 0 by {
        assert(j == 0 || j == 1);
    }
    match (&a.slot, &b.slot) {
        (Slot::Store, Slot::StoreData) => a.domain.as_str() == target,
        (Slot::BuilderStore, Slot::BuilderData) => {
            if target != "usr-assistant" && target != "sys-ctrl" { return false; }
            let mut namespace = target.to_string();
            namespace.push_str("-builder");
            a.domain == namespace
        },
        _ => false,
    }
}

/// Root and private observations captured together before attachment.
pub struct WritablePair {
    /// Writable system-state backing that must survive installation.
    pub root: WritableVolume,
    /// User-data backing that must survive installation.
    pub private: WritableVolume,
}

/// Opaque mutation request retaining the writable state that must survive attachment.
pub struct AttachmentPlan {
    // Exact VM checked against both generation namespace and observed state by the adapter.
    target: String,
    // Root backing and complete definition captured before mutation.
    root: WritableVolume,
    // Private backing and complete definition captured before mutation.
    private: WritableVolume,
    // Complete immutable pair admitted by the checked constructor.
    definitions: Vec<Definition>,
}

impl View for AttachmentPlan {
    type V = (Seq<char>, WritableVolume, WritableVolume, Seq<Definition>);

    closed spec fn view(&self) -> Self::V { (self.target@, self.root, self.private, self.definitions@) }
}

impl AttachmentPlan {
    /// Require isolated writable backing, an explicitly halted standalone VM and a complete immutable pair.
    pub fn new(target: String, klass: &str, power: PowerState, running: bool,
        writable: WritablePair, references: &[Backing], definitions: Vec<Definition>) -> (out: Option<Self>)
        ensures out.is_some() ==> klass@ == "StandaloneVM"@ && power@ is Halted && !running
            && isolated(writable.root, writable.private, references@) && pair(target@, definitions@),
            out.is_some() ==> out.unwrap()@ == (target@, writable.root, writable.private, definitions@),
    {
        let root = writable.root;
        let private = writable.private;

        if klass != "StandaloneVM" || !power.is_halted() || running
            || !isolation(&root, &private, references) || !valid_pair(target.as_str(), &definitions) { return None; }
        Some(Self { target, root, private, definitions })
    }

    /// Exact target selected by the attachment checks.
    pub fn target(&self) -> (out: &str)
        ensures out@ == self@.0,
    { self.target.as_str() }

    /// Borrow the checked complete pair; callers cannot mutate it through this request.
    pub fn definitions(&self) -> (out: &[Definition])
        ensures out@ == self@.3,
    { &self.definitions }

    /// Compare the complete root/private observations after external installation.
    pub fn preserves(&self, root: &WritableVolume, private: &WritableVolume) -> (yes: bool)
        ensures yes == (same_backing(self@.1.backing, root.backing)
            && same_backing(self@.2.backing, private.backing)
            && self@.1.record@ == root.record@ && self@.2.record@ == private.record@
            && independent(*root) && independent(*private)),
    {
        backing_equal(&self.root.backing, &root.backing) && backing_equal(&self.private.backing, &private.backing)
            && self.root.record == root.record && self.private.record == private.record
            && is_independent(root) && is_independent(private)
    }
}

}
