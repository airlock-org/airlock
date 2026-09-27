# Share immutable store objects between dom0 and guests

Latest status (2026-09-27, USB inspection): **acceptance incomplete**. The
committed release applied successfully and reached `reboot-required`; all
expected running guests matched their selected generations. Shutdown was
orderly. The subsequent boot failed during host database reconstruction.
Local fixes `432a2ae` and `5c87f84` make stored derivations available during
registration and preserve early-boot logging in deployed releases. Nineteen
focused host-store tests pass; a new physical boot remains unverified.
Earlier status sections below are historical.

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

## Second image trial: overlay path validation

The image containing 944cbb3 booted at `sys-dev-1`. The corrected daemon
configuration and `.links` layer were present; dom0 and sys-dev had no failed
units. A host Nix query exposed a third integration defect: OverlayFS retained
`/sysroot/...` lower/upper option strings after switch-root, while Nix correctly
expected `/var/lib/airlock/...`. Nix refused the mismatched mount.

The new implementation mounts the final overlay in a chrooted child during
stage 1, using libc loaded before entering the future root. It shares the
initramfs mount namespace but records final-root paths, keeping Nix's mount
validation enabled. It neither needs an executable in the not-yet-mounted
store nor replaces the store under running host processes.

An isolated mount-namespace probe on the laptop executed the exact new mount
helper with the existing read-only lower view, a copied lower database and a
fresh disposable upper. It reproduced the stage-2 read-only bind mount. The
pinned Nix successfully queried the running host system's hash and added a
small object; the object was verified in the disposable upper. Probe mounts
vanished on namespace exit and temporary files were removed. The active host
store and daemon configuration were unchanged. Evidence:
`.airlock-logs/physical-20260927-r2/nix-mount-probe.log`.

This validates query/write behavior and the corrected mount strings in the
probe, not a completed deployment. A new image boot, daemon query, plan/apply,
reboot, rollback and the remaining acceptance checklist are still required.
Regression tests for the chroot-before-mount ordering and syscall failure were
added, but not run in this session. No image was built by the agent.

## Third image trial: host daemon working, builder restart fix

The c15785b image boots with final-root overlay paths; a query and tiny write
through dom0's actual daemon passed. The written object was verified in its
persistent upper and rooted for later reboot verification. Sys-ctrl sees the
exact host system in its lower view, without a duplicate in its upper.
Host pool assembly completed between 6.942 and 8.631 seconds after boot.
The initial whole-machine plan failed at host binding; no release is selected or applied.

Distinct root/private volume identities were recorded for sys-dev, sys-ctrl and
usr-home. Private-data sentinels were created in all three. These observations
do not yet establish all closure-isolation or shared-root-image criteria.
Malformed control requests and unauthorized direct sys-dev admission failed.

Restarting sys-dev's builder service exposed a cleanup defect: Python's
`os.path.ismount` misses same-filesystem database self-binds. Stop left the
database read-only, so the next metadata registration failed. The source now
reads `/proc/self/mountinfo` for both mount guards and stop cleanup. Four focused
pytest cases passed, including same-filesystem detection and stop/start with a
preserved upper. On the laptop, unmounting only the stale DB bind and starting
the original service recovered it; the added Nix object and private sentinel
survived. The source fix still needs deployment and a real service restart.
No live helper or service override was installed.

Evidence: `.airlock-logs/physical-20260927-r3/`, including `baseline.log`,
`persistence.log`, `policy.log`, `volume-bindings.log`, `builder-upper.log`,
`builder-restart-failure.log`, `builder-recover.log`, `builder-regression.log`
and `mutations.md`. Reboot, recovery, rollback, GC and full performance
acceptance remain open.

### Initial third-trial planning result

The c15785b plan failed after 603 seconds, including a 286.81-second build.
All seven guest root artifacts were reused with zero root payload transfer.
Only sys-dev and sys-ctrl needed guest packages: four objects each, totalling
1,626,112 payload bytes; the other five reused every object. Sender inventory
construction still took 13–81 seconds per guest, so transfer elimination does
not eliminate planning work. These are partial metrics, not end-to-end success.

Dom0 imported the host transport and published its pool generation, then
controller-base refresh rejected an unchanged package because image metadata
uses a Nix base32 hash and live `dump-db` emits hexadecimal. Source now compares
decoded hash bytes, NAR sizes and reference sets, preserving existing record
formatting. Real conflicting identities remain rejected. Seventeen focused host
tests passed. Machine receiver errors now return bounded protocol errors with
coordinator stderr (15 focused protocol tests passed), avoiding an empty-JSON
error at the client. Both fixes still need normal committed-source deployment.
The detached direct binding diagnostic reproduced the failure without selecting
a release; its files are recorded under `/var/tmp/airlock-acceptance-bind-probe`.
Evidence: `admission-progress.log`, `bind-probe-result.log`,
`registration-conflict.log`, `registration-regression.log` and
`protocol-regression.log` in the r3 evidence directory.

### Fixed-source plan succeeded

Committed `3a6f665` completed normal controller planning in 487 seconds:
171.90 seconds building and 314.09 seconds admitting/transferring/planning.
It transferred 38 host objects (117,041,106 compressed bytes; 126,784,376 NAR
bytes); the host transfer/decompression/import stage took 11.09 seconds.
Binding, including host publication and additive controller-base creation, took
74.10 seconds. Guest payload was 5,063,168 bytes, and all seven root artifacts
were reused without payload transfer. The two unique root bases occupy
1,925,120 and 2,211,840 allocated bytes, with 512-MiB and 1-GiB logical sizes.

The exact candidate is
`/var/lib/airlock/machine/candidates/41acc0cba3d02d21fc4ae34870686ea0b2c0f59b365ccaebc8ed3eb8ce926ba2`.
It changes all seven declared guests, retires none, and requires a host reboot
after apply. No apply has run yet. A repeat of the same plan runs detached to
measure reuse before activation. The old binding-diagnostic files were removed;
dom0 had no failed services. Live boot artifact hashes matched both the normal
and recovery entry descriptors (seven unique files); recovery has 804 stored
objects. This is an integrity check, not proof of a successful recovery boot.
Evidence: `fixed-plan-result.log`, `boot-artifacts.log` and
`diagnostic-cleanup.log` in the r3 directory.

### Repeat planning and apply request

The repeat plan returned the exact same candidate and change set. All seven
guests transferred zero package/root payload; host transfer needed zero objects
and only a 23-byte compressed empty-stream envelope. Build/evaluation took
46.25 seconds and submission/planning 307.90 seconds. This is **not an
uncontended speed benchmark**: a concurrent compaction dry run held the pool
lock, and sys-net negotiation recorded 109.52 seconds. The dry run passed,
reported nine segments and no candidates/reclaimable bytes, and moved/deleted
nothing. Remaining time includes rebuilding sender inventories.

The host transport closure has 1,201 objects with no guest root-image or guest
closure-output dependencies. Sys-dev and sys-ctrl use the same immutable root
artifact but distinct writable/private volumes; a root-file write in sys-dev
was absent in sys-ctrl and usr-home and was removed. Deletion/rollback isolation
still needs separate acceptance.

Normal `airlock-sys-dev-machine-deploy apply` was started detached from clean
`3a6f665` at `~/.local/share/airlock/physical-tests/todo050607-r3-apply-20260927-011620/`.
Its durable dom0 result and host reboot still need verification. Evidence:
`repeat-plan.log`, `transport-closure.log`, `root-isolation.log`,
`compaction-result.log` and the r3 mutation record.

## Offline USB inspection after failed restart, 2026-09-27

The mounted USB's durable transaction `1790471934430773647` confirms that
release `3a6f665` applied and reached `reboot-required`. All six intended running
guests matched their selected systems; usr-browser remained stopped as declared.
The selected host was `2dq5fwkk2pllak44q0ycmk9d169jnzk8-nixos-system-dom0-26.11.20260904.801bef6`,
with pool generation 3. The journal ends in an orderly power-off at 02:25 BST.
Only the original successful boot has a persistent journal/early-boot log.

Failed-boot state locates the interruption: the original database is intact,
the replacement database exists with zero registered paths, and published
upper duplicates have been moved into the retained staging directory. Using
read-only USB metadata and local temporary copies reproduced Nix's error:
`store path ...-airlock-host-build-probe.drv does not exist`. Registration
loads parse derivation files; the empty staging store hid the acceptance-test
recipe even though its payload was preserved in the host upper. Exposing the
recipe made the exact reconstructed registration load successfully. This
reproduction and on-disk state identify the defect; the failed boot itself
left no new persistent error log.

Fix `432a2ae` mounts the lower payload view before lower registration, assembles
the final host store before upper database reconstruction, and gives the
staging database a temporary read-only bind of that store. Cleanup unmounts
the bind on failure and leaves the old database in place. Two new regression
cases use real Nix registration with emulated privileged mounts to check saved
derivation/output preservation and failed-load recovery; all 19 focused
host-store tests passed. They do not validate the complete physical mount/boot
sequence. Fix `5c87f84` enables existing early-boot diagnostics in deployed
USB host releases as well as the image; its Nix wiring assertion was added
but not evaluated in this inspection.

The USB's files and boot selection were not edited. Its ESP was mounted
read-only for inspection. No image was built. Evidence is in
`.airlock-logs/usb-inspection-20260927/`: deployment status/journal, original
initrd log, failing/successful database reproduction and pytest results.
Physical reboot, rollback, recovery and remaining acceptance gates stay open.
