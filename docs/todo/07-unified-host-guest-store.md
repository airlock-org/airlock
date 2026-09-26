# Share immutable store objects between dom0 and guests

Status (2026-09-26): proposed design; not implemented or validated. This extends
[direct pool deployment](06-direct-pool-machine-deployment.md) to dom0's own
Nix objects. TODO 06's physical acceptance remains separate.

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

## Current implementation and reusable components

- Dom0 uses an ordinary Nix store for its running system, kernel, boot/recovery
  dependencies and host deployment imports.
- Guest packages are admitted to the shared pool and served through EROFS
  metadata views backed by shared object segments. Guest root artifacts have
  separate verified identities and remain bound to guest generations.
- Sys-ctrl and sys-dev use an image-seeded, frozen builder view containing the
  union of guest runtime closures, selected build tools and offline flake inputs.
  Each has its own persistent writable layer and Nix state.
- That builder view does not automatically include dom0's ordinary store or
  objects admitted after the image was prepared. Changing the base identity is
  currently rejected to protect the persistent upper layer.

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

## Implementation stages

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
