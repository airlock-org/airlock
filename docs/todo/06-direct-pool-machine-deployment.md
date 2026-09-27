# Direct pool admission for whole-machine updates

Latest status (2026-09-27, fourth image): **acceptance incomplete**. Normal
return from independent recovery passed. A committed usr-home-only update and
rollback also passed without a host reboot; only usr-home changed, the marker
was removed, and private data survived. Original source `5c87f84` is restored
in both builder VMs. Installed validators reject damaged metadata in disposable
copies. The missing-view recovery console test passed: the user booted recovery,
confirmed the metadata was absent, restored it, and successfully selected host
generation 1. A normal reboot, restored-file verification and removal of the
remaining diagnostic directory are next. Other acceptance criteria remain open.

Status (2026-09-25): local implementation now includes the root-image reuse
work described below. The user subsequently ran `make test-quick`: 594 passed, 24 skipped, three
failed and two warnings. Two missed fixtures needed a root image and the new
`missing_root` response field; the parent flake check could not fetch the machine
commit from its local sibling origin. The fixtures are corrected and the machine
commit is now retained in that origin. These fixes have not been retested.
No image was built and no live session was changed for this work. Physical
acceptance and performance measurements remain open.

Earlier evidence: the earlier implementation passed the quick suite and
both flake checks (579 passed, 24 skipped, two existing warnings across
separate runs). The subsequent controller network change passed nine focused
tests; the full quick suite has not been rerun at that revision. See
[sys-ctrl validation evidence](05-sys-ctrl.md#remaining-acceptance-work).
The first image's whole-machine controller build was killed before pool
transfer. An 8 GiB controller later completed a diagnostic network-assisted
whole-machine build from the previous source revision, but that build did not
submit guest manifests or a machine plan. A separate guest-only trial built
and admitted a candidate, then dom0 rejected `GuestPlan`. The newly committed
source was pushed to sys-dev. The first whole-machine plan stopped before pool
transfer because the booted controller's offline build produced a `dirty`
manifest revision. A guest-only sys-ctrl update was then planned, applied and
verified complete. Its Qubes preferences reverted to the old image's 4 GiB and
no NetVM, so the next whole-machine build exited 137. Live preferences were
restored to 8 GiB and sys-firewall; the next whole-machine plan succeeded.
Guest admission transferred only 11 objects (1,907,712 payload bytes), all for
sys-dev; every other guest reused all its package objects. The host transport
imported 28 Nix objects in a 5,915,115,984-byte uncompressed export stream,
including VM root images. Admission, host transfer and planning together took
594.76 seconds after the build. These figures do not measure compressed wire
bytes, allocated disk use or the time spent in each step. Evidence:
`.airlock-logs/physical-controller-2026-09-25/updated-controller-restored-prefs-plan.log`.
No whole-machine release has been applied. Physical acceptance remains open.

The controller keeps a full release/build root and sends a separate transport
output containing host dependencies and detached guest topology metadata.
Guest artifacts use the existing incremental manifest/missing-object exchange;
dom0 binds the receipts to exact verified pool generations. The host transport
also negotiates missing ordinary-store paths before streaming its Nix export.
Retries reuse an exact retained admission with the same provenance. Admission
cleans fragment directories abandoned by process death under the pool lock.
Plans retain host transport roots; applying moves protection to transaction
roots, and release inventory/pruning tracks planned candidates.

No migration or compatibility protocol is required for this unreleased software.

[TODO 07](07-unified-host-guest-store.md) proposes extending the pool to dom0's
own Nix objects and enabling builder reuse of selected host objects. That work
is separate from this TODO's direct admission and root-image reuse criteria.

## Goal

Have sys-ctrl submit a guest package manifest, let dom0 reuse objects already
in the shared pool, and transfer only missing objects directly into that pool.
Dom0 still imports its own runtime dependencies into its ordinary Nix store.
Guest boot/root images must remain available under dom0's control, with reuse
of verified retained artifacts rather than a mandatory additional Nix-store
copy. Existing guest packages and unchanged root images should need validation
and generation metadata without another payload transfer.

Keep the shared pool at `/var/lib/airlock/closure-store` in dom0 and preserve
Xen, qrexec, QubesDB, qubesd, libvirt, and Qubes policy. Packages built in
sys-ctrl's private writable store still need admission into the trusted pool;
being on the same physical disk does not make them admitted shared objects.

## Implementation

- Reuse `airlock-pool-send`, the versioned artifact format, and the existing
  dom0 admission receiver. Preserve validation, object reuse, filtered guest
  views, and retained inactive generation publication.
- Connect admission results to the machine coordinator's release planning,
  activation, and rollback. Verify that admitted generations match the intended
  guest and release provenance before activation. Admission success alone is
  not deployment success; define activation authority separately.
- Exercise interruption, retry, and temporary-data cleanup through the complete
  controller/deployment path, preserving retained and running generations.
- Remove references in deployment releases/GC roots that would otherwise retain
  whole guest source closures in dom0's ordinary store. Keep dom0's actual
  runtime dependencies and boot/recovery artifacts intact.
- Keep retention and rollback working through pool objects and generation
  metadata. Do not require controller availability to keep running guests alive.
- Measure allocated storage and reference retention after repeated updates.
  Moving a duplicate elsewhere on dom0 does not satisfy this requirement.
- Use the manifest and validated pool inventory to send only missing objects.
  Reuse the existing admission protocol where possible; version any necessary
  extensions. Measure transferred bytes and elapsed admission time as well as
  storage use to show that unchanged payloads are reused.

## Reuse guest root images across image creation and deployment

The USB image seeds runnable guest storage and omits the original root-image
Nix outputs from dom0's retained dependencies. Before this change, machine releases added
those outputs through `system.extraDependencies` in `machine-release.nix`.
That receiver checked only registered Nix-store paths, so it imported the original
image files even when working VM disks already exist. The observed export
included seven root-image outputs. Their logical filesystem sizes include
empty space; this is separate from guest package duplication.

- Define a shared artifact identity and inventory for image creation and
  deployment. Record and verify content hashes, format, logical size and boot
  compatibility. An unchanged root artifact must retain its identity across
  unrelated repository, package or VM preference changes; control filesystem
  UUIDs and timestamps where they would otherwise make rebuilds differ.
- Seed a verified immutable root artifact in dom0-managed storage and bind it
  to guest generations. Reuse it only after validation. A running VM's writable
  root volume is not automatically an unchanged, reusable build artifact.
  Preserve the existing Qubes/Xen volume and boot mechanisms.
- Reuse must never alias writable volumes across VMs. Only an immutable base
  whose complete contents and boot requirements match may be shared; preserve
  separate per-VM writable layers, private volumes and ownership records.
  Keep VM-specific secrets and mutable identity out of shared bases, and bind
  each guest's generation to its validated artifact and volume identities.
- Negotiate missing root artifacts alongside package admission. Transfer only
  absent or genuinely changed roots, and preserve sparse allocation or use an
  equivalent representation that avoids streaming the full empty disk area.
- Replace unconditional guest root-image dependencies in the host transport
  with validated artifact identities only once dom0 durably retains the
  required boot and rollback artifacts. Keep dom0's own runtime, kernel and
  recovery dependencies available, including while sys-ctrl is stopped.
- Cover image seeding, admission, activation, interrupted retries, rollback
  and GC with the same ownership and retention rules. Avoid merely relocating
  duplicate images to another dom0 directory.

## Root-image implementation recorded locally

- The generic boot-image builder fixes its filesystem UUID, hash seed and inode
  timestamps, removes the VM name from its derivation identity, and emits a
  separate small descriptor output. Its contents contain boot scaffolding, not
  VM-specific secrets or writable guest state. Filesystem size, format and boot
  ABI are part of the validated descriptor.
- `root_format.py` defines a versioned canonical sparse representation and hashes
  its complete contents, including the logical size and positions of nonzero
  blocks. Holes and allocated zero blocks have the same identity. Receivers
  reject malformed, overlapping, truncated, oversized or corrupt payloads.
- Image preparation and deployment retain one verified immutable `root.img` per
  content hash under `closure-store/root-artifacts/`. Generation manifests bind
  each domain to its root descriptor and original source identity. The prepared
  pool checks these roots; offline Qubes preparation copies from the retained
  base into independent native root volumes and creates separate private disks.
  Image installation explicitly restores sparse allocation even if a substituted
  Nix seed materialized its holes.
- Incremental admission uses `airlock.pool-artifact.v3` with a versioned
  missing-object/root response. Reused roots send no payload. New roots travel
  as sparse extents alongside missing package fragments, with bounded sizes and
  durable publication before generation commit. Retries reuse verified roots;
  recovery removes temporary files and generation GC removes unreferenced bases.
- Release transport carries a small `root-artifacts.json` catalog instead of
  unconditional root-image Nix dependencies. Dom0 keeps its kernel dependencies.
  The coordinator independently describes admitted generations and compares
  their verified root descriptors with the release catalog before planning and
  activation. Candidate manifests preserve those bindings for later activation.
- Runtime setup resolves retained roots by domain, source and staged generation,
  checks that root/private volume identities are independent and unreferenced
  by other VMs, then uses native `qvm-volume import` into that VM's own root.
  Roots are rechecked when loading or switching generations, including rollback.
  The VM records its selected root content hash as well as its source identity;
  an existing disk is treated as current only when both match. Existing retention
  and running-generation protection pin their immutable bases.
- Reports separate package and sparse-root payload bytes, root logical and
  allocated bytes, unique root inventory, host NAR and compressed wire bytes,
  build time, inventory validation, payload reception, root admission, package
  admission and machine binding/planning. Host reception additionally reports
  read/decompression, importer pipe-write and final importer wait durations;
  these are wall-time phases of a pipeline, not independent CPU measurements.
  Guest payloads use uncompressed tar with sparse roots, so their payload byte
  counts exclude only tar/protocol framing rather than a compression ratio.
- Pytest source coverage was added for sparse identity, corruption, independent
  copied disks, retention, changed-root admission, retry, rollback and release
  binding. Existing prepared-image fixtures now provide root artifacts. The user
  ran the quick suite with the result recorded above; the follow-up fixture fixes
  have not been retested. Real Qubes volume isolation, reproducible
  rebuilds and the performance criteria below still require validation.

## Completion criteria and evidence

- [ ] On the first deployment from a newly booted image and on repeated updates,
  unchanged root images transfer no payload and require no extra ordinary-store
  copy. Test a memory/NetVM-only change and a guest package-only change.
- [ ] A genuine boot/root-image change admits the required new artifact;
  corrupt or incomplete artifacts are rejected. Boot and rollback work with
  sys-ctrl stopped, and GC preserves roots required by retained generations.
- [ ] Two VMs using the same immutable base retain distinct writable and
  private volumes. Writes, rollback and deletion in one VM cannot alter the
  other's data; updating one VM cannot switch the other's selected artifact.
- [ ] Measure root-image and package bytes separately, including compressed
  transfer bytes, logical image sizes and allocated storage. Record build,
  validation, transfer and import times separately to explain remaining cost.

- [x] A whole-machine update prepared by sys-ctrl reuses existing pool objects
  and transfers only missing guest objects; record bytes and elapsed time for
  initial and repeated updates.
- [ ] Guest-only source closures are not retained in dom0's ordinary store.
  Verify release/GC references and allocated disk usage across repeated updates.
- [ ] Dom0 runtime and boot/recovery dependencies remain available independently
  of sys-ctrl, and host updates preserve whole-machine generation verification.
- [ ] Admitted generations match the intended guest and release provenance;
  incomplete or invalid artifacts cannot be activated.
- [ ] Interrupted transfers, retries, and cleanup preserve running and retained
  generations without accumulating unbounded temporary data.
- [x] Reboot and rollback work with direct admission and preserve guest private
  data; stopping sys-ctrl leaves running guests' storage usable.
- [ ] Retention and GC preserve active and rollback generations after repeated
  deployments.

Read [machine-deployment.md](../machine-deployment.md) before physical work.
Implement and commit changes in the repository, use pytest and `make test-quick`,
and deploy through the documented machine-plan/machine-apply workflow. Record
machine-readable evidence under `.airlock-logs/`; report local validation and
physical deployment separately. Use only guarded Airlock GC targets.

## Starting points

- [Sys-ctrl ownership](05-sys-ctrl.md)
- [Completed direct admission work](../completed/sys-dev-pool-admission.md)
- [Pool-backed builder](../sys-dev-build-store.md)
- [Admission policy and receiver](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
- [Artifact sender and receiver CLI](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/artifact_cli.py)
- [Artifact validation and admission](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/artifacts.py)
- [Release construction](../../airlock-machine/lifecycle/misc/modules/machine-release.nix)
- [Machine coordinator](../../airlock-machine/lifecycle/runtime/tools/machine-deploy.py)
- [Machine deployment tests](../../airlock-machine/tests/test_machine_deploy.py)

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

Follow-up: the second image booted but host Nix rejected stale `/sysroot`
overlay option paths. The corrected mount helper passed an isolated live
query/write probe without modifying the active store. See TODO 07's “Second
image trial” section; full deployment acceptance remains blocked on a rebuilt
image containing that fix.

## Third image trial, 2026-09-27

The c15785b image boots with valid final-root overlay paths. Queries and a
small write through the real host Nix daemon succeeded. Controller memory,
network binding, host dependency visibility and policy rejection checks passed.
The initial controller plan failed at host binding; no release is selected.
A sys-dev builder restart exposed a read-only database bind left mounted by
`os.path.ismount`; source now checks the kernel mount table. Clearing that stale
bind restored the original service with its added package and private data
intact. Deployment of the fix and full reboot/rollback acceptance remain open.
See TODO 07's third-trial section for evidence and remaining boundaries.

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

### Pending diagnostic mutation: missing-view recovery boot

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
Next: manually reboot normal `Airlock`, verify exact generations, saved data and
the restored metadata hash, then remove the diagnostic directory's remaining
symlink and record. That cleanup and return-to-normal verification remain pending.
