# Share immutable store objects between dom0 and guests

Status (2026-09-26): image assembly succeeded after seed permission and Xen
EFI validation fixes. The first laptop boot hung after the host store's saved
assembly checkpoint; no persistent journal was available. Runtime acceptance
remains incomplete. Local follow-up explicitly orders NixOS closure discovery
after pool assembly and enables USB boot diagnostics (including recovery):
`/var/log/airlock-boot/*-initrd.log` for early boot and `/var/log/journal` for
stage 2. Early log lines are synced to disk as received; failures before the
root filesystem mounts still require a screen photo. The ordering race is a
concrete source defect, not yet a confirmed explanation of the laptop hang.
The follow-up image has not been built or booted.
This extends [direct pool deployment](06-direct-pool-machine-deployment.md);
TODOs 05, 06 and 07 must be validated together on the next fresh image.

## Local implementation recorded

- The image first prepares the existing guest seed, then admits the complete
  host closure into a final unified seed. This ordering avoids a host/seed Nix
  dependency cycle. Exact store identities reuse existing object extents.
- Dom0 has a systemd initramfs service that validates the kernel-command-line
  host identity, registration, index, blob map and physical pin, then assembles
  EROFS bases and a private OverlayFS upper before stage 2. Python storage
  primitives, Nix registration tools, loop and device-mapper tools are carried
  in the initramfs; Qubes services and networking are not dependencies.
- Normal Nix clients use the daemon, whose backend is the local overlay store.
  Imports and builds write into `/var/lib/airlock/host-store/upper`. Pool
  publication verifies and admits these additions before exposing boot records.
- Planning binds host pool identity as well as guest generation receipts.
  Applying publishes before guest disruption, installs the boot entries, seals
  the exact Xen/kernel/initramfs artifacts, and records selection. Running and
  selected host records remain separate. Publication does not mount a new base
  over an active host or controller.
- The recovery specialisation has `inheritParentConfig = false`. Its conventional
  closure lives in `/var/lib/airlock/recovery-store`, independently of the pool.
  It includes console authentication, disk tools and retained-host selection,
  without desktop, guest services or networking. Image assembly removes the
  ordinary full host-store copy only after installing boot entries. Channels
  are not copied; locked build inputs already belong to the builder seed.
- Image builder views include the approved host closure. Runtime host publication
  prepares an additive sys-ctrl successor containing its prior registered inputs.
  Qubes replaces both builder volumes in a stopped setup; an active controller
  continues using its previous volumes. Root-owned predecessor bindings authorize
  the guest's offline transition while preserving its upper and Nix database.
  Superseded physical pins are released after the new volumes are saved.
- Early boot removes identical published host-upper copies, rebuilds registration
  using Nix's dump/load format, preserves trust/output metadata, and uses a
  recoverable directory-rename sequence for the database. Saved upper references
  retain any older host bases they still need. Release pruning retires host boot
  choices; old physical pins are dropped only after the offline reference
  transition. Existing pool GC/compaction handles the resulting unreferenced
  generations and extents under the same pool lock.
- Added pytest source coverage for registration conflicts, boot identity,
  publication interruption, metadata damage, authorized builder transitions,
  database recovery, Xen artifact corruption and image-copy removal. Existing
  recovery/coordinator test expectations were updated. None were executed.

### Storage and operational boundaries

Builder transitions are additive: objects in the approved builder input set are
retained even if the host release that introduced them is retired. Retiring a
host is not authority to discard a controller's saved build dependencies.
Independent recovery and initramfs/ESP artifacts deliberately duplicate a subset
of payloads. Build-time guest/unified seeds and image installation also have
transient copies; they are not a second retained ordinary host store.

Image assembly records `host-store-install.json` with recovery bytes, pool bytes,
zero remaining ordinary host objects, and pool-plus-store installation allocation.
`airlock-host-store status` reports pool, upper and recovery allocation. These are
measurement hooks, not measured performance claims; full peak allocation and
cold/repeat timings still need the acceptance run.

Recovery is operator-selected from the boot menu, including if normal pool
assembly fails. In recovery, `sudo airlock-host-store recover SYSTEM` checks the
retained pool binding and installed EFI artifact hashes before choosing the Xen
entry. It does not reboot automatically. Normal recovery selection, ordinary Nix
operations, the exact pinned SQLite schema, daemon mount namespaces and power-loss
ordering all require validation on the rebuilt image.

## Goal and expected gains

Retain immutable Nix objects once in the dom0-managed pool and expose them
through separate, explicitly selected closure views for dom0 and each guest.
Allow sys-ctrl's builder view to reuse appropriate host objects already present
on the machine, while keeping writable state and activation authority separate.

Expected gains, to be measured:

- Reduce duplicate package payloads across dom0's ordinary store and the pool.
- Avoid downloading or rebuilding matching objects already available locally.
- Admit a payload once and make it available to another authorized closure
  view through metadata, without another full payload copy/import.
- Account for host, guest, builder and rollback retention in the same pool.

Shared physical storage does not grant a VM access to every stored object.
Closure views determine exposure. A host object becomes visible to sys-ctrl
only when included in its approved builder view with matching Nix registration.

This cannot eliminate genuinely missing build dependencies. Runtime closures
do not necessarily contain compilers, sources, derivations and other inputs
needed to rebuild their packages. Reuse requires matching Nix object identities;
similar package names or versions are insufficient. A unified pool alone will
not accelerate the controller if its builder view still excludes those objects.

## Reused components

The implementation extends guest EROFS object maps, admission and physical pins
to the `dom0` pool domain. Guest root artifacts and Qubes-owned writable root and
private volumes retain their existing identities and lifecycle. Builder metadata
uses the existing two read-only Qubes volumes and a separate persistent upper.

Reuse existing object admission, verification, closure indexing, generation
publication, retention, segment pinning and Qubes volume mechanisms. Preserve
Xen, qrexec, QubesDB, qubesd, libvirt and Qubes policy.

Airlock is unreleased: target newly built images. No backwards compatibility
or migration from existing installations is required.

## Preferred boot design

Put the minimal pool-assembly tools in the initramfs. Mount the backing
filesystem, validate the selected host generation and assemble dom0's pool
view before entering the normal system. Then start dom0 with that view at
`/nix/store` and the matching Nix state.

Do not require qubesd, a controller VM, networking or an already running normal
dom0 system to construct this view. Factor out the storage primitives needed
in early boot from the higher-level VM lifecycle integration.

Keep a separately bootable recovery system and the kernel/initramfs required
to reach it available without a functioning main pool view. Precisely define
and measure the small bootstrap/recovery duplication retained for this purpose.
Do not retain a second complete ordinary host store merely as a bootstrap copy.

The earlier suggestion of booting conventionally and switching stores is useful
as an architectural starting point. Prefer assembling the final view in the
initramfs, avoiding a replacement of `/nix/store` underneath running services
and active Nix operations.

## Implementation stages (local source added; validation pending)

1. **Establish the storage and boot contract.** Inventory the host closure,
   bootstrap/recovery requirements and overlap with guest/builder closures.
   Define the selected host manifest, registration metadata, physical pins and
   boot-selection records. Check which pool-assembly dependencies can run in
   the initramfs without circular dependencies on the view they assemble.

2. **Seed and mount a host closure view.** Extend image preparation to admit
   host objects alongside guest objects, deduplicating exact identities. Build
   dom0's view and registration from the selected host generation. Assemble it
   during early boot and retain the independent recovery path. Guest writable
   roots and private volumes remain owned separately by Qubes.

3. **Integrate ordinary Nix operations.** Prototype a read-only pool base with
   a private writable layer, reusing the existing builder approach where it
   fits. Define daemon access, registration, temporary roots, profiles and
   write/import behavior. Nix's local overlay store is experimental; validate
   the repository's pinned Nix version rather than assuming guest setup can be
   used unchanged for dom0. New upper-layer objects must be verified and
   durably admitted before releases depend on their pool representation.

4. **Publish host updates and retain generations.** Bind each host release to
   its exact pool generation, registration and boot artifacts. Make crash
   ordering explicit: persist and pin objects and metadata before selecting a
   bootable generation. Keep the running host protected even when another
   generation is selected for the next boot. Interrupted publication must leave
   a bootable retained generation or a usable recovery path.

5. **Refresh builder views safely.** Include selected host runtime objects and
   useful available build inputs in sys-ctrl's view. Replace the current frozen
   base restriction with a defined transition that preserves upper-layer
   references, registration and physical pins. Start with a stopped controller
   and an atomic base transition; hot rebasing is not required. A saved upper
   layer must never silently attach to a different lower base.

6. **Unify retention and remove redundant copies.** Coordinate Nix GC with pool
   GC and compaction. Protect selected/running host generations, retained
   rollback generations, recovery artifacts, guest generations, in-progress
   admissions and persistent builder dependencies, including stopped VMs.
   Release redundant writable-layer copies only after safe publication and
   reference transition. Separate databases may remain where required; define
   how their roots protect shared physical objects. Measure steady-state and
   peak storage, including staging, recovery and writable layers.

Adding selected host inputs to the existing builder seed is a possible
intermediate experiment to reduce cold-build time. With dom0 still using its
ordinary store, that can duplicate payloads and does not complete this TODO.

## Acceptance criteria

- [ ] A fresh image boots dom0 from its selected pool view with sys-ctrl stopped
  and no network access. Qubes services and declared guests start normally.
- [ ] Recovery boots independently when the selected host view is missing,
  corrupt or cannot be assembled; it can select a retained working generation.
- [ ] Dom0 and a guest can reference the same admitted package without a second
  retained full payload copy. Record allocated bytes and explain deliberate
  bootstrap/recovery exceptions and temporary staging copies.
- [ ] Closure views expose only their intended objects. Writable layers remain
  isolated; a guest cannot modify host/shared objects or publish trusted state.
- [ ] Nix queries, imports, supported builds, profile selection and GC behave
  correctly against dom0's composed store and registration database.
- [ ] Sys-ctrl reuses an existing host object through its approved builder view
  without downloading, rebuilding or copying that payload into its upper layer.
  Confirm genuinely missing build inputs are still handled correctly.
- [ ] Updating sys-ctrl's builder base preserves its saved additions across a
  restart and full reboot; interrupted transitions fail closed or recover.
- [ ] Host plan/apply, reboot and rollback preserve exact host/guest generation
  bindings and guest private data, including while sys-ctrl is unavailable.
- [ ] Interruptions during admission, registration, view publication and boot
  selection cannot expose incomplete generations as successfully deployed.
- [ ] GC and compaction preserve running, selected, rollback and persistent
  builder dependencies across repeated updates and remove only unreferenced
  objects. Boot selection and GC races are covered explicitly.
- [ ] Compare cold and repeat planning against TODO 06's measured baseline:
  build time, network/transfer bytes, validation/import time, unique stored
  payload bytes and peak temporary allocation. Report remaining costs rather
  than assuming all first-update work disappears.

Add pytest coverage and follow [machine-deployment.md](../machine-deployment.md)
for physical validation. Keep machine-readable evidence under `.airlock-logs/`.
A successful build or pool mount does not establish boot, recovery or GC safety.

## Starting points

- [Controller ownership](05-sys-ctrl.md)
- [Direct pool deployment and root reuse](06-direct-pool-machine-deployment.md)
- [Existing builder store](../sys-dev-build-store.md)
- [Image pool seeding](../../lifecycle/image/prepared-guest-store.nix)
- [Image assembly](../../lifecycle/image/profiles/usb-image.nix)
- [Builder mount and registration](../../airlock-machine/lifecycle/runtime/tools/scripts/sys-dev-build-store.py)
- [Pool views and backing devices](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/views.py)
- [Builder generation metadata](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/builder.py)
- [Nix local overlay store documentation](https://nix.dev/manual/nix/2.34/store/types/experimental-local-overlay-store)

## Additional implementation entry points

- `airlock-machine/lifecycle/misc/modules/host-store.nix`
- `airlock-machine/lifecycle/image/profiles/usb-recovery.nix`
- `airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/host.py`
- `airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_boot.py`
- `airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_state.py`
- `airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_artifacts.py`
- `lifecycle/image/unified-host-store.nix`
- `lifecycle/image/tools/prepare-host-store.py`
- `lifecycle/image/tools/install-host-store.py`

## Combined physical acceptance, 2026-09-27

The rebuilt image booted successfully and sys-dev was reachable over Tailscale.
Dom0, sys-dev and sys-ctrl reported no failed services; declared autostart VMs
were running. Sys-ctrl had 8192 MiB and sys-firewall as NetVM. Both builders
used their image-seeded generation 2. Sys-ctrl's read-only builder view contained
the exact running host system, absent from its writable upper. A direct
sys-dev pool-admission request was refused (exit 126). These are partial
observations, not completion of the isolation or deployment acceptance criteria.

Persistent boot logs showed pool assembly completing between 6.956 and 8.896
seconds after boot, followed by closure discovery and successful activation.
Pool allocation was 14,040,084,480 bytes, recovery 2,943,549,440 bytes, and host
upper allocation zero at baseline. Dom0 had about 38 GiB free.

The initial source plan built its release but had no successful planning receipt
at cancellation. Its final client exit was 1 with an empty/invalid JSON response
(`Expecting value: line 1 column 1`); the typed status remained `image-baseline`
with no selection. A bounded host Nix query exposed recursive daemon connections:
its NIX_REMOTE setting was rejected by systemd percent expansion, and nix.conf
selected `daemon` rather than the intended local overlay backend. Committed
fix 29bce42 uses an escaped NIX_CONFIG override. A recorded temporary daemon
override then exposed the missing `.links` directory in the host lower view.
Fix 944cbb3 adds a read-only administrative layer during early boot. The runtime
override was removed; the running host view was not replaced. No release was
applied or selected. The new fixes require a rebuilt image and manual boot;
reboot, rollback, GC, persistence and update-performance acceptance remain open.

Evidence: `.airlock-logs/physical-20260927/` (boot and daemon logs, store/volume
observations, source-import receipt, policy rejection, and mutation record).
The remote initial plan log is in sys-dev at
`~/.local/share/airlock/physical-tests/todo050607-20260926-232115/`.
