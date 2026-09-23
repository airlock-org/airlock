# VM trash purge and release pruning

Status: pending. Follow-up to [completed VM retirement and restoration](../completed/vm-retirement-trash.md).

## Goal

Safely release obsolete machine generations and permanently delete eligible
trashed VMs. Recoverable retirement and restoration already work; an initial
guarded `trash-purge ID --confirm ID` exists, but historical release roots remain
pinned and there is no supported release-pruning command.

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

The existing `lab-retire` test VM is currently restored and halted, with its
diagnostic markers removed. It has not been purged. Recheck its live identity
and state before any destructive trial.
