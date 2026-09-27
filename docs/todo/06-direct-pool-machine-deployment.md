# Direct pool admission for whole-machine updates

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

- [ ] A whole-machine update prepared by sys-ctrl reuses existing pool objects
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
- [ ] Reboot and rollback work with direct admission and preserve guest private
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
