# Recoverable VM retirement and trash

Status: proposed.

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
Expiry only makes trash eligible for a separate explicit guarded purge; retained
deployment or rollback references protect it for longer. Purge may permanently
remove only unreferenced resources owned by the named trash record, never shared
pool objects still needed elsewhere.
Record the original name, VM identity, source deployment, retirement time,
expiry, storage disposition, and final purge outcome.

Do not silently trash unexpected domains. The plan must distinguish declared
retirements from unknown unmanaged domains, show the affected storage, and
require the same deployment authorization as other topology changes.

## Acceptance

- Pytest coverage exercises plan, dependency rejection, apply,
  interruption/resume, restore, conflicting restore, retention, and guarded
  purge behavior.
- Removing one test VM leaves it stopped and inaccessible but recoverable with
  its private data intact.
- Deployment-based restore and rollback recover the same VM identity and data,
  including across a Dom0 reboot and after pool GC; required software remains
  available.
- Retirement prevents ordinary startup and access before and after the host
  policy reboot, including interruption/resume of that transition.
- Expiry alone cannot purge data still referenced by a retained release; an
  eligible explicit purge permanently removes only the named trash record.
- A physical deployment retires and restores a disposable VM before this path
  is used to retire `sys-config-*`.
- Operator documentation explains active, trashed, restored, and purged states,
  including the retention window and recovery commands.

In a separate follow-up after this mechanism is verified, retire `sys-config-*`
through an ordinary committed machine release and record the disposition of
each role's persistent data in the `sys-ctrl` work.
