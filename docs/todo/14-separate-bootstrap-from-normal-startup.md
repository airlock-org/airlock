# 14: Separate bootstrap from normal startup

Status: **planned** (2026-09-30).

An unchanged, previously initialized machine should reach a usable sys-gui login
without repeating full topology setup. Reuse the configuration already applied
through Qubes, perform the checks needed for this boot's GUI and input startup,
and reserve detailed bootstrap work and presentation for initialization,
configuration changes and recovery.

This concerns the pre-GUI **Airlock | Startup** console, not the
[first-run setup wizard](first-run-setup-wizard.md). Hiding the console alone
would leave the underlying delay in place.

## Current behavior and evidence

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

- [ ] Measure unchanged boots before altering startup. Attribute time to topology
  queries, storage validation, hardware guards, GPU handoff and guest startup.
  Record time to usable GUI/input separately from all autostart guests being ready.
- [ ] Separate configuration application from ordinary boot. Reuse existing
  deployment journals and selected-release state where practical. If a completion
  record is needed, bind it to the exact applied configuration and durable
  successful completion; a permanent initialized flag is insufficient.
- [ ] Define invalidation and recovery for changes, rollback, failed or interrupted
  application, missing VMs/volumes and inconsistent state. Preserve selected
  guest generations and private storage; never silently restore the seed image's
  original topology.
- [ ] Define the actual prerequisites for a usable GUI, including sys-gui even
  though its ordinary Qubes autostart is disabled. Remove the full-topology
  dependency only once those prerequisites are enforced independently.
- [ ] Keep required checks of current runtime state: Qubes service readiness,
  GPU ownership/handoff, input initialization and receiver readiness, and the
  started guest's expected system identity. Preserve artifact integrity and
  volume isolation; distinguish essential validation from redundant work.
- [ ] Keep unrelated application and builder VM preparation outside the login
  dependency chain. Preserve declared autostart behavior and real dependencies,
  and report background failures without claiming that startup fully succeeded.
- [ ] Use a brief startup indication on ordinary boots. Retain detailed progress
  for initialization, changes and repair, with actionable diagnostics for delays
  and failures. Display checking/reuse accurately instead of implying copying
  or provisioning when nothing changes.

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
- [ ] Add meaningful pytest coverage for boot-path selection, completion/state
  invalidation, prerequisites and recovery. Run `make test-quick`; keep Nix
  evaluation/build checks opt-in through `make test-nix`.
- [ ] Follow [machine-deployment.md](../machine-deployment.md) for implementation:
  commit source, use normal plan/apply, and verify the whole-machine generation.
  Record before/after physical timings under `.airlock-logs/`, separating
  usable GUI/input from background readiness and build/test success from live
  acceptance.

Implementation belongs in `airlock-machine`; update the parent pin and affected
integration/docs after coherent component commits. This work is separate from
[TODO 11](11-simplify-internal-compatibility.md)'s compatibility cleanup and
[TODO 08](08-deployment-metadata-validation-reuse.md)'s deployment preparation
performance work. Coordinate with [TODO 12](12-live-vm-additions.md) so live VM
additions remain usable and survive later boots.
