//! Select a confirmed removable disk and preserve exact source/destination identities before a write.
//! Cargo executes the Rust bodies checked by Verus. Filesystem/kernel observations, hashing, private
//! pipe ownership and faithful I/O remain adapters; these contracts prove neither durable completion
//! nor exactly-once provisioning. Explicit direct and published-image commands retain their manual policy.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use vstd::prelude::*;
use vstd::string::StrSliceExecFns;

verus! {

broadcast use vstd::string::group_string_axioms;

/// A physical disk incarnation; a reused kernel name or device number is insufficient.
pub struct Disk {
    /// Canonical kernel device path retained from selection.
    pub name: String,
    /// Kernel block-device number.
    pub number: u64,
    /// Kernel disk sequence, changed when a disk incarnation is replaced.
    pub sequence: u64,
    /// Exact hardware serial independently confirmed by the user.
    pub serial: String,
    /// Observed whole-device capacity.
    pub bytes: u64,
}

/// Fresh disk facts, including whether all observed descendants are unused.
pub struct Observation {
    /// Exact observed disk incarnation.
    pub disk: Disk,
    /// The reported transport is USB.
    pub usb: bool,
    /// The selected object is a whole disk.
    pub whole: bool,
    /// Kernel write protection is disabled.
    pub writable: bool,
    /// No mounted filesystem or swap was found on this disk or its descendants.
    pub unmounted: bool,
    /// No active device holders were found on this disk or its descendants.
    pub unheld: bool,
}

/// Descriptor identity and read-only queries through its pinned device node.
/// The external adapter supplies these facts; this type does not assert an atomic observation.
pub struct OpenedDisk {
    /// Device number obtained from the destination descriptor.
    pub number: u64,
    /// Disk sequence queried through the pinned destination's device node.
    pub sequence: u64,
    /// Capacity queried through that same pinned device node.
    pub bytes: u64,
}

/// Stable metadata of the opened image file.
#[derive(Clone, Copy)]
pub struct ImageVersion {
    /// Source filesystem device number.
    pub device: u64,
    /// Source inode retained by the open descriptor.
    pub inode: u64,
    /// Complete logical image size.
    pub bytes: u64,
    /// Source modification time in seconds.
    pub modified: i64,
    /// Nanosecond component of modification time.
    pub modified_ns: i64,
    /// Source change time in seconds.
    pub changed: i64,
    /// Nanosecond component of change time.
    pub changed_ns: i64,
}

/// Source and output binding of one newly allocated build run.
pub struct BuildRun {
    /// Private result-link path allocated for this run, not the mutable global latest link.
    pub result_link: String,
    /// Original application source revision.
    pub revision: String,
    /// Original machine-authoring source revision.
    pub machine_revision: String,
}

/// An external build's observed result; cancellation is not a successful build.
#[derive(Clone, Copy)]
pub enum BuildOutcome {
    /// The child build exited successfully.
    Success,
    /// Failure, cancellation, missing result or uncertain completion.
    Failed,
}

/// Build completion facts supplied by the owner of this run's private pipe.
pub struct BuildObservation {
    /// Actual child outcome, or the outcome received on the authenticated private pipe.
    pub outcome: BuildOutcome,
    /// Exact private result link observed at completion.
    pub result_link: String,
    /// Application revision reobserved at completion.
    pub revision: String,
    /// Machine revision reobserved at completion.
    pub machine_revision: String,
    /// Newly published immutable image output; absence never grants a write.
    pub output: Option<String>,
}

/// Inspection is distinct from an explicit destructive command.
#[derive(Clone, Copy)]
pub enum Intent {
    /// Read-only inspection.
    Inspect,
    /// Explicit user request to replace this disk's contents.
    Write,
}

/// Sparse writes do not erase holes; verified writes always materialize the complete image.
#[derive(Clone, Copy)]
pub enum CopyMode {
    /// Copy allocated source extents only.
    Sparse,
    /// Materialize every byte without full read-back verification.
    Full,
    /// Materialize and verify the entire image prefix.
    Verified,
}

/// Complete physical target identity, independent of display names or collection ordering.
pub open spec fn same_disk(a: Disk, b: Disk) -> bool {
    a.name@ == b.name@ && a.number == b.number && a.sequence == b.sequence
        && a.serial@ == b.serial@ && a.bytes == b.bytes
}

/// Source metadata remains unchanged across hashing and immediately before dispatch.
pub open spec fn same_image(a: ImageVersion, b: ImageVersion) -> bool {
    a.device == b.device && a.inode == b.inode && a.bytes == b.bytes
        && a.modified == b.modified && a.modified_ns == b.modified_ns
        && a.changed == b.changed && a.changed_ns == b.changed_ns
}

/// Source and private output coordinate remain tied to one run.
pub open spec fn same_run(a: BuildRun, b: BuildRun) -> bool {
    a.result_link@ == b.result_link@ && a.revision@ == b.revision@
        && a.machine_revision@ == b.machine_revision@
}

fn disk_eq(a: &Disk, b: &Disk) -> (yes: bool) ensures yes == same_disk(*a, *b),
{ a.name == b.name && a.number == b.number && a.sequence == b.sequence && a.serial == b.serial && a.bytes == b.bytes }

fn image_eq(a: &ImageVersion, b: &ImageVersion) -> (yes: bool) ensures yes == same_image(*a, *b),
{
    a.device == b.device && a.inode == b.inode && a.bytes == b.bytes
        && a.modified == b.modified && a.modified_ns == b.modified_ns
        && a.changed == b.changed && a.changed_ns == b.changed_ns
}

fn run_eq(a: &BuildRun, b: &BuildRun) -> (yes: bool) ensures yes == same_run(*a, *b),
{ a.result_link == b.result_link && a.revision == b.revision && a.machine_revision == b.machine_revision }

/// Only eligible whole, writable USB disks can enter the selection chain.
pub open spec fn eligible(o: Observation) -> bool {
    o.usb && o.whole && o.writable && o.disk.sequence > 0 && o.disk.bytes > 0
        && o.disk.name@.len() > 0 && o.disk.serial@.len() > 0
}

/// Initial selection permits mounted disks to be unmounted later, but grants no write by itself.
pub struct SelectedUsb {
    // Original physical target; callers cannot replace it after selection.
    disk: Disk,
    // Erased independent selection evidence.
    _observed: Ghost<Observation>,
}

impl SelectedUsb {
    /// Original physical target.
    pub closed spec fn target(&self) -> Disk { self.disk }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool { eligible(self._observed@) && same_disk(self.disk, self._observed@.disk) }

    /// Refuse ineligible hardware before accepting independent serial confirmation.
    pub fn check(observed: Observation) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().target() == observed.disk,
    {
        if !observed.usb || !observed.whole || !observed.writable || observed.disk.sequence == 0
            || observed.disk.bytes == 0 || observed.disk.name.is_empty() || observed.disk.serial.is_empty() { return None; }
        Some(Self { disk: observed.disk, _observed: Ghost(observed) })
    }

    /// Inspection and a different or absent serial cannot create destructive intent.
    pub fn confirm(self, intent: Intent, serial: &str) -> (out: Option<ConfirmedUsb>)
        ensures out.is_some() ==> out.unwrap().selected() == self && intent is Write && serial@ == self.target().serial@,
    {
        if !matches!(intent, Intent::Write) || serial != self.disk.serial.as_str() { return None; }
        Some(ConfirmedUsb { selected: self })
    }
}

/// Exact hardware confirmation, separate from image/build authorisation.
pub struct ConfirmedUsb {
    // Original eligible disk selected before confirmation or a long build.
    selected: SelectedUsb,
}

impl ConfirmedUsb {
    /// Original eligible selection.
    pub closed spec fn selected(&self) -> SelectedUsb { self.selected }

    /// Explicit standalone writing requires the independently supplied image digest.
    pub fn direct(self, digest: String) -> (out: Option<Authority>)
        ensures out.is_some() ==> out.unwrap().confirmed() == self && out.unwrap().policy() == Policy::Digest(digest),
    {
        if !digest.as_str().is_ascii() || digest.as_str().len() != 64 { return None; }
        Some(Authority { confirmed: self, policy: Policy::Digest(digest) })
    }

    /// Explicit standalone provisioning may select an already published output without claiming a new build.
    pub fn published(self, output: String) -> (out: Option<Authority>)
        ensures out.is_some() ==> out.unwrap().confirmed() == self && out.unwrap().policy() == Policy::Published(output),
    {
        if output.is_empty() { return None; }
        Some(Authority { confirmed: self, policy: Policy::Published(output) })
    }

    /// Consume confirmation while waiting; there is no destructive request yet.
    pub fn wait(self, run: BuildRun) -> (out: WaitingWrite)
        ensures out.confirmed() == self && out.run() == run,
    { WaitingWrite { confirmed: self, run } }
}

/// Successful same-run publication, built from complete source/output facts rather than a bare success flag.
pub struct CompletedBuild {
    // Exact original build run.
    run: BuildRun,
    // Exact newly published immutable output.
    output: String,
    // Erased original external completion evidence.
    _observed: Ghost<BuildObservation>,
}

impl CompletedBuild {
    /// Exact original run for composition with the waiting writer.
    pub closed spec fn binding(&self) -> BuildRun { self.run }

    /// Exact newly published output.
    pub closed spec fn publication(&self) -> Seq<char> { self.output@ }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        self._observed@.outcome is Success && self._observed@.output is Some
            && self._observed@.output.unwrap()@ == self.output@ && self.output@.len() > 0
            && self.run.result_link@ == self._observed@.result_link@
            && self.run.revision@ == self._observed@.revision@
            && self.run.machine_revision@ == self._observed@.machine_revision@
    }

    /// Refuse failed/cancelled builds, missing publications, changed source or another run's result.
    pub fn check(run: BuildRun, observed: BuildObservation) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().binding() == run,
    {
        if !matches!(observed.outcome, BuildOutcome::Success) || run.result_link != observed.result_link
            || run.revision != observed.revision || run.machine_revision != observed.machine_revision { return None; }
        let output = observed.output?;
        if output.is_empty() { return None; }
        Some(Self { run, output, _observed: Ghost(observed) })
    }

    /// Forward the same run into the pipe receipt without reconstructing its identity.
    pub fn run(&self) -> (out: &BuildRun) ensures *out == self.binding(),
    { &self.run }

    /// Forward only this successful run's exact output into the pipe receipt.
    pub fn output(&self) -> (out: &str) ensures out@ == self.publication(),
    { self.output.as_str() }
}

/// A long build retains the already confirmed physical disk and original run together.
pub struct WaitingWrite {
    // Original confirmed disk, not a new selection after the build.
    confirmed: ConfirmedUsb,
    // Original private run and source identities.
    run: BuildRun,
}

impl WaitingWrite {
    /// Original hardware confirmation.
    pub closed spec fn confirmed(&self) -> ConfirmedUsb { self.confirmed }

    /// Original build binding.
    pub closed spec fn run(&self) -> BuildRun { self.run }

    /// Only completion of this exact run can authorize the previously confirmed target.
    pub fn complete(self, build: CompletedBuild) -> (out: Option<Authority>)
        ensures out.is_some() ==> out.unwrap().confirmed() == self.confirmed()
            && out.unwrap().policy() == Policy::Build(build) && same_run(self.run(), build.binding()),
    {
        if !run_eq(&self.run, &build.run) { return None; }
        Some(Authority { confirmed: self.confirmed, policy: Policy::Build(build) })
    }
}

/// Independent image policy; manual commands cannot masquerade as completion of a waiting build.
pub enum Policy {
    /// Explicit independently supplied digest.
    Digest(
        /// Exact expected image content digest.
        String,
    ),
    /// Explicit standalone selection of an already published output.
    Published(
        /// Exact selected immutable output.
        String,
    ),
    /// Successful publication from an already confirmed waiting run.
    Build(
        /// Opaque same-run completion evidence.
        CompletedBuild,
    ),
}

/// An opened image whose metadata remained stable while its actual bytes were hashed.
pub struct CheckedImage {
    // Original stable descriptor metadata.
    version: ImageVersion,
    // Digest read from that opened descriptor, not a reopened path.
    digest: String,
    // Published output supplying this image, absent for a direct file.
    publication: Option<String>,
    // Erased independent image observations.
    _before: Ghost<ImageVersion>,
    // Erased header and file-type observations.
    _regular: Ghost<bool>,
    // Erased observation of the source GPT header.
    _gpt: Ghost<bool>,
}

impl CheckedImage {
    /// Original opened-file identity.
    pub closed spec fn version(&self) -> ImageVersion { self.version }

    /// Actual opened-file content digest.
    pub closed spec fn digest(&self) -> Seq<char> { self.digest@ }

    /// Exact supplying output when selected from a publication.
    pub closed spec fn publication(&self) -> Option<String> { self.publication }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        same_image(self.version, self._before@) && self.version.bytes > 1024 * 1024
            && self.digest@.len() == 64 && self._regular@ && self._gpt@
    }

    /// Refuse changed or implausible images, non-regular sources and a missing GPT header.
    pub fn check(before: ImageVersion, after: ImageVersion, digest: String, publication: Option<String>, regular: bool, gpt: bool) -> (out: Option<Self>)
        ensures out.is_some() ==> out.unwrap().version() == after && out.unwrap().digest() == digest@ && out.unwrap().publication() == publication,
    {
        let text = digest.as_str();
        if !regular || !gpt || before.bytes <= 1024 * 1024 || !text.is_ascii() || text.unicode_len() != 64 || !image_eq(&before, &after) { return None; }
        Some(Self { version: after, digest, publication, _before: Ghost(before), _regular: Ghost(regular), _gpt: Ghost(gpt) })
    }
}

/// Image policy must match actual opened bytes or the exact supplying publication.
pub open spec fn image_allowed(policy: Policy, image: CheckedImage) -> bool {
    match policy {
        Policy::Digest(expected) => expected@ == image.digest(),
        Policy::Published(output) => image.publication() is Some && image.publication().unwrap()@ == output@,
        Policy::Build(build) => image.publication() is Some && image.publication().unwrap()@ == build.publication(),
    }
}

fn allowed(policy: &Policy, image: &CheckedImage) -> (yes: bool) ensures yes == image_allowed(*policy, *image),
{
    match policy {
        Policy::Digest(expected) => expected.as_str() == image.digest.as_str(),
        Policy::Published(output) => match &image.publication { Some(actual) => actual.as_str() == output.as_str(), None => false },
        Policy::Build(build) => match &image.publication { Some(actual) => actual.as_str() == build.output(), None => false },
    }
}

/// Original target confirmation and image policy; no public constructor or mutable fields.
pub struct Authority {
    // Exact confirmed physical target.
    confirmed: ConfirmedUsb,
    // Original manual digest/publication or successful waiting-run policy.
    policy: Policy,
}

impl Authority {
    /// Original hardware confirmation.
    pub closed spec fn confirmed(&self) -> ConfirmedUsb { self.confirmed }

    /// Original immutable image policy.
    pub closed spec fn policy(&self) -> Policy { self.policy }

    /// Refuse a substituted physical target before even preparatory unmounting.
    pub fn matches_disk(&self, observed: &Observation) -> (yes: bool)
        ensures yes == same_disk(self.confirmed().selected().target(), observed.disk),
    { disk_eq(&self.confirmed.selected.disk, &observed.disk) }

    /// Check the actual image against its original digest/publication policy before preparatory effects.
    pub fn accepts(&self, image: &CheckedImage) -> (yes: bool)
        ensures yes == image_allowed(self.policy(), *image),
    { allowed(&self.policy, image) }

    /// Require fresh unused hardware, the exact opened destination and unchanged checked source before dispatch.
    pub fn request(self, image: CheckedImage, current: &Observation, opened: &OpenedDisk, source: &ImageVersion, mode: CopyMode) -> (out: Option<WriteRequest>)
        ensures out.is_some() ==> out.unwrap().authority() == self && out.unwrap().image() == image && out.unwrap().mode() == mode,
    {
        if !disk_eq(&self.confirmed.selected.disk, &current.disk) || !current.usb || !current.whole || !current.writable
            || !current.unmounted || !current.unheld || opened.number != current.disk.number
            || opened.sequence != current.disk.sequence || opened.bytes != current.disk.bytes
            || image.version.bytes > current.disk.bytes || !image_eq(&image.version, source) || !allowed(&self.policy, &image) { return None; }
        Some(WriteRequest { authority: self, image, mode, _current: Ghost(*current), _opened: Ghost(*opened), _source: Ghost(*source) })
    }
}

/// Exact permitted first-write request, consumed by the file-descriptor-owning adapter.
///
/// ```compile_fail,E0616
/// use airlock_provisioning_core::{WriteRequest, CopyMode};
/// /// A caller cannot change a selected request's write/verification mode.
/// fn substitute(request: &mut WriteRequest) { request.mode = CopyMode::Sparse; }
/// ```
pub struct WriteRequest {
    // Original physical target and image policy.
    authority: Authority,
    // Original checked source descriptor identity and digest.
    image: CheckedImage,
    // Original copy/read-back policy.
    mode: CopyMode,
    // Erased fresh unused-disk observations.
    _current: Ghost<Observation>,
    // Erased opened-descriptor observations.
    _opened: Ghost<OpenedDisk>,
    // Erased immediately rechecked source metadata.
    _source: Ghost<ImageVersion>,
}

impl WriteRequest {
    /// Original target and image policy.
    pub closed spec fn authority(&self) -> Authority { self.authority }

    /// Original stable image evidence.
    pub closed spec fn image(&self) -> CheckedImage { self.image }

    /// Original requested copy mode.
    pub closed spec fn mode(&self) -> CopyMode { self.mode }

    #[verifier::type_invariant]
    closed spec fn valid(&self) -> bool {
        same_disk(self.authority.confirmed.selected.disk, self._current@.disk)
            && self._current@.usb && self._current@.whole && self._current@.writable && self._current@.unmounted && self._current@.unheld
            && self._opened@.number == self._current@.disk.number && self._opened@.sequence == self._current@.disk.sequence
            && self._opened@.bytes == self._current@.disk.bytes && self.image.version.bytes <= self._current@.disk.bytes
            && same_image(self.image.version, self._source@) && image_allowed(self.authority.policy, self.image)
    }

    /// Complete image size used by the adapter's read-back bound.
    pub fn bytes(&self) -> (out: u64) ensures out == self.image().version().bytes,
    { self.image.version.bytes }

    /// Sparse copying is possible only for an explicitly sparse request.
    pub fn sparse(&self) -> (out: bool) ensures out == (self.mode() is Sparse),
    { matches!(self.mode, CopyMode::Sparse) }

    /// Full read-back is mandatory exactly when the request selected verified writing.
    pub fn verify(&self) -> (out: bool) ensures out == (self.mode() is Verified),
    { matches!(self.mode, CopyMode::Verified) }

    /// Original opened-file digest used for full read-back verification.
    pub fn digest(&self) -> (out: &str) ensures out@ == self.image().digest(),
    { self.image.digest.as_str() }

    /// Original disk number for descriptor-owned dispatch and review.
    pub fn device(&self) -> (out: u64) ensures out == self.authority().confirmed().selected().target().number,
    { self.authority.confirmed.selected.disk.number }

    /// Refuse to report unchanged source metadata after a concurrent image modification.
    pub fn source_matches(&self, current: &ImageVersion) -> (yes: bool)
        ensures yes == same_image(self.image().version(), *current),
    { image_eq(&self.image.version, current) }
}

}
