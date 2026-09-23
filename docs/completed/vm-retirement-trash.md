# Recoverable VM retirement and trash

Status: complete for recoverable retirement and restoration, 2026-09-23.
Focused pytest coverage and live disposable-VM retirement, retired-state reboot,
and restore trials pass. Permanent purge, release pruning, and physical pool-GC
acceptance are tracked in [the follow-up TODO](../todo/vm-trash-purge-release-pruning.md).

## Goal

Allow an intentional machine configuration change to remove a VM from the
active topology without immediately destroying its private data. This is a
general machine-lifecycle capability, not a development-only exception. Prove it
with a disposable VM first; removing `sys-config-*` is a separate follow-up
before introducing `sys-ctrl`.

## Required behavior

When an approved release no longer declares a VM, the Dom0 machine coordinator
must plan an explicit retirement. Applying it must stop the VM, remove its
autostart and active policy/device exposure, and place its identity,
configuration metadata, and private volumes in a transaction-specific trash
record. The operation must be journalled and resumable like other deployment
operations.

Choose the trash representation after checking Qubes identity and storage
bindings. Do not assume registered domains can safely be renamed: the current
pool backend uses domain names to identify generations. Preserve those bindings
and pin the required pool generations and boot artifacts against GC, alongside
private data. Trashed domains are not part of the active topology and must not
be startable through ordinary policy.

Reject retirement if surviving VMs or machine defaults still depend on the
removed VM. Define and test how retirement is enforced both before and after
the host reboot that installs updated policy; the interim state must not allow
the trashed VM to restart or regain access.

Restore through a committed deployment that reintroduces the VM, or through
whole-machine rollback, keeping configuration and runtime topology consistent.
Restoration must fail safely on name, identity, storage, or topology conflicts.
Use a configurable minimum retention window, initially 30 days from retirement.
Expiry must not automatically delete trash. Permanent purge and removal of
retained deployment or rollback references belong to the follow-up TODO.
Record the original name, VM identity, source deployment, retirement time,
expiry, and storage disposition.

Do not silently trash unexpected domains. The plan must distinguish declared
retirements from unknown unmanaged domains, show the affected storage, and
require the same deployment authorization as other topology changes.

## Acceptance

- Pytest coverage exercises plan, dependency rejection, apply,
  interruption/resume, restore, conflicting restore, and retention.
- Removing one test VM leaves it stopped and inaccessible but recoverable with
  its private data intact.
- Pytest covers deployment-based restore and rollback preserving VM identity
  and storage. Physical deployment-based restore preserves identity and private
  data across a Dom0 reboot; required software remains available. Physical
  recovery after pool GC belongs to the follow-up TODO.
- Retirement prevents ordinary startup and access before and after the host
  policy reboot, including interruption/resume of that transition.
- A physical deployment retires and restores a disposable VM before this path
  is used to retire `sys-config-*`.
- Operator documentation explains active, trashed, and restored states,
  including the retention window and recovery commands.

In a separate follow-up after this mechanism is verified, retire `sys-config-*`
through an ordinary committed machine release and record the disposition of
each role's persistent data in the `sys-ctrl` work.

## Implementation

The release manifest now carries an exact `retireDomains` delta and a bounded
`trashRetentionDays` setting. The coordinator keeps retired domains registered
under their original names, stops them, disables autostart, tags them
`airlock-retired`, pins their selected pool generation, and writes durable trash
metadata. Static early qrexec rules deny calls from and to tagged domains. The
preflight also rejects references in VM preferences, machine defaults, and
topology-managed qrexec policy, including in later releases while the VM
remains in trash. Committed
deployment or rollback restores the same UUID and private storage identity.

`trash-list` exposes the audit records. Required software and private data are
retained for recovery. An initial guarded `trash-purge` implementation exists,
but its physical acceptance and safe release pruning remain follow-up work.

## Physical trial, 2026-09-23

On the USB `usb-lab` machine, `lab-retire` was started, given a marker in its
persistent home, shut down, and retired by committed release `d19c259`.
Dom0 recorded trash ID `1790159152093003635-lab-retire`, the original UUID
`8479f087-ea5f-4bb3-9fac-e3f2c89f6fa1`, private storage
`lab-retire-private`, pool generation 1, and 30-day retention. The VM remained
registered but halted, with autostart disabled and the `airlock-retired` tag.
A plan for committed diagnostic release `7815b2e` was rejected because
`usr-browser` referred to the retired VM; that release was not applied.

Committed release `a259e78` restored the same UUID, removed the retirement
tag, and booted the VM with the original home marker intact. The diagnostic
marker was removed and the VM was shut down. Both deployment workers finished
successfully. After reboot, `machine-verify` reported `complete`: dom0 ran the
selected restored generation, `lab-retire` remained halted with the same UUID,
and its retired tag was absent.

In a second trial, a new private-home marker was written, then committed
release `d19c259` retired `lab-retire` again under trash ID
`1790163352245734564-lab-retire`. After reboot, `machine-verify` reported
`complete` with the retired release as both selected and running host
generation. The VM remained registered, halted, tagged `airlock-retired`,
and non-autostarting with the same UUID. Dom0's policy evaluator attributed
denials in both directions to `05-airlock-retired.policy`. Release `a259e78`
then restored the same VM; its marker survived, was removed, and the VM was
shut down. After the final reboot, `machine-verify` reported `complete` with
the restored release as both selected and running host generation. The VM was
still halted with the same UUID and no retired tag; `trash_active` was empty.
Pool-GC was not run, and no purge was attempted; retained release roots
currently prevent an eligible purge.
