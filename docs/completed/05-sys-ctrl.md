# Move configuration work into sys-ctrl

Status: **complete** (2026-09-29). Typed controller deployment, reboot,
rollback, persistence, authorization checks, real storage integration and
interrupted-transfer cleanup/retry passed. Final fault coverage used the
installed engine with a disposable pool and real devices; it is injected-failure
coverage, not physical power-cut testing. Evidence: r7
`linux-storage-interruptions-complete-root.results.json` and the root-cycle
records. Related work in 06–07 is also complete; see their acceptance summaries.

Earlier checkpoints are preserved in the
[shared acceptance history](../history/machine-acceptance-2026-09-25-to-29.md).

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
- [x] Run the controller/storage integration tests on a booted image.
  Real typed deployment, verification and rollback passed. The privileged
  storage worker (`3a4e3d2`) passed on dom0 using a disposable pool and private
  mount namespace: real loop/device-mapper/EROFS, exact closure membership,
  shared payload extents, read-only enforcement and independent detach.
  The local unprivileged pytest entry reports skipped, not passed. Evidence:
  `linux-storage-retry.results.json` in the r7 directory.
- [x] Build and inspect the host transport closure: no retained guest-only
  source closures in dom0's ordinary store.
- [x] Cold boot starts the controller and required machine topology without a
  controller/store boot dependency cycle. Fourth-image baseline below records
  the healthy host, controller and declared autostart guests.
- [x] Submit a committed guest change and a whole-machine change through sys-dev;
  verify source provenance, exact generations and controller outcome records.
- [x] Repeated updates reuse pool objects; record transferred bytes, elapsed
  time, allocated storage and retained references.
- [x] Reboot and rollback preserve guest private data and retained generations.
  Both fourth-image reboot directions passed with exact host/guest selection
  and private/builder/host-derivation sentinels retained.
- [x] Restart or fail sys-ctrl during its update; recover using sys-dev's typed
  status/resume/rollback route and, if needed, the physical recovery console.
  R5 normal apply restarted sys-ctrl and continued independently in dom0;
  sys-dev status and post-reboot verify passed. R4 also exercised controller
  outage and rollback/recovery. Abrupt power-cut cases remain separate.
- [x] Reject unauthorized/malformed requests and incomplete artifacts; exercise
  interrupted transfers, retries and temporary-data cleanup.
  R7 live probes denied sys-dev's direct MachinePlan and usr-home's MachineControl
  calls (126), rejected malformed/truncated headers and invalid release paths,
  and rejected two incomplete Nix export submissions without registering the
  bogus object or creating an activation transaction. The installed engine also
  rejected truncated/corrupt payloads and injected failures after object import
  and before generation commit; existing manifests/selection survived, staging
  was empty and retries passed. Evidence: `controller-rejections.results.json`,
  `controller-incomplete-artifact.results.json` and
  `linux-storage-interruptions-complete-root.results.json` in the r7 directory.
- [x] Retention and GC preserve running and rollback generations across updates.
- [x] Host updates preserve the `reboot-required` / `complete` distinction.
  Fourth-image apply and normal reboot verified both states; rollback again
  reports `reboot-required` until its selected host boots.

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

<a id="r7-remaining-check-consolidation-2026-09-29"></a>

[R7 remaining-check consolidation (2026-09-29)](../history/machine-acceptance-2026-09-25-to-29.md#r7-remaining-check-consolidation-2026-09-29)
