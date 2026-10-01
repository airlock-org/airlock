# 14: Separate bootstrap from normal startup

Status: **completed for the implemented scope; first physical reboot accepted**
(2026-10-01). The operator requested closeout with remaining checks recorded.
Repeated unchanged-boot reuse, fresh-image initialization, rollback and physical
failure/recovery trials remain unverified.

An unchanged, previously initialized machine should reach a usable sys-gui login
without repeating full topology setup. Reuse the configuration already applied
through Qubes, perform the checks needed for this boot's GUI and input startup,
and reserve detailed bootstrap work and presentation for initialization,
configuration changes and recovery.

This concerns the pre-GUI **Airlock | Startup** console, not the
[first-run setup wizard](../todo/first-run-setup-wizard.md). Hiding the console alone
would leave the underlying delay in place.

## Previous behavior and evidence

These findings describe startup before this implementation.

- The [coordinator's boot wrapper](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/coordinator.py)
  selects the release and invokes its setup program on every boot. There is no
  shortcut for an exact configuration that was already successfully applied.
- [Physical GPU startup](../../airlock-machine/lifecycle/misc/modules/gui-gpu.nix)
  requires full topology setup before sys-gui starts. Application and builder
  VMs are inspected even when they are not needed to reach login.
- [Setup-config](../../airlock-machine/lifecycle/misc/tools/scripts/setup-config.sh)
  reuses existing roots, kernels and volumes, but still queries preferences,
  features, services, storage identity and device assignments per VM.
  A current store source still invokes the storage helper.
- The same four-phase screen appears on later boots. Preparation/copying
  messages are also shown when existing state is merely being checked.
- The existing `boot-critical` scope selects autostart VMs and their NetVM
  dependencies. Physical sys-gui deliberately has autostart disabled; this
  selector is not a sufficient definition of what is needed for GUI startup.

Archived lab evidence under
`.airlock-logs/network-provider/boot-20260925-1310/` includes a boot whose
topology setup ran from 12:05:43 to 12:06:47: **64 seconds with every reported
configuration and storage change counter at zero**. Another saved setup log
contains **251 unchanged preference/feature/service checks** and storage-helper
calls for all seven VMs. These are historical observations, not a controlled
benchmark or current-installation timing. Per-operation costs and expected
savings still need measurement.

## Required behavior

| Situation | Behavior |
| --- | --- |
| Initial image setup | Provision required topology and storage with detailed progress. |
| Unchanged applied configuration | Reuse saved Qubes state and run bounded checks required for GUI and input startup. |
| Configuration change or rollback | Apply required differences through existing deployment machinery; reconcile pending boot work explicitly. |
| Interrupted operation or missing/inconsistent state | Report the condition and use an explicit repair or recovery path. |

Keep Nix and committed source authoritative, with existing Qubes runtime state
such as `qubes.xml` retained. Preserve Xen, qrexec, QubesDB, qubesd, libvirt,
Qubes policy and the closure-store architecture.

## Implementation

Checked items describe implemented behavior. Physical acceptance is recorded
separately below.

- [ ] Measure unchanged boots before altering startup. Attribute time to topology
  queries, storage validation, hardware guards, GPU handoff and guest startup.
  Record time to usable GUI/input separately from all autostart guests being ready.
- [x] Separate configuration application from ordinary boot. Reuse existing
  deployment journals and selected-release state where practical. If a completion
  record is needed, bind it to the exact applied configuration and durable
  successful completion; a permanent initialized flag is insufficient.
- [x] Define invalidation and recovery for changes, rollback, failed or interrupted
  application, missing VMs/volumes and inconsistent state. Preserve selected
  guest generations and private storage; never silently restore the seed image's
  original topology.
- [x] Define the actual prerequisites for a usable GUI, including sys-gui even
  though its ordinary Qubes autostart is disabled. Remove the full-topology
  dependency only once those prerequisites are enforced independently.
- [x] Keep required checks of current runtime state: Qubes service readiness,
  GPU ownership/handoff, input initialization and receiver readiness, and the
  started guest's expected system identity. Preserve artifact integrity and
  volume isolation; distinguish essential validation from redundant work.
- [x] Keep unrelated application and builder VM preparation outside the login
  dependency chain. Preserve declared autostart behavior and real dependencies,
  and report background failures without claiming that startup fully succeeded.
- [x] Use a brief startup indication on ordinary boots. Retain detailed progress
  for initialization, changes and repair, with actionable diagnostics for delays
  and failures. Display checking/reuse accurately instead of implying copying
  or provisioning when nothing changes.

## Implemented mechanism

The [boot configuration manager](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/boot.py)
uses the selected release and deployment journal, with a durable receipt bound
to topology, setup program and staged guest generations. Saved Qubes XML,
managed policy, storage descriptors and artifact metadata determine whether
configuration can be reused. Missing private storage and interrupted deployment
state produce recovery errors instead of silently restoring the image topology.
Existing storage and guest-start integrity checks still run.

Physical startup configures sys-gui and its actual dependency closure first,
then performs GPU handoff, exact guest-generation readiness and input connection.
Remaining VM configuration, network providers and declared autostart guests run
after that boundary. Background failures retain degraded-startup reporting.
Unchanged configurations use a brief console indication; provisioning retains
detailed progress with checking/reuse wording.

Component implementation `2c656cb` and console-test correction `ac6d549` were
merged into main; parent pins were `fa683f8` and `f2b547e`. The implementation
worktree and branches were retired after confirming they were clean and merged.

## Acceptance

- [ ] Repeated unchanged boots skip full topology reconciliation and unnecessary
  per-VM setup calls before login, while sys-gui and input remain usable.
- [ ] Fresh initialization, changed configurations, rollback and interrupted
  application each follow the intended path without losing private data or
  skipping required work.
- [ ] Missing/inconsistent state and required hardware or guest failures produce
  accurate diagnostics and preserve recovery behavior.
- [ ] Normal boots have concise presentation; initialization and repair retain
  useful progress. Preserve console-to-GPU handoff and diagnostic console access.
- [x] Add meaningful pytest coverage for boot-path selection, completion/state
  invalidation, prerequisites and recovery. Run `make test-quick`; keep Nix
  evaluation/build checks opt-in through `make test-nix`.
- [x] Follow [machine-deployment.md](../machine-deployment.md) for implementation:
  commit source, use normal plan/apply, and verify the whole-machine generation.
  Record deployment and post-reboot generation verification separately from
  build/test success.
- [x] Confirm the first deployed boot reaches working login, keyboard/mouse and
  application launch, with background startup completing and no failed dom0 units.
- [x] Record adjacent-boot service timings under `.airlock-logs/`, separating
  the GUI/input guard boundary from all autostart guests becoming ready.
- [ ] Measure repeated unchanged boots and precise time to usable login/input.
  Attribute remaining costs to queries, storage, hardware and guest startup.

## Live results (2026-10-01)

The operator ran `make test-quick`: **750 passed, 59 skipped in 6.64 seconds**.
New pytest coverage exercises scope selection, reuse/invalidation, dependencies,
missing private data, interrupted selection and background failure. No separate
`make test-nix` run was performed.

Normal `make machine-plan DEPLOY_TARGET=sys-dev` built machine revision
`ac6d549` in **111.92 seconds** and prepared it in **99.90 seconds**. Its plan
changed no guest generations and stopped no VMs. Apply reused the prepared
candidate with zero further builds or transfers, using worker
`airlock-machine-deploy-fca165ffbfd4`, transaction `1790862175006495260`.

After the operator rebooted, typed verification reported **complete**: running
and selected host both resolve to
`/nix/store/vxa5n5j8nyql4bkdafikd2mnr0px1v05-nixos-system-dom0-26.11.20260904.801bef6`,
and all six running guests match their selections. Usr-browser remains stopped.
The current kernel boot ID is `da35db76-d32a-45e3-b8e3-d92877614e73`; the typed
verification record's `boot_id` retains the earlier transaction boot ID.
Dom0 reported no failed units, and the operator confirmed login, keyboard/mouse
and opening an application all work.

| Observation | Previous boot | First boot with this change |
| --- | --- | --- |
| Foreground configuration duration | 80.77 s, full topology | 12.66 s, sys-gui only |
| Background configuration duration | Included in foreground | 63.27 s |
| GUI/input guard boundary after kernel boot | GPU service finished at 151.35 s; input at 149.81 s | Desktop guard boundary at 75.35 s |
| All autostart guests ready | Autostart service finished at 226.28 s | Readiness event at 203.97 s; service finished at 205.18 s |

These are adjacent boots, not a controlled benchmark. Service readiness is not
the precise instant visible login becomes usable. The first new boot reported
`reused: false` for both scopes and established the receipt; it demonstrates
foreground/background separation, not repeated unchanged-boot reuse.

Evidence is under `.airlock-logs/normal-startup/deploy-20261001/`, including
`plan.json`, `apply.json`, `after-reboot-verify.json`, `after-reboot-dom0.txt`
and `first-boot-summary.json`. The operator's quick-test report is under
`/run/media/user/data/airlock/logs/pytest-report.json`. Inspection used the existing
development dom0 terminal; no ad-hoc live configuration fix or overlay was used.

## Remaining verification at closeout

The unchecked acceptance items remain unverified. In particular, another
unchanged boot is needed to confirm `reused: true` in both scopes, skipped setup
calls and concise presentation. Fresh initialization, configuration change,
rollback, interrupted application, missing-state and hardware/guest failure
trials were not exercised physically for this change. The controlled timing
study and precise operator-observed login timing were also not completed.

Implementation belongs in `airlock-machine`; update the parent pin and affected
integration/docs after coherent component commits. This work is separate from
[TODO 11](../todo/11-simplify-internal-compatibility.md)'s compatibility cleanup and
[TODO 08](../todo/08-deployment-metadata-validation-reuse.md)'s deployment preparation
performance work. Coordinate with [completed plan 12](12-live-vm-additions.md) so live VM
additions remain usable and survive later boots.
