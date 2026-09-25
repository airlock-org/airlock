# Direct pool admission for whole-machine updates

Status (2026-09-25): the earlier implementation passed the quick suite and
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
restored to 8 GiB and sys-firewall; another whole-machine plan is running.
Pool reuse, transfer bytes, storage use and performance remain unmeasured. No
whole-machine release has been applied. Physical acceptance remains open.

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

## Goal

Have sys-ctrl submit a guest package manifest, let dom0 reuse objects already
in the shared pool, and transfer only missing objects directly into that pool.
Dom0 still imports its own runtime dependencies and boot/recovery artifacts
into its ordinary Nix store. Existing guest packages should need validation and
generation metadata, without another payload transfer or ordinary-store import.

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

## Completion criteria and evidence

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
