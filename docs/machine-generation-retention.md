# Machine generation retention and background GC

Status: implemented locally; not built, deployed or accepted in the live lab yet.
No tests were run at the operator's request.

## Retention policy

Count verified whole-machine snapshots, not individual VM generations or
candidate/transport records. Equivalent records share a machine-generation
identity based on the host, setup implementation and complete Qubes topology.

Keep:

- The newest two successfully verified machine generations.
- The closest available verified generation at least seven days old.
- A newer successor while it ages into the next weekly rollback choice.

The successor permits up to four policy-retained generations during rotation.
Without it, frequent updates would discard every intermediate snapshot before
it became a week old, leaving the original weekly rollback indefinitely. Until
a week-old snapshot exists, keep the oldest verified snapshot as the anchor.

Existing protections are additional: selected and running generations, the
immediate rollback, recovery profiles, explicit operator retains, unfinished
deployments and unexpired trash. Do not override these to enforce a numerical
limit. Prepared/unverified candidates are not treated as verified generations;
their cancellation/expiry is a [separate task](todo/undeployed-candidate-retention.md).

Verification age is preserved across repeated verification and rollback. For
historical completed transactions, import their existing verification timestamp;
if absent, conservatively use the completed status file's modification time.
Only complete transactions whose recorded running host matches their snapshot
enter this history. Reboot-required or failed transactions do not.

## When cleanup runs

After a successful apply that needs no reboot, queue cleanup. If reboot is
required, wait until guarded boot startup verifies the selected host and guest
systems. Explicit whole-machine verification also queues any outstanding cleanup.
This uses the existing system-generation verification criteria; it is not a
new physical GUI or hardware acceptance test.

A root-owned systemd path watches the durable pending marker and activates
`airlock-generation-maintenance.service`. The worker runs in the background at
reduced CPU/I/O priority. It holds the deployment lock across policy pruning and
pool GC, preventing concurrent updates or another cleanup from racing it.
Subsequent deployment operations may wait for that lock; the successful update
does not synchronously run GC itself.

The worker uses the existing journalled pruning mechanism and preserves all
its blockers. It releases default admission retains only for storage generations
referenced by pruned machine snapshots and no surviving snapshot. Legacy
snapshots lacking exact generation maps protect matching system versions.
Explicit/retirement retention updates are preserved. It then invokes existing
closure-store GC, keeping the newest two storage generations per domain in
addition to current, running, retained and in-progress data. Shared objects remain
protected. It never garbage-collects the host Nix store or deletes VM private disks.

Cleanup failures retain the pending request and a retryable journal. A retry
finishes its own interrupted prune before continuing. Successful cleanup removes
the pending marker and records the cleaned identity, so repeated verification of
that same selected snapshot does not continually trigger another GC.

## Configuration and observation

`airlock.machineDeployment.generationRetention.enable` defaults to true for
machines with persistent deployment enabled. Set it false to disable automatic
policy protection and scheduling; this does not itself remove retained artifacts.

Dom0 administrator commands (local code; available after deployment):

```sh
airlock-machine-deploy generation-retention-plan
systemctl status airlock-generation-maintenance.service
journalctl -u airlock-generation-maintenance.service
```

The preview lists protected snapshots, proposed pruning targets and unverified
records. It does not prune. These commands add no new production qrexec authority.
Existing internal `release-*` names remain until the
[terminology migration](todo/machine-generation-terminology.md).

Durable state is under `/var/lib/airlock/machine/maintenance/`: verified history,
the pending request, latest cleanup journal and last cleaned identity. The journal
includes pruning results, storage-retention changes, GC output and observed
available space before/after. Filesystem changes from other live processes can
affect the observed space difference.

## Validation still required

Regression coverage is written for selection, weekly rotation under frequent
updates, snapshot deduplication, reboot-required deferral, stable verification
age, preservation of shared/manual storage retains and retry after failed GC.
It has not been run. Python and Nix syntax parsing passed; no release was built.
Before live acceptance, review the retention preview and verify post-reboot
queueing, protected rollback choices, failure recovery and before/after space.
