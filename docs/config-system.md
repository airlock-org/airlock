# Config System

Scope: the separate `sys-config-*` candidate workflow and shared storage
architecture. This is **not the active physical-lab deployment authority**.
Use [machine deployment](machine-deployment.md) for the host/sys-dev deployment
paths and [sys-ctrl](completed/05-sys-ctrl.md) for the proposed successor.
This parent-repository integration copy adapts component links from
[airlock-machine documentation](../airlock-machine/docs/config-system.md);
neither copy should be read as proof of the current lab topology.

The config system is Airlock's first self-hosted topology slice. Dom0 still owns
Qubes control authority, but the desired Qubes topology and dom0 system plan
come from a Nix flake edited inside dedicated config VMs.

## Declared Objects

`airlock-machine/lifecycle/bootstrap/config-system.nix` declares:

- `sys-config-vault`, an offline closure-backed NixOS `StandaloneVM`.
- `sys-config-staging`, an offline closure-backed NixOS `StandaloneVM`.
- `sys-config-validator`, an offline closure-backed NixOS `StandaloneVM`.
- the Qubes VM kernel used by those NixOS domains.
- bootstrap qrexec policy for staging export, validator checks, vault archival,
  current-baseline sync, and deployment records.
- a boot-time `airlock-setup-config.service` that ensures the bootstrap topology
  exists after Qubes dom0 services are up.
- an on-demand `airlock-config-bootstrap-current` command that asks
  `sys-config-vault` for the latest current config bundle and imports it into
  `sys-config-staging` when staging is empty or already on that commit. Config
  VMs are never started automatically during dom0 boot.

The bootstrap policy remains dom0/NixOS-owned in `30-airlock-config.policy`.
Staged config cannot remove dom0's ability to fetch current baselines, fetch
new candidates, validate, archive, or apply candidates. Additional
Airlock-owned qrexec policy is declared in staged topology and rendered by
`airlock-setup-config` to:

```text
/var/lib/airlock/qubes-policy.d/90-airlock.policy
```

Static Qubes policy is still assembled by Nix under `/etc/qubes/policy.d`.
Airlock prepares `/var/lib/airlock/qubes-policy.d` as the runtime combined policy
directory, symlinks static rules into it, keeps `90-airlock.policy` mutable, and
starts `qubes-qrexec-policy-daemon` with that runtime policy path.

## First-Boot Config Baseline

`sys-config-vault` is the authoritative archive. On its first explicit start,
it initializes `/home/user/airlock-config-vault` as a Git repository and, if no current
deployment or baseline exists yet, creates an initial current candidate from the
initial runtime config embedded in the image. It records that baseline under
`candidates/<commit>/`, writes `latest-current`, and maintains a `current`
symlink to the selected candidate snapshot.

When requested explicitly, dom0 runs `airlock-config-bootstrap-current`. The
command starts `sys-config-vault` and `sys-config-staging`, exports the vault's
current bundle through `airlock.Config.ExportCurrent`, and imports it through
`airlock.Config.ImportCurrent`. Existing dirty staging work is left untouched;
an empty staging repo is cloned from the vault bundle.

## Domain Construction

Airlock supports two domain construction families.

Traditional Qubes domains keep the mature TemplateVM/AppVM model:

```nix
domains.sys-net = {
  backend.type = "qubes-template";
  template = "fedora-44-xfce";
  label = "red";
  netvm = null;
};
```

These are ordinary Qubes domains. Fedora, Debian, `sys-usb`, and other
conventional guests stay on this path unless there is a specific reason to move
them. The bootstrap `sys-net` and `usr-cmd` domains use the Nix-native path below.

NixOS domains use the `nix-closure` backend:

```nix
domains.work = {
  backend = {
    type = "nix-closure";
    system.sourceRef.build = "nixosConfigurations.work.config.system.build.toplevel";
    kernel.name = "6.12.64-1.fc41";
    root.sourceRef = {
      build = "packages.x86_64-linux.work-root-image";
      subpath = "root.img";
    };
    store.sourceRef = {
      build = "packages.x86_64-linux.work-store-closure";
      subpath = "";
    };
  };
  label = "green";
  netvm = null;
};
```

Authored flake output references use `sourceRef = { build = "..."; }`, with an
optional `subpath`. A reference may also be supplied directly through `source`.
Normalization emits `source_ref.attr` for validation and activation consumers.
The authored options are `sourceRef` and `build`; `source_ref` and `attr` belong
to normalized records.

Authored options use camelCase, including `activationCommand`, `activationAction`,
`switchAction`, `skipStart`, `readOnly`, `persistentSize`, `qrexecPolicy` and guest
`hostName`. Their snake_case deployment fields are normalized output.

A `nix-closure` domain is represented as:

```text
Nix system closure
+ small root image
+ read-only closure store volumes
+ private persistent state
+ volatile state
+ VM resources
+ devices/networking
+ qrexec/security policy
```

Dom0 resolves build references from the validator-approved candidate, imports
the small root image into the Qubes `root` volume, admits the exact closure into
the Airlock closure store, creates separate Qubes volumes for the selected
generation, and sets prefs/features/services/policy. The EROFS metadata volume
is attached at `/dev/xvde`. The filtered blob-data volume is attached at
`/dev/xvdf`. Early guest bootstrap mounts `/dev/xvde` read-only at
`/nix/store` with `/dev/xvdf` as its external EROFS blob device before the real
NixOS init runs. The topology schema currently constrains those devices because
that is the validated Qubes/libvirt ordering for this path.

The default shared-closure backend keeps one canonical physical representation
for each admitted Nix store object. Object data is stored as deterministic
POSIX tar fragments in:

```text
/var/lib/airlock/closure-store/objects/segments/000001.blob
/var/lib/airlock/closure-store/objects/segments/000001.json
/var/lib/airlock/closure-store/objects/segments/000002.blob
/var/lib/airlock/closure-store/objects/segments/000002.json
/var/lib/airlock/closure-store/objects/blob-metadata/<store-entry>.json
```

Segments are append-only while open and immutable once sealed. The default
target segment size is 2 GiB. A store object is never split across segments; if
one object is larger than the target, it receives a dedicated oversized sealed
segment. Object metadata records the stable store identity plus one current
location and any retired verified locations. Closure and generation identity
remains based on store paths, so compaction can move live objects by publishing
new current locations and refreshing affected closure views without changing the
semantic generation.

Each exact closure gets:

```text
/var/lib/airlock/closure-store/views/by-closure/<closure>/store.erofs
/var/lib/airlock/closure-store/views/by-closure/<closure>/store.blob-map.json
/var/lib/airlock/closure-store/views/by-closure/<closure>/object-refs
```

`store.erofs` is a small EROFS tar-index metadata image containing only the
declared closure namespace. `store.blob-map.json` maps that closure's logical
external-data stream to canonical object extents in one or more sealed segment
blobs. If two closures include the same Nix store path, their blob maps point at
the same segment id, segment-local offset, length, and hash.

At VM start, the Qubes `airlock-closure` storage pool asks the storage engine to
create a read-only device-mapper view from the closure blob map. The guest
receives only that filtered `/dev/mapper/airlock-*` block device as `store-data`;
it never receives segment files, the canonical object pool, or an unfiltered
global store mount.
Root inside the guest can read only the bytes referenced by the declared
closure view.

Physical cleanup is split into two operations. Logical GC removes unreachable
generations, views, and object metadata. Segment compaction then considers sealed
non-oversized segments whose current live-byte utilization is below the
configured threshold. A compaction run never edits a sealed segment in place:
it copies each still-live object from the selected old segment into a new
segment, verifies the copied bytes by hash, atomically publishes the new object
location, refreshes closure blob maps, and then retires old segments only when no
running-generation or runtime device-mapper view still references the old
physical extents.

Use:

```bash
airlock-closure-store compact --dry-run true
airlock-closure-store compact --dry-run false
```

The default policy compacts at most one low-utilization segment per run with a
50 percent live-byte threshold. The command reports candidate segments, live and
dead bytes, bytes copied, bytes reclaimed, moved objects, retired segments,
segments blocked by active views, and write amplification.

To reconstruct or back up the closure store, retain:

```text
/var/lib/airlock/closure-store/MANIFEST.json
/var/lib/airlock/closure-store/objects/segments/
/var/lib/airlock/closure-store/objects/blob-metadata/
/var/lib/airlock/closure-store/objects/metadata/
/var/lib/airlock/closure-store/views/by-closure/
/var/lib/airlock/closure-store/domains/
/var/lib/airlock/closure-store/running/
```

Sealed segment blobs are immutable and backup-friendly. Incremental backups only
need new sealed segments plus updated metadata/manifests after new admissions.

Each domain generation records an exact closure manifest under:

```text
/var/lib/airlock/closure-store/domains/<domain>/generations/<n>/manifest.json
```

The Qubes-facing volumes are produced by the `airlock-closure` storage pool.
The object pool, closure views, and generation manifests are the authoritative
state. `shared-closure` with `erofs-objectmap` is the only supported NixOS VM
store backend.

### Admission and prepared USB stores

The ordinary admission path starts with guest packages in dom0's `/nix/store`.
The small `store-closure` artifact contains a list of store paths and a manifest,
not the package contents. Its references retain the source package payloads.

During setup, `airlock-setup-config` invokes
`airlock-nix-domain-storage ensure-store` to prepare sys-gui. For each store path
not already admitted, the storage engine reads the source files from
`/nix/store`, creates a temporary deterministic tar fragment, and appends its
bytes to `/var/lib/airlock/closure-store/objects/segments/`. It then builds the
filtered read-only closure view from those shared extents. This is a real copy
and repackaging step, not a bind mount or a metadata-only operation. Later
admissions reuse existing objects, including objects admitted for other VMs.

```text
Image's ordinary /nix/store (source package files; retained)
    -> first admission: copy/repackage previously unadmitted objects
Shared segment pool (one representation per admitted store object)
    -> small per-closure metadata and filtered block mappings
Guest /nix/store (read-only view; no additional full package copy)
```

Admission does not delete the source files or remove their Nix references.
Consequently, sharing within the runtime pool does not eliminate duplication
between that pool and the image's ordinary `/nix/store`, even for packages dom0
also uses. Read-only access and guest isolation do not inherently require this
duplication; it is a limitation of this source-based admission path.
Putting the pool in another VM would not by itself remove the source copy.

The earlier 20 GiB GUI developer image hit `ENOSPC` during admission. On a
32 GiB image, admission succeeded but took approximately 22 minutes on the test
USB, with about 22 GiB read and 7 GiB written according to systemd accounting.

The USB developer image now prepares its guest store during image construction.
Both the hybrid and AMD-lab sys-gui closures are admitted as separate retained
generations in one shared pool. The build seals the segments and writes
descriptors containing hashes of the installed metadata and segment bytes.
The disk builder places the pool directly at `/var/lib/airlock/closure-store`,
outside the image's ordinary Nix store. It is not copied there on first boot.

Seed builds must be reproducible across independent rebuilds of their two Nix
outputs. Dom0 retains the small descriptors, but the build-time pool output
can be garbage-collected and regenerated. The first implementation reused
runtime admission's random transaction IDs, wall-clock timestamps and random
EROFS filesystem UUIDs. A regenerated pool could therefore disagree with the
still-cached descriptors even though both had the same Nix derivation identity.
This caused the 2026-09-14 USB bootstrap integrity failure before GPU assignment.

Fresh seed construction now uses an explicitly scoped deterministic build
context: metadata timestamps use the epoch, transaction IDs derive from the
domain/generation/system, and EROFS uses a fixed timestamp and closure-derived
UUID. Runtime admissions retain real timestamps and random transaction IDs;
no runtime integrity checks are relaxed. POSIX tar fragments also omit PAX
access/change timestamps, use stable extended-header names and encode file bytes
independently of host hardlink deduplication. This preserves long paths and
symlinks without depending on the build host's inode history.

The image builder runs the prepared-store checker directly in `preVM`, before
copying the pool or assembling the disk. It checks all descriptors and hashes
each shared file once. The separate validation dependency remains useful as a
build/test target, but its cached receipt is not sufficient evidence for a new
image assembly. Neither the receipt nor the build-only checker adds the pool or
guest source closure to dom0's runtime store. Reproducibility tests build the
same seed twice with changed source inode timestamps, then cross-check each
pool with the other build's descriptors, modeling either output surviving
collection.

Fix verification on 2026-09-14: 13 focused prepared-store/storage/recovery
checks passed (the optional full-OS closure accounting test was not run).
The full USB seed passed finalized-output validation for both variants across
2,741 files, and `nix build --rebuild` of the seed confirmed identical outputs.
The original broken artifacts fail the new validator. Evidence is preserved
under `.airlock-logs/prepared-store-reproducibility/`; a rebuilt/flashed USB
boot remains the hardware acceptance check.

The 2026-09-16 USB failure exposed a gap in that earlier test: `--mtime=@1`
does not remove POSIX tar's PAX `atime` and `ctime` fields. The failed sys-dev
fragment contained real host timestamps, and the local pool failed the same
metadata checksum as the USB. The separate validation receipt was still cached
as passed. The archive normalization and in-image-build check above address
both faults; do not work around this failure by disabling bootstrap metadata
verification or manually editing its checksums. Rebuild the image and provision
it normally after the fix. A successful local validation is not proof of a
successful physical boot.

Fix verification on 2026-09-16: the timestamp-changing regression failed before
the fix and passed afterward. The rebuilt eight-variant USB seed passed full
validation across 3,931 files. `make test-quick` passed 290 tests (23 skipped),
and the focused image-hook test confirmed that corrupt metadata stops assembly
despite a prior successful receipt. The validation result is rooted at
`.airlock-logs/prepared-store-reproducibility/validation-pax-fix`; the full image
rebuild and physical boot remain separate acceptance steps.

The topology and prepared descriptors deliberately discard Nix references to
guest source packages, using Nix's image-artifact reference-discard mechanism.
Explicit dependencies retain the descriptors, guest root images and Qubes
kernel artifacts that dom0 still needs. The guest system paths remain valid
identities inside the guest's filtered store, without requiring those paths to
exist in dom0's store. Packages independently required by dom0 still remain
there, so dom0/guest overlap continues to occupy two representations. The
build machine's source store is not modified or pruned by this process.

On first setup, the storage helper recognizes `prepared.json`, checks the exact
domain/system/root/kernel/closure identity and device layout, and verifies the
prepared metadata against its checksums. Bootstrap trusts the image's payload:
it checks segment paths, presence and sizes without reading or hashing their
contents. Full payload checksums remain mandatory in finalized-output image
validation and available explicitly through the prepared-store module's
`--check POOL DESCRIPTORS` command. Same-size payload corruption after image
validation is therefore not detected by bootstrap. Missing or truncated segments
and invalid metadata fail instead of silently starting a fresh
admission. Qubes then creates its normal read-only store volumes and records
the selected generation. Subsequent matching setup uses the existing generation
without repeating this seed verification. Ordinary source-based admission
remains available for later updates supplied with their source packages.

Prepared descriptors describe the original sealed seed. After compaction or
removal of a seed generation, they are not a substitute for an update artifact;
the ordinary generation and Qubes volume state remain authoritative. Reimaging
installs a fresh seed. Runtime writable VM state and future updates still need
free space even though the initial package-conversion workspace is eliminated.

Implementation references: [closure manifest construction](../airlock-machine/lifecycle/misc/lib/qube-template.nix)
(`mkNixStoreClosureManifest`), [domain admission](../airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py)
(`ensure_shared_store`), and [object reuse and copying](../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/objects.py)
(`admit_object_tar_fragment`, `append_file_to_open_segment_blob`).
Prepared images use [the seed builder](../lifecycle/image/prepared-guest-store.nix),
[USB dependencies](../lifecycle/image/profiles/usb-prepared-store.nix), and
[preparation/verification](../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/prepared.py).

### What closure and VM storage sharing saves

A Nix system closure is the system plus all its transitive store dependencies.
Its manifest can be small while the package data it names occupies gigabytes.
Filtering controls which paths and bytes a VM can read; physical sharing comes
from multiple filtered views referencing the same backing objects.

| Component | Storage behavior |
| --- | --- |
| Guest root image | Small bootstrap filesystem; it does not embed a full `/nix/store`. |
| Closure manifest and EROFS view | Path lists, filesystem metadata, and mappings, rather than another full copy of package data. |
| Shared package objects | Reused across unrelated VMs and retained generations when they use the same store paths. Different store paths are not automatically deduplicated merely because some files match. |
| Private and volatile volumes | Independent VM state, not deduplicated by the closure pool. Sparse image capacity is not the same as allocated disk blocks; writes consume real space. |
| Ordinary dom0 store | Separate from the shared pool. Prepared USB images omit guest-only source packages; packages dom0 itself needs still count in addition to the guest backing data. |

For example, if two VM closures each need 6 GiB of backing object data and share
5 GiB of identical store objects, the pool needs about 7 GiB of payload rather
than 12 GiB, plus metadata and alignment overhead. A further VM with an already
represented closure adds no new package payload, but still needs its own VM
state. This example describes the runtime pool only, not total USB usage.

Updates add newly required objects while reusing unchanged ones. Retaining old
generations retains their unique objects, and logical GC alone does not reclaim
dead bytes inside segment files; physical compaction is described above.
Measure allocated filesystem blocks for real disk usage, not just sparse volume
sizes, manifest sizes, or Nix `narSize` (serialized archive size).

### Possible future directions (not implemented)

The `/nix/store` pathname and the physical storage behind it are different
things. NixOS programs still require their expected `/nix/store/...` paths, but
those paths could be provided by a mounted view rather than a separate copy of
the package files. Build-time preparation is now implemented for the developer
USB as described above. Possible follow-ups are extending it to other image
profiles and giving dom0 its own closure view of the same backing pool. That
would let identical store objects used by dom0 and guests share backing bytes
too; it is not a supported dom0 boot configuration today.

Repackaging cannot be replaced by an ordinary file move because source files
and segment fragments have different layouts. An image builder could instead
produce the shared representation and omit unnecessary source copies from the
final image, without deleting anything from the build machine's Nix store.

Sharing dom0's backing store needs an early-boot mount path that works before
normal dom0 services or any guest VM starts. It also needs bootable updates and
rollback, protection of active dom0 generations during GC and compaction, and
consistent Nix store metadata. Since the existing views are read-only, local
builds and package installation would require a writable layer or a separate
staging store with a defined admission/update lifecycle. Hosting the backing
pool in another VM adds a boot dependency; it does not solve these issues or
remove duplication by itself.

These are separate storage and boot milestones. The immediate GUI milestone
keeps dom0's current boot/store design: provide enough measured space for
sys-gui initialization, then verify a personalized image reaches a usable GUI
when booted through virtual USB storage before another physical laptop trial.

## Warm Integration Loop

Use local tests first:

```bash
make test-quick
```

For repeated real Qubes/Xen checks against an already-created nested dom0 disk,
reuse the warm targets:

```bash
make vm-config-warm-flow
make vm-nix-closure-warm-flow
```

`make vm-nix-closure-warm-flow` imports the current local closures for
`airlock-setup-config`, `airlock-nix-closure-domain-smoke`, and the
`airlock-closure` Qubes storage adapter into the running nested dom0 before it
runs the focused NixOS-domain lifecycle test. It also sets
`AIRLOCK_VM_SKIP_BASE_BUILD=1`, so it reuses the existing nested base-image
result link instead of resolving a new base image for every warm iteration. The
synced commands are exposed under:

```text
/run/airlock-warm/bin
```

and `qubesd.service` receives a temporary `/run/systemd/system` override that
prepends the synced storage adapter and the required `dmsetup`/`losetup` tools.
This makes warm iterations test the current checkout without recreating the
whole nested dom0 disk.

The cold targets remain the reproducibility checks:

```bash
make vm-config-flow
make vm-nix-closure-flow
```

## Config Roles

The bootstrap config roles have fixed names and separate persistent state:

```text
sys-config-vault      -> config-vault-private
sys-config-staging    -> config-staging-private
sys-config-validator  -> config-validator-private
```

All three default to `netvm = null`, disable `network-manager`, and use normal
qrexec. Their security boundary is Qubes' domain boundary plus the bootstrap
qrexec policy.

`sys-config-staging` owns the editable candidate repository:

```text
/home/user/airlock-config
```

`sys-config-validator` validates exported candidates, evaluates
`.#airlock.topology` and `.#airlock.system`, and dry-runs referenced builds.
Because the VM system store is read-only, config validation runs Nix through
`airlock-config-nix`, which uses a private writable chroot store rooted at:

```text
/home/user/.local/share/airlock/nix-root
```

That keeps `/nix/store` as the immutable system identity of the running guest
while still allowing `sys-config-staging` and `sys-config-validator` to lock, evaluate,
and dry-run candidate flakes.

`sys-config-vault` owns the historical archive:

```text
/home/user/airlock-config-vault
```

It records:

```text
candidates/<candidate-commit>/
deployments/<timestamp>-<candidate-prefix>/
latest-candidate
latest-deployment
latest-deployment-attempt
```

## Staged Edit Surface

Inside `sys-config-staging`, edit:

```text
/home/user/airlock-config/airlock/runtime.nix
```

The high-level Nix-native domain surface is:

```nix
nixNative.domains.work = {
  role = "base";
  label = "green";
  netvm = null;
  packages = with pkgs; [ git jq ];
  etc."airlock/domain-role".text = "work\n";
};
```

That expands to a guest NixOS system, a small root image, a read-only store
image, and a normalized `backend.type = "nix-closure"` topology entry.

The same file can declare conventional Qubes TemplateVM-backed domains with
the generic `domains` or `topology.extraDomains` surfaces. Those remain
first-class for Fedora, Debian, and other non-NixOS guests.

## Command VM

The initial runtime config declares a Nix-native `usr-cmd` domain with:

```nix
nixNative.domains.usr-cmd = {
  role = "cmd";
  netvm = "sys-firewall";
};
```

The bootstrap `vm-gui-config` topology materializes the same domain, so a fresh
boot has `usr-cmd` available before the first self-hosted apply.

`sys-net` is part of the Airlock bootstrap topology. It is an ordinary Qubes
domain, but its first-boot configuration is generated from the Nix topology
instead of being hand-created after boot. The expected command-workspace network path is:

```text
usr-cmd -> sys-firewall -> sys-net -> physical network
```

For `sys-net`, an empty `devices.pci` list means `airlock-setup-config`
discovers dom0 PCI devices with the network-controller class and assigns the
available devices to `sys-net`, matching the normal Qubes installer behavior.
In nested bootstrap development images the launcher exposes two emulated NICs:
one remains dom0's direct development uplink for SSH and package access, and the
second is assigned to sys-net as `dom0:00_03.0` with `no-strict-reset` enabled.
The nested launcher also exposes an emulated IOMMU so Xen/Qubes can enforce that
PCI assignment. On non-dev images dom0 has no direct DHCP uplink, so the same
sys-net topology can claim the host network-class PCI devices.

If an external domain is referenced by a candidate but does not exist yet, setup
defers that NetVM assignment until the domain exists and setup is rerun.
The self-hosted `airlock check` and `airlock apply` path is stricter: after the
validator emits a topology, dom0 verifies that referenced domains are either
declared by the candidate or already exist in Qubes, and rejects the candidate
before promotion or activation if a referenced NetVM is missing. For external
network paths, dom0 verifies that any external provider exists and provides
network, and that `sys-net` has no upstream NetVM when it is used as the
physical uplink.

The `cmd` role layers development tools on top of the minimal Qubes guest base.
It includes `git`, the private-store Nix CLI wrapper, Rust and Python tooling
used by this repository, `make`, shell/editor basics, the `airlock` VM control
CLI, and the pinned nixpkgs `codex` and `zellij` packages.

The VM keeps `/nix/store` as the read-only activated generation identity.
Interactive Nix commands run through `airlock-config-nix`, which stores mutable
build state under the VM private volume instead of writing to the system store.

Persistent user state is initialized at:

```text
/home/user/dev
/home/user/.codex
```

`CODEX_HOME` is set to `/home/user/.codex`, matching Codex's documented default
state directory. Codex authentication and configuration are mutable private VM
state; no API keys, tokens, `auth.json`, or user config are placed in Nix
configuration or the Nix store. In a nested dev-mode image, `make
vm-sync-codex-auth` sends the host login state to `usr-cmd` through the dedicated
`airlock.CodexAuthImport` qrexec service. Dom0 does not retain the credential.

Dev-mode dom0 behavior is selected at image build time. The Make image targets
default to `VM_IMAGE_MODE=dev`; pass `VM_IMAGE_MODE=release` to build the same
image family without dom0 direct DHCP, the dev marker, or dev-only dom0 tools.
Explicit flake outputs are also available, for example
`airlock-vm-gui-config-dev-image` and `airlock-vm-gui-config-release-image`.

The expected repository path is:

```text
/home/user/dev/airlock-org/airlock
```

Airlock does not bake a mutable checkout into the immutable VM closure. If that
path already exists it is preserved. On a fresh VM, clone or move the repository
there once, then use:

```text
cd ~/dev/airlock
zellij --version
codex
nix flake check
make test-quick
airlock status
airlock check
airlock apply
```

`usr-cmd` remains an ordinary unprivileged domain, not a `sys-*` service VM. Its
qrexec authority is the narrow config-control path granted to `usr-cmd`; it
does not receive sys-config-staging, sys-config-validator, sys-config-vault, dom0 filesystem,
or arbitrary dom0 command-execution services.

Dom0 host changes are declared under:

```nix
dom0.host = {
  packages = with pkgs; [ vim ];
  etc."airlock/example".text = "hello\n";
  systemd.services.airlock-example = {
    description = "Airlock example service";
    serviceConfig.Type = "oneshot";
    script = "${pkgs.coreutils}/bin/true";
  };
  configuration = {
    boot.kernelParams = [ "loglevel=4" ];
  };
};
```

These fields compile into the candidate `nixosConfigurations.airlock-dom0`
closure. Dom0 builds and switches that closure only after validator approval.

## Apply Flow

The normal flow is:

```text
sys-config-staging
    exports committed candidate
        ↓
sys-config-validator
    validates and evaluates topology/system
        ↓
dom0
    archives attempted candidate to sys-config-vault
    resolves build references
    applies topology and dom0 system plan
        ↓
sys-config-vault
    records deployment result
```

Run from dom0:

```bash
sudo airlock-config-apply --apply
```

`airlock-config-apply` resolves every `nix-closure` source reference to a
concrete store path before invoking `airlock-setup-config`. It records
materialized domain artifacts, topology setup output, dom0 system apply output,
and deployment metadata.

`airlock-config-status` reports the active topology, Qubes runtime domains,
current deployment records, current host generation links, pending reboot
state, pending VM restart state, and the Airlock-managed qrexec policy file as
JSON.

## Command VM Control Path

The ordinary `usr-cmd` is allowed to request narrow config
generation operations from dom0:

```text
usr-cmd
    airlock status/check/apply/rollback
        ↓ qrexec
dom0 airlock-config-control
        ↓
sys-config-validator / sys-config-vault / dom0 apply or rollback
```

The VM-facing `airlock` command submits a clean Git commit as a `config.bundle`
plus a `airlock.config-candidate.v1` manifest. The manifest records the commit,
tree, branch, source role, qrexec transport, requested target class, and the
dom0 status observed by the client before submission. Dom0 and
`sys-config-validator` do not trust this status to decide validity; it is candidate
identity/context only.

The dom0 qrexec entry point is fixed to four operations:

```text
airlock.Config.Status
airlock.Config.Check
airlock.Config.Apply
airlock.Config.Rollback
```

`Check` and `Apply` accept only the staged candidate tar shape
`config.bundle + manifest.json`. They then call the existing
`airlock-config-archive-candidate` and `airlock-config-apply` machinery with that
candidate tar. `usr-cmd` never receives a qrexec primitive for shell commands,
scripts, arbitrary file writes, direct dom0 filesystem access, or direct Nix
store writes.

`airlock.Config.Check` validates in `sys-config-validator` without archiving or
activating. `airlock.Config.Apply` validates in `sys-config-validator`, archives the
validated candidate in `sys-config-vault`, resolves build references in dom0, and
activates only the resulting validated topology/system plan. Failed evaluation,
build, test, or activation leaves the last current deployment record intact.

Rollback is intentionally narrow. `airlock.Config.Rollback` can only switch the
dom0 NixOS host profile back to the previous recorded/profile generation and
run that generation's `switch-to-configuration switch`. It does not accept a
client-supplied path or command.

## Setup Path

The dom0 module writes the desired bootstrap topology to:

```text
/etc/airlock/qubes-topology.json
```

Apply it explicitly with:

```bash
sudo airlock-setup-config --apply
```

The nested config image also enables `airlock-setup-config.service` at boot.
That one-shot unit runs after `libvirtd`, `qubes-db-dom0`, `qubesd`, and
`qubes-qrexec-policy-daemon`. A fresh image should come up with the three
config VMs present. Existing matching objects are left alone, so later runs are
normally no-ops. There is no timer.

## Remaining Architecture Work

The default `shared-closure` path physically shares canonical Nix store object
fragments across unrelated NixOS domains and generations, while preserving
independent read-only closure views. Remaining storage work is narrower:

- validating the prepared USB store through a complete GUI boot and measuring
  its startup time and disk usage, then extending preparation to other images.
- running more Qubes/Xen lifecycle coverage for segment compaction under active
  VMs.
- reducing object admission overhead if tar fragment creation becomes a
  bottleneck for large store paths.
- running repeated warm and final cold Qubes/Xen lifecycle tests for the EROFS
  external-data mount path.
