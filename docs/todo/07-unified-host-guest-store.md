# Share immutable store objects between dom0 and guests

Current checkpoint (2026-09-28): **root-change acceptance failed; active testing
stopped to reduce cost**. Transaction `1790549788826426596` reached
`reboot-required`, selecting trial `da65cc9` / host `nnha9msy…`, but usr-home's
running disk is 1536 MiB while its current pool manifest still describes the
old 1024 MiB root. Do not count this trial as passed or request a reboot yet.
See the final failure checkpoint. Fix `ad3dfd6` is implemented and tested locally; deployment remains pending.

Current continuation: maintenance fix `fbe6062` booted successfully; typed
verification marked transaction `1790546188487824369` complete. Saved data,
builder additions and boot records passed; installed maintenance code hashes
match the commit. Recovery, GC and compaction passed. A machine plan for the
root-artifact trial is running. Overall acceptance remains open. See the final
continuation.

Latest status (2026-09-27, fourth image): **acceptance incomplete**. Normal
return from independent recovery passed. A committed usr-home-only update and
rollback also passed without a host reboot; only usr-home changed, the marker
was removed, and private data survived. Original source `5c87f84` is restored
in both builder VMs. Installed validators reject damaged metadata in disposable
copies. The missing-view recovery console test passed: the user booted recovery,
confirmed the metadata was absent, restored it, and successfully selected host
generation 1. The subsequent normal reboot passed: exact generations, saved
data and restored metadata hashes were verified, and the diagnostic directory
was removed. Other acceptance criteria remain open.

Current follow-up: maintenance and the queued root-image trial were stopped
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
- [ ] Sys-ctrl reuses an existing host object through its approved builder view
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
