# Direct pool admission for whole-machine updates

Status (2026-09-25): outstanding. Direct guest admission exists, but the
whole-machine update path still imports guest packages into dom0's ordinary
Nix store. Controller integration has local validation and still needs physical
acceptance. This work is required by the [sys-ctrl migration](05-sys-ctrl.md).

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

- [Sys-ctrl migration and ownership](05-sys-ctrl.md)
- [Completed direct admission work](../completed/sys-dev-pool-admission.md)
- [Pool-backed builder](../sys-dev-build-store.md)
- [Admission policy and receiver](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
- [Artifact sender and receiver CLI](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/artifact_cli.py)
- [Artifact validation and admission](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/artifacts.py)
- [Release construction](../../airlock-machine/lifecycle/misc/modules/machine-release.nix)
- [Machine coordinator](../../airlock-machine/lifecycle/runtime/tools/machine-deploy.py)
- [Machine deployment tests](../../airlock-machine/tests/test_machine_deploy.py)
