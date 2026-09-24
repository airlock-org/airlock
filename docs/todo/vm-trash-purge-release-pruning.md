# VM trash purge and release pruning

Status: physical restore-after-GC trial passed; final purge acceptance pending. Follow-up to
[completed VM retirement and restoration](../completed/vm-retirement-trash.md).

## Goal

Safely release obsolete machine generations and permanently delete eligible
trashed VMs. Recoverable retirement and restoration already work. Local code now
provides guarded release pruning and purge; live deployment/testing is the next step.

## Required work

- Add an explicit pruning plan and operation that identifies obsolete release
  roots and explains blockers. Protect the running and selected generations,
  retained rollback/recovery choices, unfinished transactions, and unexpired
  trash recovery requirements. Preserve audit records and make interruption
  recoverable; do not require manual deletion of roots.
- Keep purge explicit and limited to one confirmed trash record. Enforce its
  retention deadline (30 days by default), remaining release references, VM
  UUID, storage ownership, and stopped state. Expiry alone never deletes data.
- Permanently remove only the eligible VM's owned private resources. Release
  its pool retention safely; shared software still needed by other VMs,
  retained generations, or pinned builder views must remain available.
- Document pruning, recovery limitations, purge eligibility, and final outcomes.

## Acceptance

- Pytest covers protected-release refusal, retention, identity conflicts,
  interruption/resume, repeated operations, and purge audit outcomes. Use a
  controlled test clock for expiry cases; do not change the live machine clock
  or edit trash records to bypass retention.
- On a disposable VM, run supported pool cleanup while it is retired, then
  restore it and verify the same identity, private data, and required software.
- Verify purge refusal before expiry and while retained releases reference the
  VM. After expiry and supported pruning, explicitly purge the eligible record
  and verify only its owned resources disappear; other VMs and pinned builder
  views still work. Exercise shared pool data, not only unique objects.

The earlier `lab-retire` test VM was restored and halted on the previous USB
installation; that installation was replaced by a fresh image. The normal image
and release sources no longer declare this disposable VM. A temporary committed
trial release must introduce a disposable VM after image boot, so the recovery
image cannot retain it during the purge trial. Keep the trial commits available
through physical acceptance.

## Local implementation and next test

`release-list`, `release-prune-plan`, `release-retain`, and `release-prune` expose
protected releases and journal exact root/profile removals. Pruning preserves
audit records and blocks reuse of the old release. Purge checks actual volume
ownership and can resume after VM removal or pool GC. Pool retirement shares
the maintenance lock, retains an audit receipt, and prevents generation-ID reuse.
The sys-dev client starts destructive maintenance in detached dom0 workers.
Commands and limitations are in [machine deployment](../machine-deployment.md).

Local verification: component `make test-quick` passed 177 tests; parent
`make test-quick` passed 397 with 23 skipped. The standalone coordinator built,
and its packaged Python environment successfully imported the Qubes client.
Tests cover interrupted pruning/purge, exact targets, ownership conflicts,
retention, and shared objects surviving GC for a second VM or retained builder
generation. These are local results; no live prune or purge has been performed.

After the fresh image boots, create and deploy a committed trial release that
introduces a disposable VM. Create and check private-data markers, then deploy
another committed release that explicitly retires it and captures its volume
snapshot. Verify refusals before expiry, run the retired-VM
pool-cleanup/restore trial, then retire again for the purge trial. Use the
configured retention period (minimum one day); do not accelerate expiry by
changing the clock or record. Advance the rollback choice with a later committed
release, inspect and prune eligible references, and explicitly purge only the
test record.
Physical results must be recorded before marking this follow-up complete.

## Physical trial — 2026-09-24

The latest USB image booted with working GUI input and sys-dev at `sys-dev-1`.
The committed trial release `18ef40f` (parent `0626d3f`) introduced
`trash-probe` after image boot and set one-day retention. The first deployment
restarted affected guests, selected a host generation, and reached `complete`
after reboot. The disposable VM was UUID
`cf7c5bef-33b8-4d3f-959e-8ff117c5a2f6`, with private volume
`varlibqubes:appvms/trash-probe/private`. A marker in its private home had
SHA-256 `c283a6a303cd0dc7377fb53d810dc172c88527b8dab8dfa9401e9b8363e03a14`.

The committed retirement release `4b09749` (parent `d56b5e1`) removed the
VM and explicitly retired it. Its plan named the same UUID and pool generation
1. After applying and rebooting, `machine-verify` returned `complete` and
`trash-list` showed record `1790208942071199999-trash-probe` with expiry
2026-09-25 00:15:43 UTC. An immediate `trash-purge` refused with
`trash retention window has not expired`. `release-list` protected the source
release as the selected previous release and for unexpired trash.

While retired, `airlock-closure-store gc` returned `status: passed` and retained
`trash-probe` generation 1. Reapplying `18ef40f` restored the record; after
reboot, `machine-verify` returned `complete`, the UUID and private volume were
unchanged, and the marker's SHA-256 matched exactly. The VM was shut down after
the check. This validates restore after supported pool cleanup for this VM;
the command reported no removed generations or segments, so it does not prove
shared-segment reclamation in this physical run.

Remaining physical acceptance: retire the VM again, advance the rollback
choice with a later release, confirm protected prune/purge refusals, and after
the new record's actual one-day deadline prune eligible references and purge
that exact record. Check that other VMs and retained builder views still work.
Do not reuse the expired first record: restoration removed it.
