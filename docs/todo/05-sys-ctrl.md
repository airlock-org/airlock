# Move configuration work into sys-ctrl

Status (2026-09-25): local implementation is ready for review; validation remains
outstanding. This pass made source changes only: no tests, Nix builds, image
builds, or live deployments were run. Do not treat this document as physical
acceptance.

Airlock is unreleased. Backwards compatibility and migration from the old
`sys-config-*` workflow are not requirements. Those roles have been removed
from the active bootstrap topology and standalone domain outputs; no migration
service or compatibility deployment protocol is added.

## Ownership and request flow

| Responsibility | Owner |
| --- | --- |
| Committed candidate submission | sys-dev / development repository |
| Candidate source history, validation, builds and writable Nix store | sys-ctrl |
| Release preparation and admission requests | sys-ctrl |
| Shared guest pool, generation validation, retention and GC | dom0 |
| Qubes lifecycle, devices, policy, activation and recovery records | dom0 |
| Recording observed deployment outcomes | sys-ctrl, using dom0 status |

Normal submission is `sys-dev -> sys-ctrl -> dom0`. Preserve Xen, qrexec,
QubesDB, qubesd, libvirt and Qubes policy. Sys-ctrl is highly trusted because it
prepares executable systems. It has no physical NIC or network VM by default;
builds use the pinned offline inputs and the pool-backed private store. A lock
serializes candidate import, build and submission in the controller.

The normal machine client uses typed qrexec commands. Status, verification,
resume, rollback, release retention/pruning and trash operations use a bounded
command schema directly from sys-dev to dom0. This recovery path remains
available when sys-ctrl is stopped. The existing development shell is separate
from normal deployment authority.

## Local implementation

- Whole-machine build/plan/apply and guest requests submit committed source to
  sys-ctrl. Source bundles are bounded and checked against the revision and
  lock hash before building.
- Sys-ctrl saves candidate source archives and request records under
  `~/.local/share/airlock/controller/candidates/<revision>/`. Build roots remain
  under `~/.local/share/airlock/machine-deploy/<revision>/`.
- [Direct whole-machine pool admission](06-direct-pool-machine-deployment.md)
  sends missing guest objects to the pool and only host/transport dependencies
  to dom0's ordinary store. Dom0 binds exact validated generation receipts to
  a durable machine candidate before planning or activation.
- Apply runs as a detached dom0 worker, including updates that restart sys-ctrl.
  A controller timer records dom0's observed outcome. A submitted worker does
  not count as success; `current.json` advances only after dom0 reports
  `complete` and the selected host is running.
- The old `sys-config-*` declarations, standalone outputs and active policy
  wiring are retired. Reusable configuration-library code is separate from the
  active controller workflow. No old candidate data is migrated.
- Host-side `make machine-plan` / `make machine-apply` for `usb-lab` push the
  committed source through sys-dev and invoke the controller client. The
  explicitly selected development transport remains a diagnostic tool.

## Remaining acceptance work

All checks below remain open until actually exercised and recorded.

- [ ] Run `make test-quick` and the relevant controller/storage integration tests.
- [ ] Build and inspect the host transport closure: no retained guest-only
  source closures in dom0's ordinary store.
- [ ] Cold boot starts the controller and required machine topology without a
  controller/store boot dependency cycle.
- [ ] Submit a committed guest change and a whole-machine change through sys-dev;
  verify source provenance, exact generations and controller outcome records.
- [ ] Repeated updates reuse pool objects; record transferred bytes, elapsed
  time, allocated storage and retained references.
- [ ] Reboot and rollback preserve guest private data and retained generations.
- [ ] Restart or fail sys-ctrl during its update; recover using sys-dev's typed
  status/resume/rollback route and, if needed, the physical recovery console.
- [ ] Reject unauthorized/malformed requests and incomplete artifacts; exercise
  interrupted transfers, retries and temporary-data cleanup.
- [ ] Retention and GC preserve running and rollback generations across updates.
- [ ] Host updates preserve the `reboot-required` / `complete` distinction.

Read [machine-deployment.md](../machine-deployment.md) before physical work.
Commit changes in `airlock-machine` first, then its parent submodule pointer.
Use the repository deployment workflow and capture evidence in `.airlock-logs/`.
Use only guarded Airlock GC targets. Report source, builds and physical
verification separately.

## Implementation entry points

- [Controller client/server](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-guest-deploy.py)
- [Detached guest activation](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-guest-receive.py)
- [Dom0 release receiver](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-machine-receive.py)
- [Typed observation and recovery](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-machine-control.py)
- [Controller packaging and outcome timer](../../airlock-machine/lifecycle/misc/modules/sys-dev-machine-build.nix)
- [Deployment services and policy](../../airlock-machine/lifecycle/misc/modules/machine-deployment.nix)
- [Release construction](../../airlock-machine/lifecycle/misc/modules/machine-release.nix)
- [Coordinator](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/coordinator.py)
- [Pool-backed builder](../sys-dev-build-store.md)

Moving the pool into sys-ctrl, replacing the Xen storage backend or Qubes control
mechanisms, and full GUI completion are outside this work.
