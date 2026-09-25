# Move configuration work into sys-ctrl

Status (2026-09-25): the earlier implementation passed the quick checks:
579 passed, 24 skipped, two existing warnings. The machine and parent image
flake checks ran separately with `--no-build`. After the network change, the
focused controller, candidate, topology and USB-lab checks passed (nine tests);
the full quick suite has not been rerun at this revision. The first physical
plan failed with exit 137 in the 4 GiB controller. An 8 GiB live controller
subsequently built the whole-machine release with network access, but only as
a diagnostic build from the previous source revision. The current source
declares 8 GiB, one build job, sys-firewall routing and online fetching. It
was pushed to sys-dev. A guest-only update then installed that sys-ctrl system;
dom0 status and verify reported `complete` with its selected and running guest
generation matching. Whole-machine planning still has not succeeded. No
whole-machine release has been applied. Physical acceptance remains outstanding.

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
prepares executable systems. It has no physical NIC; its network VM is
sys-firewall. Builds use the committed lock file and the pool-backed private
store, fetching missing inputs or substitutes as needed. A lock
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

## First physical trial

The booted sys-dev checkout matched the committed machine revision. Typed dom0
status and release inventory worked, and the restricted pool RPC refused a
request from sys-dev. The first `airlock-sys-dev-machine-deploy plan` reached
sys-ctrl, imported the committed source and pinned inputs, then its Nix build
exited 137. The controller remained responsive and rejected a malformed request
after the failed plan. Evidence is in
`.airlock-logs/physical-controller-2026-09-25/`. Exit 137 indicates SIGKILL;
controller memory pressure is the likely cause: local evaluation of the same
release derivation peaked at 5,693,196 KiB resident memory. The VM journal was
not accessible through the typed observation interface. The controller sizing
change passed the machine flake check, controller bootstrap candidate check, USB
lab topology check, and parent image flake check. A guest-only `usr-home` plan
subsequently built and admitted a candidate, but dom0 rejected `GuestPlan`.
The unchanged guest system may have caused this, but the old qrexec client
concealed the receiver error. The client and receiver now report bounded
stderr for a conclusive diagnosis on the next image. No guest apply ran.
The 8 GiB live plan progressed to missing fixed-output sources. A diagnostic
Nix archive imported the libssh2 patch, then the next missing source was a
Python tarball. Temporarily assigning sys-firewall to the live controller
confirmed network access, but the installed controller still passed `--offline`
to Nix. The declared network route and controller build arguments now permit
fetches. These live changes and diagnostic imports do not constitute verified
deployment of the new source generation.

## Current physical trial

The one-time diagnostic build without `--offline` finished successfully in the
live 8 GiB controller and produced an `airlock-machine-release` store path. It
used the previously submitted source revision `18c0b99`; this proves the VM can
build when missing inputs may be fetched, but does not test the newly packaged
controller service, pool transfer or dom0 planning. The live sys-ctrl NetVM
setting now matches the committed source; the booted image still packages the
old controller code, which passes `--offline` to Nix. The new source revision
`d78f134` was pushed to sys-dev with a clean checkout and matching lock hash.
The first committed-source plan built a release, but the old controller's
`--offline` input overrides made its manifest revision `dirty`; its lock hash
still matched. A manual build without `--offline` produced a release manifest
with the exact `d78f134` revision and lock hash. The guest-only sys-ctrl plan
and apply then succeeded through the supported route. Dom0 reported `complete`
and `machine-verify` passed for the selected and running sys-ctrl generation.
This updated the controller code, but the guest-only candidate inherited the
booted image's old Qubes preferences: 4 GiB and no NetVM. The next whole-machine
plan reached the updated controller but its build exited 137. The live sys-ctrl
preferences were restored to 8 GiB and sys-firewall, sys-ctrl was restarted,
and a new plan was started. It has no recorded result yet. No host reboot or
whole-machine apply has occurred. Evidence is under
`.airlock-logs/physical-controller-2026-09-25/`, including the plan and guest
logs; the original diagnostic build log and exit marker are in sys-ctrl's
`~/.local/share/airlock/physical-tests/`.

## Remaining acceptance work

- [x] Run the quick suite, including machine and parent image flake evaluation.
  Evidence: `.airlock-logs/pytest-report.json`,
  `.airlock-logs/flake-check-no-build/pytest-command.log`, and
  `.airlock-logs/controller-image-check-junit.xml`.
- [ ] Run the controller/storage integration tests on a booted image.
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
