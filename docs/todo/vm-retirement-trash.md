# Recoverable VM retirement and trash

Status: proposed.

## Goal

Allow an intentional machine configuration change to remove a VM from the
active topology without immediately destroying its private data. This is a
general machine-lifecycle capability, not a development-only exception. Use it
to retire the obsolete `sys-config-*` roles before introducing `sys-ctrl`.

## Required behavior

When an approved release no longer declares a VM, the Dom0 machine coordinator
must plan an explicit retirement. Applying it must stop the VM, remove its
autostart and active policy/device exposure, and place its identity,
configuration metadata, and private volumes in a transaction-specific trash
record. The operation must be journalled and resumable like other deployment
operations.

Prefer representing trash initially as disabled, clearly renamed Qubes domains
with their volumes still registered, if physical testing confirms that this
preserves identity and makes restoration reliable. Trashed domains are not part
of the active topology and must not be startable through ordinary policy.

Rollback or an explicit restore must recover a trashed VM within its retention
window. Restoration must fail safely on name, identity, storage, or topology
conflicts. A separate guarded purge operation may permanently remove expired
trash only when no retained deployment or rollback generation references it.
Record the original name, VM identity, source deployment, retirement time,
expiry, storage disposition, and final purge outcome.

Do not silently trash unexpected domains. The plan must distinguish declared
retirements from unknown unmanaged domains, show the affected storage, and
require the same deployment authorization as other topology changes.

## Acceptance

- Pytest coverage exercises plan, apply, interruption/resume, restore,
  conflicting restore, retention, and guarded purge behavior.
- Removing one test VM leaves it stopped and inaccessible but recoverable with
  its private data intact.
- Rollback restores the same VM identity and data, including across a Dom0
  reboot.
- Expiry alone cannot purge data still referenced by a retained release; an
  eligible explicit purge permanently removes only the named trash record.
- A physical deployment retires and restores a disposable VM before this path
  is used to retire `sys-config-*`.
- Operator documentation explains active, trashed, restored, and purged states,
  including the retention window and recovery commands.

After this mechanism is verified, retire `sys-config-*` through an ordinary
committed machine release and record the disposition of each role's persistent
data in the `sys-ctrl` work.
