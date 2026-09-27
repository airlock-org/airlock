# Move configuration work into sys-ctrl

Latest status (2026-09-27, fourth image): **acceptance incomplete**. The
`5c87f84` release passed a real host reboot: typed verification reported
`complete`, with exact host/guest generations, saved private data, builder
additions and the saved host derivation/output mapping intact. Rollback was
then requested through sys-dev with sys-ctrl stopped. It restored the image's
guest versions and selected host pool generation 1, reaching `reboot-required`.
Data and boot-artifact verification passed again. The next step is a manual
normal reboot to finish rollback acceptance. Recovery, GC/compaction and
remaining fault/isolation/performance criteria are still open.

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
- [x] Cold boot starts the controller and required machine topology without a
  controller/store boot dependency cycle. Fourth-image baseline below records
  the healthy host, controller and declared autostart guests.
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
