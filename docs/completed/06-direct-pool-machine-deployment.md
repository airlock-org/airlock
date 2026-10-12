# Direct pool admission for whole-machine updates

Status: **complete** (2026-09-29). Root update/reboot/rollback, controller-disabled
offline boot, isolation, persistence, recovery, maintenance and measured planning
passed within the documented physical/automated boundaries. Final fixes
`4845da0` and `b1cf5db` passed **720 quick tests, 25 skipped** and normal deployment.
After booting **Airlock (Generation 10)**, typed verification marked transaction
`1790721069384601443` complete; both normal builders are healthy, saved markers
match, and temporary recovery units/scripts are absent. Host pool generation 9
is mounted with its own database; boot-menu generation numbers are independent.
Evidence: `.airlock-logs/boot-verification-fix/final-apply/boot10-verify.json`,
r7 `boot10-final-checks.results.json`, and the shared milestone history.
See the [shared acceptance history](../history/machine-acceptance-2026-09-25-to-29.md)
for the dated milestones and earlier checkpoints.

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

[Completed 07](07-unified-host-guest-store.md) extends the pool to dom0's
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

- [x] On the first deployment from a newly booted image and on repeated updates,
  unchanged root images transfer no payload and require no extra ordinary-store
  copy. Test a memory/NetVM-only change and a guest package-only change.
- [x] A genuine boot/root-image change admits the required new artifact;
  corrupt or incomplete artifacts are rejected. Boot and rollback work with
  sys-ctrl stopped, and GC preserves roots required by retained generations.
- [x] Two VMs using the same immutable base retain distinct writable and
  private volumes. Writes, rollback and deletion in one VM cannot alter the
  other's data; updating one VM cannot switch the other's selected artifact.
- [x] Measure root-image and package bytes separately, including compressed
  transfer bytes, logical image sizes and allocated storage. Record build,
  validation, transfer and import times separately to explain remaining cost.

- [x] A whole-machine update prepared by sys-ctrl reuses existing pool objects
  and transfers only missing guest objects; record bytes and elapsed time for
  initial and repeated updates.
- [x] Guest-only source closures are not retained in dom0's ordinary store.
  Verify release/GC references and allocated disk usage across repeated updates.
- [x] Dom0 runtime and boot/recovery dependencies remain available independently
  of sys-ctrl, and host updates preserve whole-machine generation verification.
- [x] Admitted generations match the intended guest and release provenance;
  incomplete or invalid artifacts cannot be activated.
- [x] Interrupted transfers, retries, and cleanup preserve running and retained
  generations without accumulating unbounded temporary data.
- [x] Reboot and rollback work with direct admission and preserve guest private
  data; stopping sys-ctrl leaves running guests' storage usable.
- [x] Retention and GC preserve active and rollback generations after repeated
  deployments.

Read [machine-deployment.md](../machine-deployment.md) before physical work.
Implement and commit changes in the repository, use pytest and `make test-quick`,
and deploy through the documented machine-plan/machine-apply workflow. Record
machine-readable evidence under `.airlock-logs/`; report local validation and
physical deployment separately. Use only guarded Airlock GC targets.

## Starting points

- [Sys-ctrl ownership](05-sys-ctrl.md)
- [Completed direct admission work](sys-dev-pool-admission.md)
- [Pool-backed builder](../usr-assistant-build-store.md)
- [Admission policy and receiver](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
- [Artifact sender and receiver CLI](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/artifact_cli.py)
- [Artifact validation and admission](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/artifacts.py)
- [Release construction](../../airlock-machine/lifecycle/misc/modules/machine-release.nix)
- [Machine coordinator](../../airlock-machine/lifecycle/runtime/tools/machine-deploy.py)
- [Machine deployment tests](../../airlock-machine/tests/test_machine_deploy.py)

## Shared physical acceptance history

The [2026-09-25 to 2026-09-29 acceptance report](../history/machine-acceptance-2026-09-25-to-29.md)
contains the combined trial timeline, failures, diagnostic mutations, cleanup,
reboot/rollback observations and final acceptance evidence. The status and
criteria above define this plan's completed scope.

Existing section bookmarks are preserved below and forward to that report.

<a id="combined-physical-acceptance-2026-09-27"></a>

[Combined physical acceptance, 2026-09-27](../history/machine-acceptance-2026-09-25-to-29.md#combined-physical-acceptance-2026-09-27)

<a id="third-image-trial-2026-09-27"></a>

[Third image trial, 2026-09-27](../history/machine-acceptance-2026-09-25-to-29.md#third-image-trial-2026-09-27)

<a id="initial-third-trial-planning-result"></a>

[Initial third-trial planning result](../history/machine-acceptance-2026-09-25-to-29.md#initial-third-trial-planning-result)

<a id="fixed-source-plan-succeeded"></a>

[Fixed-source plan succeeded](../history/machine-acceptance-2026-09-25-to-29.md#fixed-source-plan-succeeded)

<a id="repeat-planning-and-apply-request"></a>

[Repeat planning and apply request](../history/machine-acceptance-2026-09-25-to-29.md#repeat-planning-and-apply-request)

<a id="offline-usb-inspection-after-failed-restart-2026-09-27"></a>

[Offline USB inspection after failed restart, 2026-09-27](../history/machine-acceptance-2026-09-25-to-29.md#offline-usb-inspection-after-failed-restart-2026-09-27)

<a id="fourth-image-boot-and-isolated-transition-checks-2026-09-27"></a>

[Fourth image: boot and isolated transition checks, 2026-09-27](../history/machine-acceptance-2026-09-25-to-29.md#fourth-image-boot-and-isolated-transition-checks-2026-09-27)

<a id="fourth-image-planapply-result-manual-reboot-required"></a>

[Fourth-image plan/apply result: manual reboot required](../history/machine-acceptance-2026-09-25-to-29.md#fourth-image-planapply-result-manual-reboot-required)

<a id="selected-host-reboot-passed-rollback-awaits-its-reboot"></a>

[Selected-host reboot passed; rollback awaits its reboot](../history/machine-acceptance-2026-09-25-to-29.md#selected-host-reboot-passed-rollback-awaits-its-reboot)

<a id="rollback-reboot-and-physical-storage-checks-passed"></a>

[Rollback reboot and physical storage checks passed](../history/machine-acceptance-2026-09-25-to-29.md#rollback-reboot-and-physical-storage-checks-passed)

<a id="independent-recovery-smoke-test-passed-user-console"></a>

[Independent recovery smoke test passed (user console)](../history/machine-acceptance-2026-09-25-to-29.md#independent-recovery-smoke-test-passed-user-console)

<a id="recovery-return-and-committed-single-guest-update-passed"></a>

[Recovery return and committed single-guest update passed](../history/machine-acceptance-2026-09-25-to-29.md#recovery-return-and-committed-single-guest-update-passed)

<a id="missing-view-recovery-fault-setup-subsequently-restored"></a>

[Missing-view recovery fault setup (subsequently restored)](../history/machine-acceptance-2026-09-25-to-29.md#missing-view-recovery-fault-setup-subsequently-restored)

<a id="missing-view-recovery-console-result"></a>

[Missing-view recovery console result](../history/machine-acceptance-2026-09-25-to-29.md#missing-view-recovery-console-result)

<a id="maintenance-and-interruption-acceptance-follow-up"></a>

[Maintenance and interruption acceptance follow-up](../history/machine-acceptance-2026-09-25-to-29.md#maintenance-and-interruption-acceptance-follow-up)

<a id="maintenance-stopped-at-user-request"></a>

[Maintenance stopped at user request](../history/machine-acceptance-2026-09-25-to-29.md#maintenance-stopped-at-user-request)

<a id="compaction-fix-deployment-continuation-2026-09-27"></a>

[Compaction-fix deployment continuation (2026-09-27)](../history/machine-acceptance-2026-09-25-to-29.md#compaction-fix-deployment-continuation-2026-09-27)

<a id="maintenance-release-selected-manual-reboot-pending-2026-09-27"></a>

[Maintenance release selected; manual reboot pending (2026-09-27)](../history/machine-acceptance-2026-09-25-to-29.md#maintenance-release-selected-manual-reboot-pending-2026-09-27)

<a id="maintenance-release-boot-verified-follow-up-running"></a>

[Maintenance release boot verified; follow-up running](../history/machine-acceptance-2026-09-25-to-29.md#maintenance-release-boot-verified-follow-up-running)

<a id="root-trial-route-corrected"></a>

[Root-trial route corrected](../history/machine-acceptance-2026-09-25-to-29.md#root-trial-route-corrected)

<a id="changed-root-machine-plan-passed-apply-submitted"></a>

[Changed-root machine plan passed; apply submitted](../history/machine-acceptance-2026-09-25-to-29.md#changed-root-machine-plan-passed-apply-submitted)

<a id="root-identity-mismatch-stop-costly-monitoring-2026-09-28"></a>

[Root identity mismatch; stop costly monitoring (2026-09-28)](../history/machine-acceptance-2026-09-25-to-29.md#root-identity-mismatch-stop-costly-monitoring-2026-09-28)

<a id="root-generation-fix-implemented-locally-2026-09-28"></a>

[Root-generation fix implemented locally (2026-09-28)](../history/machine-acceptance-2026-09-25-to-29.md#root-generation-fix-implemented-locally-2026-09-28)

<a id="fixed-deployment-started-2026-09-28"></a>

[Fixed deployment started (2026-09-28)](../history/machine-acceptance-2026-09-25-to-29.md#fixed-deployment-started-2026-09-28)

<a id="fixed-deployment-passed-pre-reboot-checks-2026-09-28"></a>

[Fixed deployment passed pre-reboot checks (2026-09-28)](../history/machine-acceptance-2026-09-25-to-29.md#fixed-deployment-passed-pre-reboot-checks-2026-09-28)

<a id="reboot-failed-during-host-store-assembly-2026-09-29-inspection"></a>

[Reboot failed during host-store assembly (2026-09-29 inspection)](../history/machine-acceptance-2026-09-25-to-29.md#reboot-failed-during-host-store-assembly-2026-09-29-inspection)

<a id="mount-failure-diagnostic-follow-up-2026-09-29"></a>

[Mount-failure diagnostic follow-up (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#mount-failure-diagnostic-follow-up-2026-09-29)

<a id="new-baseline-authorized-2026-09-29"></a>

[New baseline authorized (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#new-baseline-authorized-2026-09-29)

<a id="r7-changed-root-activation-passed-generation-6-reboot-pending-2026-09-29"></a>

[R7 changed-root activation passed; generation 6 reboot pending (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-changed-root-activation-passed-generation-6-reboot-pending-2026-09-29)

<a id="r7-changed-root-generation-6-boot-passed-2026-09-29"></a>

[R7 changed-root generation 6 boot passed (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-changed-root-generation-6-boot-passed-2026-09-29)

<a id="r7-root-rollback-passed-pre-reboot-verification-2026-09-29"></a>

[R7 root rollback passed pre-reboot verification (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-root-rollback-passed-pre-reboot-verification-2026-09-29)

<a id="r7-remaining-check-consolidation-2026-09-29"></a>

[R7 remaining-check consolidation (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-remaining-check-consolidation-2026-09-29)

<a id="r7-fault-handling-and-nix-behavior-2026-09-29"></a>

[R7 fault handling and Nix behavior (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-fault-handling-and-nix-behavior-2026-09-29)

<a id="r7-evidence-consolidation-2026-09-29"></a>

[R7 evidence consolidation (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-evidence-consolidation-2026-09-29)
