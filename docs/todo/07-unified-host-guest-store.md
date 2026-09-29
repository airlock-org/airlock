# Share immutable store objects between dom0 and guests

Current checkpoint (2026-09-29, r7): **generation 5 boot, persistence and
prepared apply passed** on `sys-dev`. Component `649469b` passed full
`make test-quick`: 714 passed, 24 skipped, 2 warnings. Boot
`f78fc248-b3fb-4ab5-93b0-828220637822` runs the selected `8vlimj...` host with
its own generation-5 view and database. Both host markers, guest private data
and builder additions survived. Input startup needed no reset; the user
confirmed typing, clicks and login. Dom0 has no failed units.

Typed verify completed transaction `1790704764992386034`. The first unchanged
plan took 36.55 s end-to-end (30.40 s preparation); the repeat took 18.20 s
(15.54 s preparation), reusing all seven guest generations with zero package/root
payload. Prepared apply submitted in 8.65 s (3.97 s checks), repeated no
inventory/transfer, and completed transaction `1790705987415026248` without
VM stops or reboot. This validates the unchanged path; changed-update timing,
peak temporary allocation and remaining acceptance are still open.

The isolated root trial `b2d511a` passed generation-6 boot verification:
usr-home's disk and pool record both remained 1536 MiB, and guest/private,
builder and rooted host markers survived. Transaction `1790707116774052820`
is complete. Typed rollback then restored the 1024 MiB root with matching
pool generation 6 and preserved private data; only usr-home was restarted.
Rollback transaction `1790712446217355385` is now verified **complete**.
Return boot `0840d0fe-966a-494c-af20-fa167cd47474` runs generation 5 with its
own view/database, the 1024 MiB guest root, all persistence markers and no
failed units. The changed-root update → reboot → rollback → reboot sequence
passed. TODOs 05–07 remain open; broader outage/interruption criteria are
separate. The memory-only plan `d827ad2` passed with all seven guest generations
reused and zero guest package/root payload; it has not been activated.
Component `3a4e3d2` adds only the missing privileged storage pytest worker
and its bounded loop-cleanup wait; installed runtime remains `649469b`.
The worker passed on dom0 with real loop/device-mapper/EROFS devices, exact
closure membership, rejected writes, shared extents and peer survival after
detach. Local unprivileged execution correctly reports skipped. Both builders
are restored to clean source `649469b`; the memory trial was never activated.
Sys-ctrl also reused a host object offline from its lower layer with no upper
copy, and rejected an unavailable object. That does not yet demonstrate a
successful download of a genuinely missing build dependency.
No new image was built and no missing-view fault is active. Evidence lives
under `.airlock-logs/deployment-performance/` and the r7 physical evidence
directory (`root-trial-boot6` and `root-trial-rollback-final`).

The generation 2 → 1 → 2 isolation/rollback sequence passed. Earlier r7 quick
report: 674 passed, 24 skipped, zero failures/errors. Evidence:
`.airlock-logs/physical-20260929-r7/`. TODOs 05–07 remain open; earlier dated
results remain evidence for their stated revisions.

Historical continuation (2026-09-28): maintenance fix `fbe6062` booted successfully; typed
verification marked transaction `1790546188487824369` complete. Saved data,
builder additions and boot records passed; installed maintenance code hashes
match the commit. Recovery, GC and compaction passed. A machine plan for the
root-artifact trial is running. Overall acceptance remains open. See the final
continuation.

Historical status (2026-09-27, fourth image): **acceptance incomplete**. Normal
return from independent recovery passed. A committed usr-home-only update and
rollback also passed without a host reboot; only usr-home changed, the marker
was removed, and private data survived. Original source `5c87f84` is restored
in both builder VMs. Installed validators reject damaged metadata in disposable
copies. The missing-view recovery console test passed: the user booted recovery,
confirmed the metadata was absent, restored it, and successfully selected host
generation 1. The subsequent normal reboot passed: exact generations, saved
data and restored metadata hashes were verified, and the diagnostic directory
was removed. Other acceptance criteria remain open.

Historical follow-up: maintenance and the queued root-image trial were stopped
at user request. GC passed; compaction is incomplete. See the final section
for the unfinished transaction, source state and performance follow-up.

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
  the one selected EROFS view and its own OverlayFS upper before stage 2. Python storage
  primitives, Nix registration tools, loop and device-mapper tools are carried
  in the initramfs; Qubes services and networking are not dependencies.
- Normal Nix clients use the daemon, whose backend is the local overlay store.
  Imports and builds write through `/var/lib/airlock/host-store/active/upper`,
  a bind-mounted generation directory. Pool publication freezes the target
  system plus registered host additions and their dependencies into one view.
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
  recoverable directory-rename sequence for each generation's database. Boot
  does not load other generations or run retired-generation collection. Host
  publication snapshots SQLite trust/output metadata for the next generation.
  Explicit retirement and pool GC/compaction remain separate maintenance work.
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
`airlock-host-store status` reports pool, active upper, all generation-private
state and recovery allocation. These are
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

## Compaction performance implementation

The interrupted physical run read over 60 GB across GC/compaction despite
finding no compaction candidates. Implement these first without weakening the
existing checksum or running/retained-reference checks:

- [x] Return before creating a transaction, sealing a segment or refreshing
  maps when there are no relocation candidates or reclaimable segments.
- [x] Reuse successful extent verification within one maintenance operation
  under the admission lock. Cache keys must include backing-file identity and
  change metadata, offset, length and expected digest. Never persist verification
  results across operations; changed files and corrupt/truncated data must fail.
- [x] Refresh only closure maps referencing relocated objects; preserve untouched
  map files byte-for-byte. Recovery must still repair all potentially interrupted
  relocations, even when a transaction did not record its final moved-object list.
- [x] Report elapsed time, payload bytes hashed and verification cache hits in
  maintenance results. Local fixtures cover zero-work, shared-object and
  actual-movement cases; physical timing and disk-read measurements remain open.
- [x] Add regression coverage for no-work behavior, cache invalidation/corruption,
  selective refresh, retained references, real relocation and interrupted retry.

A later optimization is to separate metadata-based maintenance planning from a
full integrity scrub, verifying payloads when copying/reusing them. That changes
when corruption is detected and needs a separate design and acceptance pass;
it is not part of the first three optimizations. The first implementation will
still read each unique referenced extent for verification, so a zero-work run
is not promised to be instant on USB.

Deploy through normal machine updates after local validation; no new USB image
is required by these Python-only changes. Recover the stopped transaction using
the updated implementation, then resume the outstanding TODO 05–07 acceptance.
Previously completed independent tests remain evidence; repeat the affected
maintenance, retention, interruption and deployment checks. Do not mark these
TODOs complete solely because the implementation or local tests pass.

Local implementation: machine commit `fbe6062`. In addition to the performance
changes, compaction/recovery preserve maps protected by `host-retained` and
`builder-persistent` pins byte-for-byte. Their original payload segments remain
retained, preventing relocated shared objects from invalidating boot checksums
or builder pin bindings.

Focused local validation: **67 passed**, including 10 new real-segment pytest
cases. Coverage includes unchanged no-work pools, one verification per unchanged
extent per operation, file replacement/overwrite/truncation and changed-digest
rejection, selective map refresh, actual relocation, running/persistent pins,
collection after pin release, interruption recovery and retry. These fixtures
use real segment files and JSON maps, not mounted EROFS views or physical VMs.
Evidence: `.airlock-logs/compaction-performance/pytest-report.json`.

Reproduce the focused checks:

```bash
make test-quick PYTEST_DEFAULT_TARGETS='airlock-machine/tests/test_pool_maintenance.py airlock-machine/tests/test_pool_artifact.py airlock-machine/tests/test_host_store.py' PYTEST_LOG_DIR=.airlock-logs/compaction-performance PYTEST_ARGS=-q
```

The `verification` result records payload bytes hashed by pool verification and
source-copy routines, cache hits and operation elapsed seconds. This is not a
block-device read counter or peak allocation measurement. Live acceptance must
also record physical I/O and storage allocation separately. Cache lifetime ends
on both successful and failed operations; no verification state is persisted.
No image was built, and no live deployment or test was performed for this change.
The stopped transaction and imported trial source still need the documented
recovery/restoration before resuming normal deployment and TODO 05–07 acceptance.

## Acceptance criteria

- [ ] A fresh image boots dom0 from its selected pool view with sys-ctrl stopped
  and no network access. Qubes services and declared guests start normally.
- [x] Recovery boots independently when the selected host view is missing,
  corrupt or cannot be assembled; it can select a retained working generation.
- [x] Dom0 and a guest can reference the same admitted package without a second
  retained full payload copy. Record allocated bytes and explain deliberate
  bootstrap/recovery exceptions and temporary staging copies.
- [x] Closure views expose only their intended objects. Writable layers remain
  isolated; a guest cannot modify host/shared objects or publish trusted state.
- [ ] Nix queries, imports, supported builds, profile selection and GC behave
  correctly against dom0's composed store and registration database.
- [x] Sys-ctrl reuses an existing host object through its approved builder view
  without downloading, rebuilding or copying that payload into its upper layer.
  Confirm genuinely missing build inputs are still handled correctly.
- [ ] Updating sys-ctrl's builder base preserves its saved additions across a
  restart and full reboot; interrupted transitions fail closed or recover.
- [x] Host plan/apply, reboot and rollback preserve exact host/guest generation
  bindings and guest private data, including while sys-ctrl is unavailable.
- [ ] Interruptions during admission, registration, view publication and boot
  selection cannot expose incomplete generations as successfully deployed.
- [x] GC and compaction preserve running, selected, rollback and persistent
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

- [Controller ownership](../completed/05-sys-ctrl.md)
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

## Fourth image: boot and isolated transition checks, 2026-09-27

The booted source is clean `5c87f847090724e2fd474076f0c669079cbc1f7f`.
Host pool assembly completed at 9.484 seconds (started at 7.122); the log shows
store assembly preceding database replacement. Dom0, sys-dev and sys-ctrl
had no failed services. Declared guests were running, usr-browser halted, and
sys-ctrl had 8192 MiB with sys-firewall as NetVM. The host Nix daemon accepted
queries and a small build. Both the output and derivation are rooted for the
next reboot, explicitly retaining the case that previously broke early boot.

An isolated mount-namespace probe used the installed storage package, a SQLite
backup and read-only binds of the store. The actual database replacement helper
completed with the saved derivation and its output mapping intact. Temporary
mounts/files were removed; the live database was not replaced. This validates
the previously failing operation, not a complete host reboot.

Both builder-store services restarted and preserved their base identities and
added package. Sys-ctrl was then shut down: typed status/release inventory still
worked through sys-dev, and usr-home/host store contents remained readable.
Restarting sys-ctrl preserved its private sentinel and added package. The exact
running host system is present in the controller lower without an upper copy.
Malformed machine-control input exited 1; sys-dev direct pool admission was
refused with exit 126. No activation occurred during these checks.

The normal plan ran detached in sys-dev at
`~/.local/share/airlock/physical-tests/todo050607-r4-plan-20260927-163453/`.
Evidence and intentional test-state details: `.airlock-logs/physical-20260927-r4/`,
including `baseline.log`, `persistence.log`, `db-probe.log`,
`builder-restart.log`, `controller-outage.log`, `policy.log` and `notes.md`.
A synchronous allocated-byte inventory exceeded the observer's 45-second limit
and yielded no report; it is not counted as a passed check. No inventory
process remained at the following observation.


### Fourth-image plan/apply result: manual reboot required

The normal controller route planned and applied revision `5c87f84` to candidate
`5266f735601f2bd835b2f411d81710f2bbb9b1fd4a97a04ca513709dd4348f1b`.
The initial plan took 574 seconds: build 254.43 seconds, submission 318.24 seconds.
Seven guest root artifacts were reused with zero root payload. Guest package
payload totaled 1,626,112 bytes; host transfer was 13 objects, 1,029,060 compressed
bytes (8,022,408 NAR bytes). Sender inventories took 220.76 seconds and binding
validation 69.86 seconds, so reuse does not yet make planning immediate.

Apply resubmitted the same candidate in 244 seconds: build 45.65 seconds,
submission 197.58 seconds, including 186.45 seconds of sender inventories.
It transferred zero guest packages, root payloads or host objects; the empty
host stream was 23 compressed bytes. Binding validation fell to 2.95 seconds.
These timings end at worker launch, not completed activation.

The detached worker `airlock-machine-deploy-2b1dcc273e3e` finished successfully
(exit 0). Transaction `1790527932393756334` and typed verification both report
`reboot-required`. All six running guests match selected generations and
usr-browser remains halted. Sys-ctrl's recorded outcome agrees. Dom0 and
sys-ctrl have no failed units. Sys-dev, sys-ctrl and usr-home private sentinels,
both builder additions, and the rooted host derivation/output survived.
The controller builder advanced from base generation 2 to 3; sys-dev stayed at 2.

Selected host pool generation 2 is
`/nix/store/pbd5g5nn8ykzva6vzrw7s1nv0l336bi8-nixos-system-dom0-26.11.20260904.801bef6`.
Its installed EFI/kernel/initrd artifacts passed the host artifact verifier;
the default entry selects it with early logging parameters enabled. Dom0 is
still running the image baseline until the user reboots into normal `Airlock`.
No worker or diagnostic override remains; intentional persistence sentinels and
GC roots are retained for reboot/rollback checks. No reboot was performed.

Evidence in `.airlock-logs/physical-20260927-r4/`: `plan-receipt.json`,
`apply-receipt.json`, `apply-status.json`, `apply-verify.json`, `post-apply.log`,
`boot-selection.log` and `final-state.log`. The detached client log is in sys-dev
under `~/.local/share/airlock/physical-tests/todo050607-r4-apply-20260927-164807/`.
Full host reboot, rollback, independent recovery and remaining retention/fault
acceptance are still outstanding; these TODOs are not complete.


### Selected-host reboot passed; rollback awaits its reboot

The user rebooted into selected host
`/nix/store/pbd5g5nn8ykzva6vzrw7s1nv0l336bi8-nixos-system-dom0-26.11.20260904.801bef6`.
Typed verification changed transaction `1790527932393756334` to `complete`:
the host and all six intended running guests matched, with usr-browser halted.
There were no failed dom0 units. Sys-dev/sys-ctrl/usr-home sentinels, both saved
builder objects and the host derivation's validity, exact output mapping and
output contents survived. Sys-ctrl retained builder base generation 3.

The actual early-boot journal shows host assembly from 7.202 to 27.805 seconds:
two bases and 1,385 store paths, lower database loading at 12.024 seconds,
final overlay at 26.146 and host database replacement at 26.255. This directly
exercises the saved-derivation case that broke the previous image's reboot.
The mounted host overlay uses final `/var/lib/airlock` paths. This is a passed
normal reboot, not proof of independent recovery or arbitrary interruption safety.

Release inventory protects the current release through running-host, selected
profile and selected-release references. The image baseline's prune plan is
blocked as the selected previous release. No pruning or GC was performed.

Sys-ctrl was deliberately stopped and confirmed halted while sys-dev and
usr-home remained running. Sys-dev's normal typed rollback command then
started detached worker `airlock-machine-deploy-2af15f761f7e`. The worker exited 0;
transaction `1790529121150146796` and typed verification are `reboot-required`.
The image's sys-dev/sys-ctrl versions were restored and the declared running
set matches its topology. Sys-ctrl restarted normally. Private sentinels,
both builder additions and the host derivation/output survived again; builder
bases remained sys-dev generation 2 and sys-ctrl generation 3.

Host pool generation 1 is now selected:
`/nix/store/inkmi5z7hcmmka8051kqydgxxbchh31v-nixos-system-dom0-efi-raw-26.11.20260904.801bef6`.
The installed boot artifacts, default entry and early logging passed verification.
Dom0 still runs the newer host until a manual normal `Airlock` reboot. No worker
or diagnostic override remains. Persistence test files and GC roots remain
intentionally for the rollback reboot. Full rollback acceptance remains pending.

Evidence: `.airlock-logs/physical-20260927-r4/reboot-verify.json`,
`post-reboot.log`, `rollback-preflight.log`, `reboot-release-list.json`,
`baseline-prune-plan.json`, `rollback-request.log`, `rollback-status.json`,
`rollback-verify.json` and `post-rollback.log`. Detached client directory:
`~/.local/share/airlock/physical-tests/todo050607-r4-rollback-20260927-171200/`.


### Rollback reboot and physical storage checks passed

The user rebooted into the image baseline host (pool generation 1). Typed
verification reports transaction `1790529121150146796` as `complete`, with
exact host and six running guest identities. Dom0 has no failed units.
All three private sentinels, both saved builder objects and the saved host
derivation's validity, output mapping and contents survived. Builder bases
remain sys-dev generation 2 and sys-ctrl generation 3. Host assembly completed
from 7.220 to 13.581 seconds, retaining both host bases and 1,385 registered paths.

Live Qubes volume inspection confirms distinct writable root and private
volume IDs for sys-dev and sys-ctrl, despite their common immutable root artifact.
A temporary probe wrote different values to the same relative filenames on
each VM's root filesystem (`/var/tmp`, confirmed on `/`) and private home.
Each read its own value, and deleting sys-dev's files preserved sys-ctrl's.
Guest package-store writes were refused and the host system path was absent
from guest package views. All probe files were removed. This covers writes and
file deletion; it does not claim a disposable VM's volume deletion or a genuine
root-artifact change has been exercised.

The first probe attempted `/var/lib` via a requested root shell. Sys-ctrl's
policy forces that service to its ordinary user, so the write was correctly
refused; the corrected probe used an ordinary-user-writable root directory.
No policy or account setting was changed. Sys-dev allowed its development root
shell, whose write into the read-only package store was also refused.

The selected host and sys-dev views share 496 package objects totaling
1,674,595,840 bytes of payload extents. Every shared object maps to the exact
same segment, offset, length and hash in both views; none exists in the host
upper layer. Fresh allocated-byte measurement reports:

- Shared pool: 14,277,185,536 bytes.
- Host writable upper: 36,864 bytes.
- Independent recovery store: 2,943,594,496 bytes.

The recovery store and copied EFI boot artifacts are deliberate duplicates;
image assembly and admission can also require temporary copies. These figures
are current allocation, not a measurement of peak temporary storage. The host
transport has 1,201 requisite paths and contains no guest root-image or guest
closure outputs; sampled guest-only outputs are absent from the host upper.

Evidence: `.airlock-logs/physical-20260927-r4/rollback-reboot-verify.json`,
`post-rollback-reboot.log`, `isolation-inspect.log`, `isolation-probe.log`,
`shared-extents.log`, `recovery-inventory.log` and `storage-observe-summary.log`.
The reused `post-reboot.log` now also reflects the rollback boot; the earlier
forward-boot verification remains in `reboot-verify.json` and its boot journal
in `rollback-preflight.log`.


Maintenance performance follow-up: the read-only compaction plan verifies full
payload hashes in `current_live_locations`, then again while collecting physical
references through `live_segment_references`. On this USB, process I/O exceeded
16 GB before completion. Investigate reusing validated extent information within
the same locked operation; do not remove corruption checks or allow compaction
and boot selection to race. This observation is separate from deployment
transfer bytes and does not imply objects were copied or rewritten.


The detached allocation/compaction observation job completed successfully in
12 minutes 40 seconds, with 21.7 GiB read from disk and a reported 5.6 GiB peak
memory footprint. Its compaction dry run passed: all nine segments have physical
references, zero candidate/reclaimable segments, zero objects to move and zero
bytes immediately reclaimable. No package payload was moved or deleted. Actual
GC/reclamation and interruption safety remain untested on this installation.
`compaction-final.log` records per-segment references and the service journal;
raw reports remain under `/var/log/airlock/physical-tests/r4/` in dom0. The job's
first attempt lacked PATH and exited 127; the corrected run's command receipts
both exited 0. No observation worker remains running.

The next user-assisted test is `Airlock (recovery)`: inspect its recovery marker
and `/nix/store` mount, then use `airlock-host-store recover` to select the
retained normal host. The console procedure is in
[machine-deployment.md](../machine-deployment.md#independent-recovery-acceptance).
The recovery profile and boot-menu entry were verified present; independent
recovery boot and the missing/corrupt-view fault trial have not yet been run.


### Independent recovery smoke test passed (user console)

The user booted `Airlock (recovery)` and supplied a console photograph showing:

- `/etc/airlock/pool-recovery` identifies independent recovery with networking
  and guests disabled.
- `/nix/store` comes from `/dev/sda2[/var/lib/airlock/recovery-store]`, not the
  normal pool overlay. The top mount is read-only (`ro,nosuid,nodev`).
- `sudo airlock-host-store recover "$(readlink -f /nix/var/nix/profiles/system)"`
  returned a successful host-store record for generation 1 and the original
  `inkmi5z7hcmmka8051kqydgxxbchh31v` image host, then returned to the prompt.

This verifies independent recovery boot and retained host/EFI selection with
an intact pool. Missing/corrupt-view recovery is still an outstanding fault
trial. The next step is a manual normal `Airlock` reboot and typed verification
after Tailscale reconnects. No reboot was initiated by the agent.
Evidence: `.airlock-logs/physical-20260927-r4/recovery-console.png`.


### Recovery return and committed single-guest update passed

Normal boot after the independent recovery smoke test returned to `complete`,
with exact host/guest bindings, private sentinels, builder additions and saved
host derivation/output mapping intact. No failed dom0 units were reported.
Evidence: `recovery-return-verify.json` and `post-recovery-return.log` under
`.airlock-logs/physical-20260927-r4/`.

An isolated machine acceptance branch `acceptance/r4-guest-marker` committed
`a4163eae6ab8f4417b04743c878b2c9850b4569c`, adding only the usr-home file
`/etc/airlock-acceptance-r4` with value `guest-update-r4`. A matching isolated
parent commit `43dc9f5` pins it. Both main checkouts retain the original product
source. The normal source import and sys-ctrl guest-plan/guest-apply route were
used, with no image build or direct live configuration edit.

Guest plan took 152 seconds and admission transferred four objects totaling
443,392 bytes, reusing 974 objects and the unchanged 1-GiB logical root artifact
with zero root payload. Apply resubmission took 24 seconds before worker launch,
reused all 978 objects and sent zero package/root payload. Plan and apply have
distinct candidate IDs because their admission generations are 3 and 4;
the closure hash and system are identical.

Apply worker `airlock-machine-deploy-6a9df28d7307` exited 0. Transaction
`1790539581325101420` verified `complete`; only usr-home changed to
`/nix/store/ffjpr7vmc6g6nqg8m4ybzpzdn9p26bdp-nixos-system-usr-home-26.11`.
The marker and unchanged private sentinel were read from the running VM.
Guest rollback transaction `1790539773592303961` also verified `complete`,
restoring the exact pre-test host/guest bindings and removing the marker.
No host reboot was needed. Sys-dev and sys-ctrl were restored to clean source
`5c87f84`; the controller's cached original release build succeeded afterward.

Evidence: `guest-plan.log`, `guest-receipts.log`, `guest-apply.log`,
`guest-verify.json`, `guest-marker-check.log`, `guest-rollback-verify.json`,
`guest-rollback-check.log` and `restored-source-build.log`. Test commits and
worktrees are retained locally for reproducibility, not selected on the machine.

The installed host validator was also exercised against disposable copies of
actual generation metadata. Corrupt view checksums, missing retention pins,
uncommitted generations and wrong installed-boot-artifact hashes were rejected.
Original metadata and the bootable store were unchanged by this probe, and its
temporary directory was removed. Evidence: `metadata-fault-probe.log`.
This probe does not substitute for a physical recovery boot with a missing view.

### Missing-view recovery fault setup (subsequently restored)

After the checks above, the selected generation-1 host metadata was moved from
`/var/lib/airlock/closure-store/views/by-closure/27bde8f537a1b1f733fc8046f16307fc98aab16cd6b28ef1c691436ea8e8decc/store.erofs`
to `/var/lib/airlock/recovery-probe/store.erofs`. Its hash was verified against
the original record. The running mounted host remains usable, but a normal
boot will fail until restoration. Package payload segments were not changed.
`recovery-probe/view` is a symlink to the original metadata directory, and
`recovery-probe/original.json` records the exact binding. Evidence:
`.airlock-logs/physical-20260927-r4/recovery-fault-prepare.log`.

The user must choose **Airlock (recovery)** at the next boot, then run:

```bash
sudo sh -c 'cd /var/lib/airlock/recovery-probe && test ! -e view/store.erofs && echo MISSING_VIEW_CONFIRMED && mv store.erofs view/'
sudo airlock-host-store recover "$(readlink -f /nix/var/nix/profiles/system)"
```

Capture the console output before another reboot. A subsequent normal boot
must verify exact generations and data again. Remove the diagnostic directory
only after verifying that the original metadata path and hash are restored.
No deployment/build worker remains running. Fault-test acceptance is pending.


### Missing-view recovery console result

The corrected user photograph shows a recovery login followed by
`MISSING_VIEW_CONFIRMED` from the conditional restoration command. The move
completed without an error, and `airlock-host-store recover` then returned the
original generation-1 host record successfully. This establishes recovery boot
with the selected host metadata absent, followed by restoration and selection
of the retained normal host. The previous repeated photograph was not counted
as evidence for this fault trial.

Evidence: `.airlock-logs/physical-20260927-r4/missing-view-recovery-console.png`.
The subsequent normal boot passed. Typed verification reports `complete` with
exact baseline host/guest bindings. Saved guest private data, both builders'
rooted additions and the host derivation/output survived. Early store assembly
completed between 7.111 and 13.500 seconds after boot; dom0 has no failed units.
The selected generation-1 metadata hash and installed boot artifacts were
verified before removing the diagnostic symlink, record and directory. No
missing-view diagnostic mutation remains.

Evidence under `.airlock-logs/physical-20260927-r4/`:
`missing-view-return-verify.json`, `post-missing-view-return.log`, and
`recovery-fault-cleanup.log`. This closes the physical missing-view recovery
trial; corrupt-metadata rejection was separately checked with disposable
metadata copies, not a second physical corrupt-view boot.


### Maintenance and interruption acceptance follow-up

After the missing-view recovery return, the focused `make test-quick` selection
of `test_host_store.py`, `test_pool_artifact.py` and `test_machine_deploy.py`
passed: **165 tests**. This covers simulated interrupted admission/publication,
registration recovery, invalid artifacts, changed-root transfer, retention and
deployment resume. These are automated fixtures, not physical power cuts.
Evidence: `.airlock-logs/physical-20260927-r4/focused-tests/pytest-report.json`.

A new regression test uses the real pool file lock with concurrent host
selection and maintenance entry points. Maintenance stays blocked until boot
selection has written its selected record; the focused check passed. Machine
commit `089b42b` adds only this test. The live product remains `5c87f84`.
Evidence: `selection-race-tests/pytest-report.json` in the same evidence directory.

The existing external quick-suite report was located and copied to
`previous-quick-suite/pytest-report.json`: **616 passed, 24 skipped**, recorded
2026-09-27 before this follow-up. Its `closure-store-linux-storage` result
explicitly says **skipped** for lack of privileges, even though the enclosing
pytest case reports passed. Do not count that case as real loop/device-mapper
integration evidence.

Actual live pool maintenance was started detached as `airlock-r4-maintenance`
with default two-generation retention, followed by one bounded compaction run.
It snapshots manifests, checks retained records after each operation and checks
selected host/boot artifacts. Results are under
`/var/log/airlock/physical-tests/r4/maintenance` on dom0; completion and post-run
guest verification are pending. This operates on the Airlock closure pool, not
Nix garbage collection of a host `/nix/store`.


A follow-up root-image trial is queued as detached unit
`airlock-r4-root-trial`, waiting for successful maintenance before using the
normal typed guest-plan/apply route. Isolated committed source `db62fb5` changes
only usr-home's root size from 1024 to 1536 MiB; it has been imported into
sys-dev. The worker requires only usr-home to change, no host reboot, exact
source provenance and unchanged other guest bindings. Its logs and durable
phase are in `/var/log/airlock/physical-tests/r4/root-trial`. Root/data checks,
rollback and restoration of original source remain pending. The main product
configuration is unchanged. Do not reboot while these workers are active.


### Maintenance stopped at user request

Both `airlock-r4-maintenance` and the queued `airlock-r4-root-trial` were stopped
and verified inactive. The root trial never left `waiting-for-maintenance`.
GC completed successfully, removing obsolete sys-ctrl-builder generation 1 and
its unreferenced view; it removed no payload segments or root artifacts.
Compaction did not finish. Its transaction `22b13939caaf47c785271ac3b09a46db`
remains `preparing` with **zero candidate segments**. Do not count this as a
compaction pass or restart the queued trial automatically. Transaction recovery
is pending; no expensive recovery scan was started after the stop request.

The selected host and installed boot artifacts were verified after stopping;
there are no failed dom0 units. Sys-dev still has the isolated root-trial source
`db62fb5` imported, but it has not been deployed. Restore source before ordinary
deployment. Evidence: `maintenance-stop.log` and `maintenance-stop-verify.log`
under `.airlock-logs/physical-20260927-r4/`.

Performance follow-up: compaction repeatedly hashes canonical object payloads
while finding references and refreshing every closure map, including shared
objects repeated across views. Even zero-candidate runs seal the open segment,
refresh all maps, and recompute references. Add an early return when there is
nothing to move or retire; reuse verified extent results within the locked
operation; refresh only maps affected by relocated objects. Keep checksum
verification for moved data and conservative running/retained reference checks.
Measure payload bytes read and elapsed time, including the zero-work case.


### Compaction-fix deployment continuation (2026-09-27)

The live baseline remains healthy and both stopped r4 workers remain inactive.
Committed source `fbe6062` replaced the unused root-trial checkout in sys-dev and
was forwarded to sys-ctrl through the normal typed plan route. A detached plan
is building; logs: `~/.local/share/airlock/physical-tests/r5-plan` in sys-dev.
The interrupted zero-candidate compaction transaction remains pending recovery
with the updated code; the old expensive recovery path was not restarted.

Read-only validation found that the interrupted old compaction had refreshed
retained dom0 generation 2's blob-map segment size/status fields, invalidating
its boot-record checksum. The running image baseline and its recovery choices
still validated. Under the pool admission lock, generation 2's exact original
map was reconstructed from the retained manifest's segment descriptors and
restored **only after its SHA-256 matched the original boot record**:
`aa9c2c74de9ed210c496c15e9ea18447eb543ef22a8c0ac68f844100e39682e6`.
Host validation then passed. No boot record or package payload was changed.
This repair removes a mutation left by the old diagnostic run; the committed
`fbe6062` pinned-map protection prevents recurrence in the new implementation.

Evidence: `.airlock-logs/physical-20260927-r5/retained-hosts.log`,
`inspect-retained-map.log`, and `restore-retained-map.log`. The pre-restoration
map and receipt are retained in `/var/log/airlock/physical-tests/r5` on dom0.
No new USB image has been built. New-release activation and reboot remain next.


### Maintenance release selected; manual reboot pending (2026-09-27)

Normal typed plan/apply through sys-ctrl selected committed revision `fbe6062`.
Transaction `1790546188487824369` reached **`reboot-required`**. All six running
VMs match their selected systems; usr-browser remains intentionally halted.
Selected dom0 generation 3 is
`/nix/store/vbj6nh1ivynjh75bsvnm7jw2m83nnsi5-nixos-system-dom0-26.11.20260904.801bef6`;
the running host is still image generation 1. No new image was built.

The plan took 920 seconds: release build 229 seconds, submission 688 seconds,
including 404 seconds of guest inventory calculation and 235 seconds of host
binding. It transferred 5,032,960 guest package bytes and 117,052,461 compressed
host bytes (33 missing paths); every root artifact was reused with zero root
payload transferred. Apply's submission process exited zero after 462 seconds
(45 seconds cached build, 416 seconds submission); the independent apply worker
subsequently reached reboot-required. Its repeated submission reused all guest
objects and roots and transferred zero host paths. These are deployment timings,
**not compaction performance results**.

After the VM restarts, checks passed for private-data sentinels in sys-dev,
sys-ctrl and usr-home, both builders' saved Nix objects, and the saved host
output and derivation. All five retained host boot records validate, including
the restored generation 2 map and new generation 3. Selected generation 3's
installed boot artifacts also validate. Dom0 has no failed units, and no
deployment or r4 maintenance/trial units remain loaded.

Evidence: `.airlock-logs/physical-20260927-r5/` files `plan.json`, `apply.json`,
`apply-exit-elapsed.log`, `apply-status.json`, and `pre-reboot.log`.
Next: the user reboots into normal Airlock; verify exact host/guest selections
and installed maintenance code, then recover the stopped compaction transaction
and measure compaction/GC. The stopped root trial remains stopped. Acceptance
for TODOs 05–07 is still incomplete.


### Maintenance release boot verified; follow-up running

After the manual reboot, typed `verify` completed transaction
`1790546188487824369`: running and selected dom0 are generation 3 (`vbj6nh1…`),
and all running guests match selection. The three private-data sentinels, both
builders' saved objects, host output/derivation and all five retained host boot
records passed again. Installed `maintenance.py` and `segments.py` hashes match
committed `fbe6062`. Dom0 has no failed units. Evidence: r5
`post-reboot-verify.json` and `post-reboot.log`.

Detached unit `airlock-r5-maintenance` runs recovery of the stopped transaction,
GC, then bounded compaction, validating retained manifests and host boot records
after each operation and saving verification-byte/timing metrics under
`/var/log/airlock/physical-tests/r5/maintenance`.

The root trial was rebuilt on the maintenance fix: isolated committed source
`da65cc9` changes only usr-home's root size from 1024 to 1536 MiB. It is imported
into sys-dev. Detached `airlock-r5-root-trial` waits for successful maintenance
before using typed guest-plan/apply, checking that only usr-home changes and no
host reboot is needed. It stops for root/data verification after activation.
Logs: `/var/log/airlock/physical-tests/r5/root-trial`. The memory-only follow-up
is prepared locally as `3c74286`; neither trial changes the main product config.
Restore source `fbe6062` and the selected baseline after the trials. Do not count
these in-progress checks as passes or reboot while they run.


### Exact guest views and maintenance progress

All six running guests expose exactly their approved manifest's store paths:
sys-dev 617, sys-ctrl 606, sys-gui 1513, sys-net 639, sys-firewall 597 and usr-home
977. There are no extra or missing entries (`r5/closure-membership.log`). Combined
with r4's root/private-write isolation, peer deletion, read-only store rejection
and denied unauthorized pool request, this closes the closure-view isolation
criterion. A truncated source upload was rejected before checkout changes
(`r5/truncated-source.json`); this is source-transport coverage, not a simulated
power cut during package admission.

Recovery passed in 422.40 seconds, hashing 14,338,229,760 payload bytes with
11,534 verification-cache hits. GC passed in 358.89 seconds, hashing
14,475,450,880 bytes with 2,417 cache hits. GC removed only obsolete sys-ctrl-builder
generation 2 and its view; no root artifacts or payload segments were removed.
The worker's retained-map/manifests and boot-artifact checks passed after both
operations. Compaction is still in progress; each operation has its own cache.

The planned memory-only trial is now `8621216`, based directly on `fbe6062`,
so it can follow rollback of the root trial while keeping the original 1 GiB
root. Prepared but unused `3c74286` is superseded and will not be deployed.


### Updated maintenance passed; root trial planning

R5 recovery, GC and real compaction all passed in 1,162.19 seconds total.
Compaction completed in 358.84 seconds with `noop: true`: no candidate segments
or reclaimable segments, no new transaction, no map refresh or segment sealing.
It hashed 14,475,450,880 bytes and reused 2,417 checksum results. All 29 retained
manifests, retained host maps/boot records and selected boot artifacts survived
each operation unchanged. The interrupted old compaction is recovered.

The older r4 dry run took about 760 seconds; the stopped old actual compaction
read over 61 GB without completing. These are not controlled benchmark runs:
pool contents and cache state differ, and old physical I/O counters are not the
same metric as new payload bytes hashed. The updated no-work path completes
and eliminates repeated hashing within an operation, but still reads about
14.5 GB per maintenance operation on this pool. Separating routine metadata
maintenance from full integrity scrubs remains the documented future design.

Physical no-work maintenance/preservation and r4 GC deletion, combined with
the 67 local maintenance/host tests (real segment relocation, pinned maps,
interruption/recovery and released-extent collection) and the real lock race
check, close the maintenance criterion. Actual relocation was tested locally;
this physical pool had no relocation candidates. Evidence: r5
`maintenance-progress.log`, `maintenance-results.log`, and dom0's corresponding
`/var/log/airlock/physical-tests/r5/maintenance/*.json`. The root-image trial has
advanced to planning; root boot and rollback checks remain pending.


### Root-trial route corrected

The first r5 root trial used guest-plan, which updates guest packages and
intentionally inherits the selected root artifact. It admitted usr-home
generation 6 with the original 1 GiB root and unchanged system, then rejected
planning with `admitted guest system is already selected`. This was a test-route
error, not a failed new-root activation: no guest was stopped or switched.
`root-trial` and its follow-up `root-finish` exited before mutating runtime state.
Evidence: r5 `root-progress.log`, `root-failure.log` and
`unchanged-root-rejection.log`.

Detached `airlock-r5-root-machine-plan` now uses the normal machine-plan route
for committed `da65cc9`, which includes root artifacts and topology changes.
It checks the 1536 MiB usr-home root and reuse of every other root before
stopping for plan review. Logs: `/var/log/airlock/physical-tests/r5/root-machine`.
Do not assume this topology change needs no host reboot; inspect the resulting
plan before apply. The live maintenance release remains selected and healthy.


### Changed-root machine plan passed; apply submitted

The corrected `da65cc9` machine plan passed in 579.31 seconds: build 61.08
seconds, submission 516.73 seconds, including 133.36 seconds binding the host
view. Exact topology comparison against selected `fbe6062` found only one
change: usr-home's root source. The plan changes/stages/stops only usr-home,
retires/restores nothing, and requires a host reboot for the updated topology.

The new root is 1,610,612,736 logical bytes (1536 MiB), occupies 2,953,216
allocated bytes in the pool, and transferred 2,949,690 payload bytes. Its hash
is `176a3746b7a0f0d0b8facd17de11b504dcd2a913d8f688b7859e846ec021cb81`.
All other roots were reused with zero payload. All guest package objects were
reused with zero package payload; dom0 received four missing paths totaling
113,022 compressed bytes, imported in 0.165 seconds. Guest/root byte counts
exclude protocol framing. Evidence: r5 `root-plan.json` and
`root-machine-apply-progress.log`.

Detached `airlock-r5-root-machine-apply` passed the exact topology guard and
started normal typed apply. It records submission/activation under
`/var/log/airlock/physical-tests/r5/root-machine-apply`, stops at
`reboot-required`, and never reboots automatically. New-root runtime checks,
manual host reboot, rollback and source restoration remain pending.


### Root identity mismatch; stop costly monitoring (2026-09-28)

Normal machine apply reached `reboot-required` for transaction
`1790549788826426596`, revision `da65cc9`. The selected host is
`/nix/store/nnha9msy1fknv5vyk03f3dr0kim1agx3-nixos-system-dom0-26.11.20260904.801bef6`;
running dom0 remains the previously verified `vbj6nh1…` maintenance release.
An independent pre-reboot check found usr-home's actual disk is 1,610,612,736
bytes, while `domains/usr-home/current/manifest.json` still identifies the old
1,073,741,824-byte root (hash `dbce21f3…`). This is a failed acceptance check,
despite the coordinator reporting reboot-required. No physical reboot was
requested and no live metadata was manually repaired.

Likely cause from source inspection: `ensure_shared_store` returns early when
the package-store source and volumes are unchanged, before considering staged
generation/root identity. A root-only change leaves that source unchanged.
Coordinator verification checks selected/running system paths but does not catch
this root/generation mismatch. Confirm with a focused regression test, fix the
helper and verification locally, commit, then deploy through the normal route.
Avoid forcing generation switches for unchanged running guests merely because
a new admission receipt has a different generation number.

Evidence: r5 `root-apply-status.json`, `root-pre-reboot-failure.json`, and
`root-pre-reboot-payload.sh`. Existing compaction, GC, reboot, recovery, isolation
and package-update results remain valid. Root-change boot/rollback and the
memory-only reuse trial remain open; do not repeat completed acceptance work.

Cost checkpoint: stop model-driven polling and narrated waits. Next work should
be a bounded fix plus focused tests, followed by one detached deployment/check
batch with durable exit/status files. Check only on completion, failure or user
request. Main product remains pinned to `fbe6062`; trial source `da65cc9` is in
both builders. The isolated trial must be rolled back and source restored after
verification. The main repository contains no root-size product change.


### Root-generation fix implemented locally (2026-09-28)

Component commit `ad3dfd6` fixes the confirmed early-return bug. The storage
helper now checks system, root and kernel identity before reusing an attached
generation, validates staged admission even on the no-change path, and selects
the matching generation for root changes and rollback. An equivalent newer
receipt does not force an unchanged running VM to rebind or restart.
Coordinator verification now checks the VM's store generation against the
pool's selected generation and expected system/root/kernel/closure identity;
a matching `/run/current-system` alone no longer passes this mismatch.

Focused `make test-quick` passed **163 tests, 1 skipped** in 15.31 seconds:
`test_nix_domain_storage_identity.py`, `test_machine_deploy.py`, and
`test_prepared_store.py` (with `-k "not usb_prepared_store"`). The skipped
`test_prepared_usb_runtime_closure` is not counted as a pass. Evidence:
`.airlock-logs/root-identity-fix/pytest-report.json`. Regression cases cover both
root-size directions, running-VM rejection, malformed staged identity, unstaged
root changes, equivalent-admission reuse and coordinator mismatch rejection.

This is a local fix only. No live commands, deployment or image build were run
for this fix; the physical root mismatch recorded above remains unresolved.
Next agent: deploy the fixed main source through the normal plan/apply route
first (its 1 GiB usr-home root restores the original topology), then recreate
the isolated root trial on this fix. Do not reuse old trial commits that omit
the fix, manually alter live pool metadata, or rerun unrelated passing tests.
Use detached bounded batches and inspect durable results at completion. Ask for
a manual reboot only after the fixed deployment's root/pool identity checks pass.


### Fixed deployment started (2026-09-28)

Imported committed component `ad3dfd6` into sys-dev and launched the bounded
normal plan/apply batch on dom0 as `airlock-r6-fixed-deployment.service`.
Launch check confirmed active/running, phase `planning`; no deployment pass
is claimed yet. This supersedes the local-only checkpoint above.

Durable remote evidence: `/var/log/airlock/physical-tests/r6/`, including
`phase`, command stdout/stderr/exit/elapsed files, and either `result.json`
or `failure.txt`. Local harness, source receipt and launch evidence:
`.airlock-logs/physical-20260928-r6/`. The diagnostic worker is stored at
`/var/log/airlock/physical-tests/r6-worker.py` (compressed transfer companion
`r6-worker.b64`); these are test artifacts, not production overrides.

The batch checks root/pool identities, actual guest disk sizes, saved data,
builder additions and retained host boot records after activation. It never
reboots the laptop. Inspect its final result before requesting a manual reboot;
do not reboot merely because apply was submitted. No model-driven polling.
Changed-root boot/rollback and memory-only reuse remain pending afterward.


### Fixed deployment passed pre-reboot checks (2026-09-28)

The r6 detached batch finished successfully and stopped at
`awaiting-manual-reboot` for component `ad3dfd6`. Planning took 608.85 seconds;
apply submission took 442.27 seconds. Coordinator verification reports
`reboot-required`. All seven guest root/pool bindings match the release;
usr-home now selects generation 5 with its original 1 GiB root. Actual disk
sizes and saved data passed for sys-dev, sys-ctrl and usr-home. Both builder
objects, the host derivation/output, retained boot records and selected boot
artifacts passed verification.

Evidence: remote `/var/log/airlock/physical-tests/r6/result.json`, captured in
`.airlock-logs/physical-20260928-r6/status-now.log`. Selected host:
`/nix/store/nhyj8spd495f9j28fk7wi725jr21yii3-nixos-system-dom0-26.11.20260904.801bef6`.
The worker has exited; manual physical reboot is the next step. Post-reboot
verification and remaining changed-root/rollback and memory-only trials are
still pending; this is not final completion of TODOs 05–07.


### Reboot failed during host-store assembly (2026-09-29 inspection)

The requested reboot did not pass. Offline USB log
`b40b79ad-b2da-42fa-bf73-4a47824e7a29-initrd.log` identifies selected host
`nhyj8spd495f9j28fk7wi725jr21yii3` from the r6 deployment. Root filesystem
check and mount succeeded; boot-record validation and all five EROFS base
mounts (generations 5, 1, 4, 2, 3) completed. At 15.902 seconds the read-only
OverlayFS union of those bases plus `lower-scaffold` failed with mount exit
32. `airlock-host-store.service` failed, blocking discovery of the NixOS
closure and entering initrd emergency mode. No stage-2/guest startup occurred.
The generic mount error does not establish why the kernel rejected the union.
A secondary emergency-service message reports missing `plymouth`.

Evidence copied locally to
`.airlock-logs/physical-20260929-boot-failure/initrd.log`. USB contents were
inspected without agent changes; the operator granted read ACLs to early boot
logs. Pre-reboot metadata/data checks remain valid but did not prove cold-boot
mount assembly. TODOs 05–07 remain open; investigate this mount failure before
continuing changed-root and memory-only acceptance trials.


### Mount-failure diagnostic follow-up (2026-09-29)

Offline boot records identify five distinct metadata images/closure hashes for
generations 1–5. Multiple recovery aliases share generation 1, which assembly
already deduplicates. This rules out simply listing the same metadata image
twice; it does not establish the kernel's rejection reason. Records preserved
in `.airlock-logs/physical-20260929-boot-failure/boot-records.json`.

Component `7e34747` adds a best-effort persistent failure report before mount
cleanup: original command/exception, mountinfo, supported filesystems, kernel
command line and a bounded kernel-message tail. The kernel read has a five-second
timeout. Reports use mode 0600 under `/var/log/airlock-boot/`; cleanup errors no
longer mask the original failure. Focused pytest through `make test-quick` passed
5 tests, including unavailable diagnostics and kernel-read timeout behavior
(`.airlock-logs/boot-mount-diagnostics/pytest-report.json`). No image build or USB
modification was performed. This improves diagnosis only; the boot failure is
not fixed. Obtain the failing boot's kernel messages at its emergency console,
or deploy this diagnostic initrd before repeating the boot. Existing USB logs
cannot retroactively supply the missing kernel detail.


### New baseline: isolated host generations (2026-09-29, not yet tested)

The user explicitly chose a fresh image baseline and declined preservation of
the old failing USB. Its saved logs remain historical evidence, not proof of
the original mount rejection's cause. No compatibility/migration path for the
old shared host-upper layout is provided.

Local implementation now:

- Boot validates and mounts exactly the command-line host's immutable pool
  view. It neither enumerates other boot records nor runs retirement collection.
- Writable host state lives under `host-store/generations/<id>/`: upper, work
  and Nix database. One bind at `host-store/active` keeps upper/work on the same
  mount and supplies the stable daemon URI. Database replacement uses only that
  generation's state; a damaged newer database cannot be an older boot's input.
- First publication captures the target closure and registered host-upper
  additions, including dependencies currently in the lower store, into its
  fixed view. SQLite backup preserves trust/output metadata for first boot.
  Additions after that publication remain in the originating generation's
  writable state; publication does not silently expand an existing boot view.
  Retained private uppers can therefore keep copies also published in a later
  view. Their allocation is reported separately; they are not silently discarded
  to improve a deduplication measurement. Shared immutable view payloads still
  reuse pool extents. Measure this retention cost on the new baseline.
- Apply checkpoints the current host's machine selection, then binds its new
  selection to the target before disrupting guests. Boot restores its own
  complete transaction and controller, or the initial image's staged guest
  generation map. A selected incomplete transaction still fails closed. These
  bindings use the normal direct-pool candidates outside `/nix/store`.
- Retained Xen/kernel/initrd files live in `EFI/airlock`, with checksum-bound
  `airlock-xen-*` entries. Menu regeneration recognizes saved role/system
  comments without opening old store views, and leaves sealed entries unchanged.
  Retirement removes only files not shared by another retained descriptor.
- Boot installation still uses upstream `switch-to-configuration boot`, in a
  private mount namespace exposing only available profile links. Real profiles
  and their history are unchanged; unavailable old views do not become installer
  dependencies. Declarative boot hooks replace the old compatibility drop-in code.
- Mount failures record exact errno, original exception, mountinfo and a bounded
  kernel-message tail in `/var/log/airlock-boot/*-host-store-failure.json`, before
  cleanup. Earlier validation failures are also recorded; cleanup errors do not
  mask the original failure. Independent recovery remains available.

Added/updated pytest coverage exercises selected-only assembly with a corrupt
newer boot record, missing/damaged view, missing pin and corrupt database;
returning generation's saved upper; publication reference closure; SQLite WAL
metadata snapshots; machine-binding restore and interrupted-transaction rejection;
retained boot artifacts/menu handling, profile filtering and mount errno/cleanup.
Mount operations in the assembly unit fixtures are emulated: these are not a
substitute for physical boot evidence. **Tests were not run during implementation.** Static Python
parsing and diff whitespace inspection were performed only. No image was built,
no live session was contacted and the USB was not modified. Component commit
`c2bb176` contains the implementation and new regression tests; the parent
repository pins it for the next baseline build.

Next validation on the fresh baseline:

1. User runs `make test-quick`, then builds the image after tests pass.
2. Boot image A and verify one host base, matching guest selections and writable
   Nix behavior. Record host/guest data and build-object sentinels.
3. Apply release B normally; reboot and verify host, guests, Nix registrations
   and sentinels. Check retained A and recovery EFI entries still validate.
4. Boot A from its retained entry; verify A's store, machine selection and guest
   generation bindings. Return to B and check its own additions remain available.
5. In a controlled fault trial, make only B's view metadata unavailable (preserve
   it for restoration; do not alter shared payload segments), then boot A and
   independent recovery. Restore B's metadata before resuming normal tests.
6. Resume the still-open TODO 06 changed-root boot/rollback and memory-only reuse
   trials, and recheck affected retention/GC behavior. Historical passes for
   unchanged behavior remain evidence, but do not mark 05–07 complete until the
   new boot/rollback behavior and remaining acceptance criteria pass.

### First baseline quick-test run (2026-09-29)

The user ran `make test-quick`: 663 passed, 24 skipped, eight setup errors and
one failure. All eight errors came from the new boot fixture mocking admission
without creating its registration file. Component `c2a6d82` supplies that file
while retaining real publication and validation. The parent flake check failed
before evaluation because the local sibling submodule source lacked `c2bb176`.
The component commits were fetched into that source under
`refs/airlock/parent-build-source`, without changing its checkout. Both causes
have been addressed; the user must rerun tests. No tests or image builds were
run while making these fixes, and no live/USB changes were made.

### Second baseline quick-test run (2026-09-29)

The user reran `make test-quick`: 665 passed, 24 skipped and seven failed.
The parent flake check now passed. Six boot tests exposed a production bug:
first boot parsed the absent previous database as an empty closure, which the
strict registration validator correctly rejects. Component `a6f2e71` represents
an absent database as no prior records and uses the selected registration
directly when there are no saved upper objects. Existing database dumps remain
strictly validated. Added coverage checks both an existing database with no
saved upper objects and rejection of an empty dump from an existing database.

The separate terminal test timed out awaiting client exit after its interrupt
sequence. Component `e431c44` replaces the race-prone command-echo synchronization
with foreground-process output and a fresh prompt after Ctrl-C; it also drains
the final client output before waiting for exit. Confirmation awaits the user's
rerun. Python syntax parsing and diff whitespace checks passed; no tests,
image builds or live/USB changes were run during this repair. The local sibling
Git source contains both commits for parent flake evaluation.

### Fresh-image menu duplication (2026-09-29)

The user's first-boot photo showed both generated and retained normal/recovery
entries. Sealing had copied the boot entries without removing their native menu
copies. Duplicate recovery titles/versions caused systemd-boot to display long
identifiers. The local repair uses short role titles and `Generation N` version
fields, verifies retained boot files before deleting matching native menu
entries, and redirects loader defaults before removal. Menu regeneration prefers
the retained entry for the same selected system, preserving rollback selection
and sealed entry bytes. Distinct older normal generations remain selectable.
Added regression coverage for duplicate removal, concise labels, regeneration,
and preserved rollback defaults. Static Python parsing and whitespace checks
passed; tests and image rebuilding remain with the user. The running USB was
not changed.

### R7 fresh baseline and detached plan (2026-09-29)

SSH through `sys-dev` succeeded after the user approved the replacement SSH
host key and authorized Tailscale SSH. Running host:
`/nix/store/sazv1kzrs8c8wa5xqafyjqcwmcsbndg3-nixos-system-dom0-efi-raw-26.11.20260904.801bef6`.
Both builder repositories initially reported `e431c44`. Only
`/var/lib/airlock/host-store/bases/1` is mounted as EROFS. Sys-ctrl has 8192 MiB
and uses sys-firewall; its builder base is generation 2. Required guests and
machine autostart are healthy; usr-browser remains stopped as configured.
The 57-GiB root has 38 GiB available. The install receipt reports zero ordinary
host store objects remaining after seeding and 2,943,639,552 recovery bytes.
These are baseline observations, not completed update/rollback acceptance.

Test-owned persistence files were created under
`~/.local/share/airlock/physical-tests/r7/` in sys-dev, sys-ctrl and usr-home;
dom0 uses `/var/lib/airlock/physical-tests/r7/`. Host and builder files were
added to their respective Nix stores and rooted for later preservation checks.
An initial diagnostic incorrectly tried to read a builder object through the
guest runtime store; reading its actual `/rw/airlock-builder/merged/nix/store`
path succeeded. No runtime product code was modified. Remove these test roots
and sentinels after final verification.

The standard source importer installed committed `41ee7dd` into sys-dev.
Detached user unit `airlock-r7-plan` runs the typed plan route through sys-ctrl,
with a one-hour command timeout and durable stdout/stderr, phase and JSON result
under `~/.local/share/airlock/physical-tests/r7/plan/` in sys-dev. The test driver
only plans and checks revision/no retire-or-restore actions; it cannot apply or
reboot. No active polling of the long command. Review the result before apply.
Local baseline/sentinel receipts and the driver are in
`.airlock-logs/physical-20260929-r7/`. No reboot is currently needed.

### R7 plan relaunched independently of SSH (2026-09-29)

The first status check found that sys-dev's user service manager stopped
`airlock-r7-plan` at 12:55:28 UTC, ten seconds after starting it. `Linger=no`;
the user manager exited after SSH logout, leaving `phase=planning` without a
result. The command had reached controller source import/input fetching;
no apply was requested. This was a test-launcher lifetime error.

Restarted the typed plan under dom0's **system** unit `airlock-r7-plan`, using
`qvm-run --pass-io --user user sys-dev 'airlock-sys-dev-machine-deploy plan'`.
The launch returned active/running (PID 10174). It is detached from SSH and has
a 3700-second runtime limit. Authoritative logs are now in dom0 at
`/var/log/airlock/physical-tests/r7/plan/{phase,stdout,stderr,exit,elapsed}`;
`exit`/`elapsed` are written on command completion. The original sys-dev logs
are retained as evidence of the stopped attempt. No linger settings or product
code were changed. Review the completed plan before applying; no reboot yet.

### R7 controller request survived launcher loss (2026-09-29)

The next status check corrected the previous diagnosis: the dom0 retry exited
in one second with `Resource temporarily unavailable`, because the original
sys-ctrl request still held the controller lock. Controller PID 1068 remained
active after the SSH-side launcher stopped. At inspection it had run for about
15 minutes, finished the build, and was admitting usr-browser through
`airlock-pool-send`; `admissions.json` existed for `41ee7dd`. No request was
applied. The earlier claim that the whole plan had stopped was incorrect: only
the requesting launcher stopped. The retry made no concurrent deployment.

Leave the existing controller operation running. Its authoritative durable
result is sys-ctrl's
`~/.local/share/airlock/controller/candidates/41ee7dd63ac57406d8dc3dd05a76f7ec80149062/plan.json`,
written before the response is printed to the disconnected caller. Build timing
is in `~/.local/share/airlock/machine-deploy/<revision>/build.json`; admission
receipts are saved incrementally in the candidate's `admissions.json`.
The failed retry's dom0 logs do not describe the original request's outcome.
Evidence: r7 `status-plan.results.json` and `controller-status.results.json`.
No repeated polling, further retries, activation or reboot were performed.

### R7 plan passed; typed apply submission started (2026-09-29)

The original controller saved a successful `plan.json` before exiting.
Revision `41ee7dd`, candidate
`/var/lib/airlock/machine/candidates/d3fa8777c911b072994fef29a7e44ff71c71034619dffc97da4a8089e700590d`.
Build: 510.52 seconds. Subsequent preparation: 648.44 seconds, including
196.59 seconds binding validation and 7.01 seconds host transfer/import.
Guest sender inventory scanning was a major cost (sys-gui alone 246.79 seconds).
Host transfer: 36 paths, 126,918,080 NAR bytes, 117,092,599 compressed bytes.
Guest packages: sys-ctrl 12 objects / 3,209,216 payload bytes, sys-dev 11 objects /
1,907,200 bytes; all other guests reused every package. Every guest reused its
root image with zero root payload bytes. This proves first-plan root reuse for
this baseline, not yet changed-root activation/rollback or repeat-update cost.

Pre-apply review confirmed exact revision/lock provenance, all staged guest
generations 2, host pool generation 2, no retire/restore actions, and the current
host still on generation 1. A fresh baseline has `host/running.json`, with no
`host/selected.json` until selection; the initial read-only probe was corrected
accordingly. Existing generation-1 state and the host persistence GC root exist.

Started dom0 system unit `airlock-r7-apply-submit` (PID 13054), invoking the
normal typed `airlock-sys-dev-machine-deploy apply` route via sys-dev. It will
revalidate/reuse the prepared source and admissions, then submit the detached
dom0 activation worker. Unit lifetime is bounded to 3700 seconds; logs are at
`/var/log/airlock/physical-tests/r7/apply/`. Request submission is not successful
activation; inspect machine status and saved sentinels before asking for reboot.
No reboot, active polling, or direct coordinator bypass was performed. Local
receipts: r7 `plan.json`, `pre-apply.results.json`, `start-apply.results.json`.

### R7 pre-reboot verification passed (2026-09-29)

Transaction `1790688503537734589` reached `reboot-required`; typed coordinator
verification passed with running guests matching the selected systems. Apply
submission completed successfully in 444 seconds. Repeat preparation reused all
guest packages and roots (zero payload bytes), and zero host paths were imported
(23 compressed framing bytes). Cached build took 45.84 seconds and subsequent
preparation 396.74 seconds; reuse still has substantial inventory/validation cost.

All three guest persistence hashes match the baseline; both builder store
additions remain valid and readable. The dom0 store addition is also valid.
Generation 1's saved machine binding contains the baseline guest generations 1;
generation 2's binding contains the selected transaction and guest generations 2.
Host generation 1, host generation 2, and independent recovery have verified
boot artifact descriptors. The loader default points to generation 2's retained
entry. The menu now contains three independent choices: Airlock, Airlock
(generation 1), and recovery. Old sealed labels remain unchanged as intended.
An initial diagnostic incorrectly demanded an installed EFI descriptor for every
published image system alias; verification was corrected to cover every actual
installed descriptor plus explicit running/selected-host requirements.

The only failed systemd unit was the known test retry rejected by the controller
lock; its failed state was cleared after preserving its logs. No failed units
remain. Evidence: r7 `pre-reboot.results.json` and
`boot-artifact-check.results.json`. Manual reboot requested; no automated reboot
or subsequent test work until the user confirms it. After reboot, verify exactly
one generation-2 base, matching host/guest selection and saved state before
starting rollback tests.

### R7 generation-2 boot passed; menu rollback next (2026-09-29)

After the user's reboot, the running host matches selected `qs4fs63...`.
Coordinator verify advanced transaction `1790688503537734589` to `complete`.
Only `/var/lib/airlock/host-store/bases/2` is mounted, and `/nix/var/nix/db`
binds generation 2's private database. Required guests are running, usr-browser
remains intentionally stopped, and dom0 has no failed units. All guest
persistence hashes, both builder additions and the original dom0 store object
survived. Generation-1 and generation-2 machine bindings remain distinct.
Evidence: r7 `post-reboot.results.json`.

Added and rooted `/nix/store/7nzs8jw48y1pwca1vv1s7qfp9r94smz8-generation2-marker`,
containing `r7:only-generation-2`. Verified it is in generation 2's upper and
absent from generation 1's upper. Its root is
`/var/lib/airlock/physical-tests/r7/generation2-root`; remove after acceptance.
Evidence: `generation2-sentinel.results.json`. This will check persistence on
return to generation 2 and isolation while booted into generation 1.

Requested a manual reboot selecting **Airlock (generation 1)** from the boot
menu, without first applying a rollback transaction. This directly exercises
restoration from the selected boot generation's own machine binding. Verify
baseline host/view/database and guest generation 1, saved data, and absence of
the generation-2-only object from the active store, then return to generation 2.
No automated reboot or further mutation while waiting for the user.

### R7 direct menu rollback to generation 1 passed (2026-09-29)

The user selected generation 1 in the boot menu. Running host is the original
`sazv1k...` baseline; only `bases/1` is mounted and its private database backs
`/nix/var/nix/db`. All seven guest selections reverted to generation 1, and
all six running guests' actual systems match their generation-1 manifests.
Usr-browser remains stopped. Dom0 has no failed units. The coordinator reports
`image-baseline` with no selection, as expected for the original image binding;
its `host_matches_selection=false` reflects the absent transaction selection,
not a failed rollback. No rollback apply transaction was needed.

All three private-data hashes match; both builder additions and the original
host addition remain valid and readable. The generation-2-only marker is absent
from the active store but still exists in generation 2's private upper.
Evidence: r7 `rollback-status.results.json` and `rollback-guests.results.json`.
Requested manual reboot selecting **Airlock** (generation 2) to verify the
return restores its selection, database, guests and saved addition. No product
changes, automated reboot or destructive fault injection performed.

### R7 return to generation 2 passed; missing-view fault staged (2026-09-29)

Generation 2 booted again. Coordinator verify reports `complete`, its own
base/database are mounted, all six running guest systems match generation 2,
and private-data hashes plus original host/builder additions match. The
additional generation-2-only object is valid and readable again. This completes
the normal 2 → 1 → 2 sequence with the new isolated state layout. Evidence:
`return-generation2.results.json`.

A deliberate, reversible diagnostic mutation is now active: moved only
`/var/lib/airlock/closure-store/views/by-closure/42d6b0863f4b11e74e105d4774c62be6b056c52cf3d5ceddda1bd48a109ab14a/store.erofs`
to `/var/lib/airlock/physical-tests/r7/missing-generation2-view/store.erofs`.
Verified its checksum first and checked no guest generation uses that metadata
image. Shared payload segments were untouched. `restore.json` in the same test
directory records source/destination and SHA256
`f427fa4489d18a9aa511307417f5c259dfcd51e751d3b2641d76171106e5c2aa`.
The running mounted view remains usable until shutdown. Generation 2 must not
be booted until this file is restored and its hash checked.

Used the existing `airlock-host-store select` command to make generation 1 the
boot default, after validating its independent artifacts. Requested manual
reboot into **Airlock (generation 1)**. Verify that it boots while generation 2's
view remains absent; restore the moved file before any return to generation 2
or maintenance. Evidence: `missing-view-setup.results.json` and
`select-generation1-for-fault.results.json`. This diagnostic is not a product
change and must be undone before final acceptance. No automated reboot.

### R7 user-reported input failure before fault-test reboot (2026-09-29)

The user reports keyboard input and mouse clicks no longer work on the current
generation-2 desktop. This is unresolved; earlier "healthy" reports described
service/store checks, not interactive input verification. SSH and qrexec remain
responsive. Read-only capture found physical input devices in dom0, active input
discovery/event services and input-proxy senders, plus input devices in sys-gui;
these facts do not establish successful delivery to the desktop. No input
services were restarted and no input configuration was changed. Cause and
relationship to generation transitions remain unproven. Evidence: r7
`input-regression.results.json` and `input-focused.results.json`.

The reversible missing-generation-2-view fault remains staged exactly as above.
Generation 1 is the verified boot default. The user requested live debugging
before reboot; retain this issue in final acceptance reporting.
Do not claim full interactive health merely from an empty failed-unit list.


### R7 input failure isolated to controller initialization (2026-09-29)

On boot `ef0091e961fa4878991181f49b0185a4`, dom0 logged atkbd failures to
deactivate and enable the keyboard, IRQ 1 remained at zero, and i8042 had no
AUX/TrackPoint device. Both preceding boots had working KBD/AUX initialization
and detected the Elan TrackPoint. In sys-gui the GDM greeter was active, GNOME
answered D-Bus, and GNOME had opened the forwarded input devices. A bounded
120-second event-category counter received touchpad events but no keyboard
events; it recorded no typed text. Touchpad button versus contact events were
not distinguished, so this does not establish working clicks.

At 15:14:10 UTC, performed a temporary diagnostic unbind/rebind of the dom0
`i8042` platform driver using a bounded transient systemd unit. KBD and AUX
initialized without the earlier errors; the keyboard and Elan TrackPoint
reappeared, and IRQ counters became nonzero. Existing Qubes discovery handled
the remove/add events and automatically forwarded both into sys-gui. No Qubes
or GUI service restart, configuration change, or reboot was needed. The first
launcher failed before touching the controller because its PATH lacked `date`;
the retry supplied the existing system PATH and succeeded. Its failed-unit
marker was cleared after preserving logs; dom0 reports no failed units.

This establishes a recoverable controller-initialization failure, not its
underlying cause or a permanent boot fix. The user subsequently confirmed that
keyboard input and mouse clicks work again; successful login was not separately
reported.
The transient rebind changed only current device state and left the driver bound;
no persistent override or diagnostic daemon remains. The independent missing-view
fault above is still staged and must be restored before booting generation 2.
Evidence: r7 `keyboard-boots`, `keyboard-rebind-observed`,
`keyboard-rebind-forwarding`, `input-capture-result` and their result JSON files;
dom0 `/var/log/airlock/physical-tests/r7/i8042-rebind.log`.


Before resuming the missing-view trial, rechecked that generation 2 is still
running, generation 1 is the EFI default, the deliberately moved file is absent
from its original path, and the saved copy matches its recorded SHA256. Keyboard
IRQ counts increased following recovery; dom0 has no failed units. Evidence:
r7 `input-recovered-pre-reboot.results.json`. Requested manual reboot into
**Airlock (generation 1)**; do not select generation 2 until its view is restored.


### R7 automatic input recovery implementation and deployment (2026-09-29)

At the user's request, machine source `cd47d32` adds a boot guard before physical
GPU startup and input connection. Only an explicit kernel atkbd failure to enable
the i8042 keyboard triggers one bounded unbind/bind attempt. A durable per-boot
attempt marker prevents repeats after failure, timeout or interruption. The guard
requires keyboard driver binding and no new enable failure after the reset.
Reports explicitly leave physical input unverified. Hardware absence or inactivity
alone does not trigger resets. Failure blocks GPU startup and invokes the existing
boot failure reporting/console with the saved input report path.

Focused checks: 13 passed (12 behavioral recovery checks plus Nix service wiring);
the final formatted behavioral checks passed again (12), and scoped Ruff lint
passed. No image build or full quick suite was run. The component commit is
available in the configured sibling source for reproducible parent builds.

Before deployment, restored the deliberately moved generation-2 metadata image
and verified its original SHA256, reselected generation 2, and obtained typed
`verify: complete`. Evidence: `input-fix-restore-view.results.json`. There is no
active missing-view fault now. The fault trial is outstanding, not passed.
Next: deploy the committed change through the normal controller route, then
request a manual reboot into the new selected generation. Do not claim the live
boot guard is installed until that deployment has completed.


### R7 input recovery deployed for the next boot (2026-09-29)

Normal `make machine-plan DEPLOY_TARGET=sys-dev` completed. The source build
took 67.0 seconds and preparation took 630.8 seconds; only 10 missing host paths
were transferred (137,018 compressed bytes). The plan had no changed, affected,
staged, stopped, retired, or restored guests. Its `start` list names the six
already-running required guests; coordinator startup skips running VMs. An
initial conservative automation gate stopped on that nonempty list without
applying anything; after checking the planner/coordinator behavior, the plan
was reviewed and normal `make machine-apply DEPLOY_TARGET=sys-dev` launched.

Apply preparation reused all guest objects/root artifacts and all host objects
(zero host paths, 23 framing bytes). Its cached build took 45.9 seconds and
preparation 395.6 seconds. Detached dom0 worker
`airlock-machine-deploy-dcd31e3b7b51` completed successfully in 114 seconds.
Transaction `1790698109572546418` is `reboot-required`; no guests were changed or
stopped, their systems still match selection, and dom0 has no failed units.

Selected host/pool generation 3:
`/nix/store/2zdblan8p76952afjy5wg8bi520n81i8-nixos-system-dom0-26.11.20260904.801bef6`.
The EFI default `airlock-xen-1eb73a3b2e6f586ed22a97451b6aa37a8a5c829fb85496c39b87130f385a52a2.conf`
is titled **Airlock**, version **Generation 3**, and names that exact system.
The generated input guard is enabled in multi-user, required before GUI startup,
and wired to the existing failure handler. Its installed script SHA256
`6e9b222966b2d04e633455e199d8da7926755da8e21d5a23b129e748d67a8310`
matches the committed local source. No ad-hoc persistent override was installed.

Evidence: r7 `input-guard-plan/receipt.json`, `input-guard-apply/receipt.json`,
`input-guard-built-units.results.json`, `input-guard-artifact-hash.results.json`,
`input-guard-worker-result.results.json`, and `input-guard-selected-boot.results.json`.
Requested manual reboot into default **Airlock**. After reboot, verify the
running host is generation 3, inspect the per-boot `input-controller.json`,
confirm usable keyboard/clicks, and run typed machine verification. A healthy
boot can establish the no-error path; it does not by itself exercise automatic
recovery from a new hardware initialization failure. Physical acceptance and the
remaining 05–07 trials are not complete.


### R7 generation 3 input success and host-addition carry defect (2026-09-29)

Boot `98b666fc-3a52-4f85-97d4-3e57ae62c681` runs the intended `2zdblan...` host,
with only `bases/3` mounted and generation 3's private Nix database. Kernel KBD
and AUX initialization succeeded. The input guard completed at 16:16:16 UTC
before forwarding started at 16:16:32; its report records no failure and no
attempt. The user confirmed keyboard, mouse clicks and login work normally.
Boot supervision and typed machine verification both report complete. This
validates the normal boot path, not automatic recovery from a fresh fault.

Private hashes for sys-dev, sys-ctrl and usr-home and both builder additions
match. The generation-2 marker remains valid in dom0, but the original
`/nix/store/53saabr3jjnr5snhhdj2d6sgifrk7pmk-persistence` is absent from the active
store. It was added in generation 1's upper, published into generation 2's
immutable view, then omitted when publication of generation 3 enumerated only
current upper additions. Generation 2's retained registration still contains it;
generation 3's does not. This is a real persistence defect, not a passed check.

The local publisher now includes Nix's live rooted path set as well as current
upper additions and the target system, with reference completeness enforced.
`--gc --print-live` queries live paths; it does not collect store objects.
The actual overlay query found the generation-2 marker in the lower-backed live
set. Regression tests cover an addition moving from upper to lower between
publications and rejection of a live path absent from registration: the full
host-generation module passed, 21 tests.

Temporary test-fixture repair: re-added the retained original
`/var/lib/airlock/physical-tests/r7/persistence`, asserted its resulting store
path is exactly the existing `store-root` target, re-registered that GC root,
and verified `r7:dom0`. No payload segments, retained generation records, or
other user data were edited. This restores the sentinel for testing the source
fix; it is not evidence that the defective publisher works.

Evidence: `input-guard-boot3.results.json`,
`input-guard-boot3-persistence.results.json`,
`host-addition-carry-diagnosis.results.json`,
`host-addition-live-query.results.json`,
`host-addition-restore-fixture.results.json`, and
`host-addition-regression-tests.log`. Keep the missing-view trial paused until
the carry fix is deployed and both host markers survive the next boot.


While the carry-fix plan runs, bounded live rejection probes passed on generation
3: sys-dev's direct MachinePlan and usr-home's MachineControl requests were
refused (exit 126); authorized sys-ctrl requests with an unsupported schema,
truncated machine header or invalid release path returned explicit errors.
A truncated guest apply request also exited 1. Two MachineApply requests with
a valid-shaped inventory but no Nix export payload both returned `Nix closure
import failed`; the bogus store path remained invalid and transaction
`1790698109572546418` remained complete. These checks used the real qrexec routes,
with no policy edits or synthetic success receipts. No garbage collection was
run. This covers rejection and empty interrupted-stream retry, not arbitrary
mid-object interruption or all temporary-data cleanup cases.
Evidence: `controller-rejections.results.json` and
`controller-incomplete-artifact.results.json`.


### Deployment preparation performance (2026-09-29)

Measured carry-fix plan: build 210.73 s; preparation 675.51 s. Of preparation,
per-guest inventory creation totalled 419 s (sys-gui alone 249 s, all 1,513
objects reused), binding/publication 226.22 s, and host transfer/import 4.95 s.
All immutable guest root artifacts were reused. Apply repeated preparation;
its host binding was already cached (4.57 s), but guest inventories were repeated.

Source `649469b` implements:

- Negotiate package availability using Nix path metadata before hashing files.
  Dom0 supplies the recorded tree identity of existing admitted objects; only
  missing payloads are read, hashed, transferred and content-validated. Pool
  admission locking still protects reuse from GC through publication.
- Query each unique package's Nix metadata once across all machine guests.
- Probe for an exact retained guest generation before preparing its inventory.
  Match system, closure source, root source and kernel; check authority,
  retention, view integrity and backing ranges. Keep original guest provenance
  in the new machine binding. Guest writable volumes are never shared.
- Apply the exact root-owned prepared candidate using a checksum-bound token.
  Revalidate its host/guest views, replan, and compare machine state; repeat the
  state check under the worker's deployment lock before mutation. Changed
  selection, transaction, running VMs, booted host or live Nix roots require a
  new plan. No guest admission or transport import is repeated on this path.
- Reuse a pinned build of the identical committed source and lock file.
- Report separate host registration, pool publication, database seeding,
  controller-base preparation, guest binding and prepared-candidate check times.

A retained generation without the optional reuse proof uses normal admission;
this prepares a new proven view using existing package objects, without editing
old generation records. The logical view checksum permits compaction relocation
but rejects altered object layout or unrecorded physical extents. Integrity
scrubbing of already admitted package bytes remains a separate operation.

Full `make test-quick` passed: 714 passed, 24 skipped, 2 warnings in 215.11 s.
The first run had one stale USB-seed protocol fixture (713 other passes); its
simulated response now supplies the pool-owned reuse identities. Live deployment
and performance measurements are pending. This is not
a completion claim for 05–07. Evidence: `.airlock-logs/deployment-performance/`
and `.airlock-logs/deployment-perf-*.log`.


### R7 performance update staged, awaiting boot (2026-09-29)

Normal `make machine-plan` / `make machine-apply DEPLOY_TARGET=sys-dev` deployed
`649469b`. Reviewed plan changed/stopped only sys-ctrl and sys-dev, reused every
guest root artifact (zero root payload bytes), and required a host reboot.
Both host sentinel paths occur in the new generation 5 registration. The prior
carry-fix generation 4 was selected but not booted; generation 5 supersedes it
for the next boot and contains that fix.

Plan used the installed previous sender/receiver: build 245.53 s, preparation
641.40 s, host transfer 39 paths / 117,140,670 compressed bytes. Apply used that
same previous preparation path: build 47.61 s, preparation 422.94 s, zero host
paths transferred. These are installation timings, not measurements of the
new optimization. The new workflow can be exercised once host and guest
components have booted together.

Transaction `1790704764992386034`, candidate
`124518d4c9322e51289c872519e4a2acc22831b7fdcba17aed14a6295a4f5c91`, finished
with `reboot-required`. Unit `airlock-machine-deploy-4e0a6be5956b` is inactive
with `Result=success`, `ExecMainStatus=0`. Updated sys-ctrl (`z62gzn...`) and
sys-dev (`041514...`) match selection; other running guests match too. Dom0
has no failed units. Its profile and default EFI entry agree on generation 5,
`8vlimj...`; the running host remains generation 3 (`2zdblan...`).

Evidence: `.airlock-logs/deployment-performance/{plan,apply}/receipt.json`,
`test-quick.log`, and r7 `performance-plan-review.results.json`,
`performance-roots.results.json`, `performance-final-selection.results.json`.
The first administrative status connection timed out during the expected guest
restart; typed status then reached `verifying`, followed by the authoritative
terminal result above. No ad-hoc live code patch or image rebuild was used.

Manual reboot into default **Airlock** is now required. Verify both host
sentinels, selected generation and input readiness after boot, then measure
unchanged/changed planning and prepared apply. TODOs 05–07 remain open.


### Generation 5 and optimized unchanged workflow verified (2026-09-29)

Boot `f78fc248-b3fb-4ab5-93b0-828220637822` runs `8vlimj...`, with only host
`bases/5` mounted and its own generation-5 Nix database. Typed verify completed
transaction `1790704764992386034`. Both rooted host objects are valid with
contents `r7:dom0` and `r7:only-generation-2`. The three private-data SHA256s
match previous evidence; sys-dev/sys-ctrl builder additions remain valid and
readable. Boot supervision is complete, there are no failed dom0 units, and
the input guard reported no failure/no attempt before forwarding. The user
confirmed keyboard, clicks and login work. Evidence: r7
`performance-boot5.results.json`.

Optimized timings (same source `649469b`):

| Operation | Local command wall time | Controller preparation/checks | Result |
| --- | ---: | ---: | --- |
| First unchanged plan after boot | 36.55 s | 30.40 s | Existing packages/roots reused; optional guest view proofs prepared |
| Repeat unchanged plan | 18.20 s | 15.54 s | All seven retained guest generations reused |
| Prepared apply submission | 8.65 s | 3.97 s | Exact prepared candidate reused; no inventory or payload transfer |

Cached build lookups took 0.03 s and 0.005 s. Both plans transferred zero
package/root payloads and required no VM stops or host reboot. The prepared
apply used candidate `d961bd05fa05d130a4e7ca6dacaf95b0b96b4134216c56d29669847e45e1c50b`;
transaction `1790705987415026248` reached complete and typed verification matched
host and guests. Submission timing excludes the detached activation worker.
Compared with previous repeated preparation (422.94 s), repeat preparation is
about 27 times faster on this machine. This is an unchanged-update comparison,
not a claim that newly changed releases build or publish at the same speed.

Evidence: `deployment-performance/boot5-plan/receipt.json`,
`boot5-repeat-plan/receipt.json`, `boot5-prepared-apply/receipt.json`,
`boot5-prepared-verify.json`, and each command's `elapsed` file.

Remaining work continues with isolated branch `acceptance/r7-root-change`,
commit `b2d511a`, based exactly on `649469b`; only usr-home's root image size is
changed to 1536 MiB. Normal source push and typed machine-plan run detached under
`deployment-performance/root-trial-plan/`. Review exact topology and root bytes
before apply; check actual selected root plus private data after activation,
then manual boot and rollback. Product main remains pinned to the baseline.


### R7 changed-root activation passed; generation 6 reboot pending (2026-09-29)

Isolated committed trial `b2d511a` changes only usr-home root size from 1024 to
1536 MiB. Preflight compared the selected and candidate topologies: the sole
difference was `/domains/usr-home/backend/root/source`. All seven guest package
closures needed zero payload; six unchanged guest generations were reused.
The new sparse root transferred 2,949,690 bytes for 1,610,612,736 logical bytes.
Four host paths transferred 113,035 compressed bytes.

Changed-release build took 71.77 s and preparation 261.37 s. Binding accounted
for 187.48 s, including 183.61 s host publication: controller-base publication
119.06 s and host-pool publication 63.24 s. These measurements identify remaining
costs for changed releases; unchanged-plan timings do not predict this path.

Typed prepared apply reused the reviewed candidate and submitted in 7.00 s
(4.61 s prepared checks), without repeating build, inventory or transfer.
Detached worker `airlock-machine-deploy-adacb1479938` exited successfully.
Transaction `1790707116774052820` is `reboot-required`; only usr-home was stopped
and staged. Its actual root volume and selected pool generation 7 agree on
1,610,612,736 bytes and the new root source. This passes the pre-reboot check
that exposed the earlier root-selection mismatch.

usr-home restarted (boot ID `0da1a82d-2608-4640-83a3-defe8e892c9f`); private
volume `appvms/usr-home/private` remains 17,179,869,184 bytes and sentinel SHA256
`c82d507df8430088f8b91dba130639b595f586074f0a3f92ba77ddfb2c71a397` matches.
Other domain topologies are unchanged, running guest systems match selection,
and dom0 reports zero failed units. The system profile, selected host record
and default EFI entry agree on generation 6, host `/nix/store/v980gii12s3vikqxnkxpm0c123r66ldz-nixos-system-dom0-26.11.20260904.801bef6`.
Both rooted host markers were checked in its registration before apply.

Next: manual reboot into **Airlock (Generation 6)**, verify host/guest state and
persistence, then rollback the trial to candidate `d961bd05fa05d130a4e7ca6dacaf95b0b96b4134216c56d29669847e45e1c50b`
and restore source `649469b` in both builders. Changed-root boot/rollback is
not yet passed. No USB image was built. Evidence:
`.airlock-logs/deployment-performance/root-trial-{plan,apply}/` and
`.airlock-logs/physical-20260929-r7/root-trial-{preflight,applied}.results.json`.


### R7 changed-root generation 6 boot passed (2026-09-29)

Boot `7339548e-52f1-4f1b-be87-0e4861537fc6` runs the selected `v980gii...`
host, generation-6 view and generation-6 Nix database. Typed verification
completed transaction `1790707116774052820`; running guests match selection.
usr-home's actual root remains 1,610,612,736 bytes and agrees with pool
generation 7. Its 16 GiB private volume and sentinel survived, as did both
builder additions, all three guest sentinels and both rooted host markers.
Boot supervision reports complete/desktop-ready, input guard reports no
failure and no reset attempted, and dom0 has zero failed units. Physical
keyboard/click operation was not separately confirmed on this boot.

The first typed verify returned exit 1 without stdout/stderr; a subsequent
verify succeeded. No deployment state or files were repaired between those
calls. The reason for the transient failure is not established.

Typed rollback was submitted as `airlock-machine-deploy-230e0fd47669`,
transaction `1790712446217355385`, targeting retained baseline candidate
`d961bd05fa05d130a4e7ca6dacaf95b0b96b4134216c56d29669847e45e1c50b`.
Rollback completion and its next boot are still pending. Evidence:
`.airlock-logs/physical-20260929-r7/root-trial-boot6.results.json`,
`.airlock-logs/deployment-performance/root-trial-apply/boot6-verify.json`,
and `.airlock-logs/deployment-performance/root-trial-rollback/`.


### R7 root rollback passed pre-reboot verification (2026-09-29)

Worker `airlock-machine-deploy-230e0fd47669` exited successfully. Rollback
transaction `1790712446217355385` is `reboot-required`. usr-home's actual root
is 1,073,741,824 bytes, agreeing with selected pool generation 6 and baseline
root hash `dbce21f3a91d475190d83ba25fb79e61f0b454f4984ad27b3e433dfc0fd534f7`.
Its private volume identity/size and sentinel hash are unchanged. Guest boot
ID `9d27a186-ffc3-4c8b-8ccb-c406e0143129` confirms the restart. Other domain
topologies are unchanged, running guest systems match selection, and dom0
has zero failed units. The selected host record, system profile and default
EFI entry agree on retained host generation 5 (`8vlimj...`).

The clean committed product source `649469b` was restored through the normal
source importer in sys-dev and checked out in sys-ctrl after checking its
working tree was clean. No deployed files or package data were patched.
The earlier inspection during `host-boot-selection` was a premature check;
the subsequent completed worker passed all assertions without repair.

Next: manual reboot into **Airlock (Generation 5)** and verify the return
boot and persistence. Root-change activation and forward boot are passed;
return boot is still pending. sys-ctrl was running in these trials, so the
controller-outage part of the completion criterion remains separate.
Evidence: `.airlock-logs/physical-20260929-r7/root-trial-rollback-final.results.json`
and `.airlock-logs/deployment-performance/root-trial-rollback/`.


### R7 remaining-check consolidation (2026-09-29)

The complete changed-root cycle passed: generation 5 → generation 6 →
generation 5, with a genuine 1024 → 1536 → 1024 MiB guest root change. Final
boot `0840d0fe-966a-494c-af20-fa167cd47474` uses only host base 5 and its database;
typed verify completed rollback `1790712446217355385`. All guest, builder and
host markers survived. No failed dom0 units; input guard attempted no reset.

Committed isolated plan `d827ad2` changes only `/domains/usr-home/prefs/memory`
from 2048 to 2304 MiB. It was **not applied**: runtime memory is still 2048,
selection is still baseline `d961bd...`. All seven guest generations reused;
zero guest package or root payload bytes. Build took 64.66 s, preparation
234.42 s; four host paths transferred 112,910 compressed bytes. Host publication
accounted for 205.43 s (111.18 s controller base, 92.98 s host pool). These costs
remain for changed whole-machine topology even when guest roots are unchanged.

A host sentinel already in sys-ctrl's approved builder lower store was realized
successfully with `airlock-config-nix build --offline --no-link`, remained absent
from its upper layer, and matched the lower payload. An unavailable path failed
with no substituter. This proves reuse and missing-path rejection; fetching a
new build dependency remains separate.

The previously unimplemented privileged storage check is now a real pytest
worker. Component `3a4e3d2` contains test-only changes; no runtime replacement or
image build is needed. Its dom0 run passed with a disposable pool/private mount
namespace, sharing a payload extent between two different closure views,
checking exact membership and write rejection, detaching one view while its peer
remained readable, then removing all test loop/device-mapper devices. Initial
execution failed an immediate loop-removal assertion; the test now waits up to
five seconds for asynchronous removal. The successful repeat left no test loops.
Local `make test-quick TESTS=closure-store-linux-storage PYTEST_WORKERS=` reports
one explicit skip because local root privileges are unavailable. The full suite
was not rerun for these test-only changes; its last runtime baseline result is
714 passed / 24 skipped. Targeted Ruff and `git diff --check` passed.

Both builders are restored to clean `649469b`. Temporary worker scripts were
removed from sys-dev and dom0; machine-readable evidence is retained. No active
plan, build or deployment remains from these checks. No reboot is pending.

Evidence under `.airlock-logs/physical-20260929-r7/`:
`root-trial-return5.results.json`, `builder-host-reuse-shell.results.json`,
`linux-storage-retry.results.json`, `memory-trial-check.results.json`, and
`acceptance-cleanup.results.json`. Plan receipt/timings are under
`.airlock-logs/deployment-performance/memory-trial-plan/`.


### R7 fault handling and Nix behavior (2026-09-29)

Installed-engine disposable-pool checks passed for truncated/corrupt transfers,
failures after object import and before commit, unchanged existing generations,
staging cleanup and successful retries. Dom0 native build/query/isolated import
passed; sys-ctrl reused a host object from its lower view and built successfully
with a new input/output confined to its upper layer. Evidence: r7
`linux-storage-interruptions-complete-root`, `host-nix-behavior`, and
`builder-new-input` result JSON files. TODO 05 moved to `docs/completed/`;
06–07 remain open. These are injected failures, not physical power cuts.
