# VM trash purge and release pruning

Status: complete for explicit VM purge and release pruning, 2026-09-24.
Follow-up to [recoverable VM retirement and restoration](vm-retirement-trash.md).

## Goal

Safely release obsolete machine generations and permanently delete eligible
trashed VMs. Recoverable retirement and restoration already work. Guarded release
pruning and purge passed local and physical trials, including early purge.

## Required work

- Add an explicit pruning plan and operation that identifies obsolete release
  roots and explains blockers. Protect the running and selected generations,
  retained rollback/recovery choices, unfinished transactions, and unexpired
  trash recovery requirements unless the operator explicitly waives the deadline
  for the exact trash record. Preserve audit records and make interruption
  recoverable; do not require manual deletion of roots.
- Keep purge explicit and limited to one confirmed trash record. Enforce its
  retention deadline (30 days by default) unless the operator explicitly requests
  early purge. Always enforce remaining release references, VM
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
- Verify purge refusal before expiry without an override and while retained
  releases reference the VM even with an override. Use an exact-record override
  to prune and purge before expiry, then verify only its owned resources
  disappear; other VMs and pinned builder
  views still work. Exercise shared pool data, not only unique objects.

The earlier `lab-retire` test VM was restored and halted on the previous USB
installation; that installation was replaced by a fresh image. The normal image
and release sources do not declare the disposable VM. Committed trial releases
introduced `trash-probe` after image boot, so the recovery image never retained it.

## Implementation and local verification

`release-list`, `release-prune-plan`, `release-retain`, and `release-prune` expose
protected releases and journal exact root/profile removals. Pruning preserves
audit records and blocks reuse of the old release. Purge checks actual volume
ownership and can resume after VM removal or pool GC. Pool retirement shares
the maintenance lock, retains an audit receipt, and prevents generation-ID reuse.
The sys-dev client starts destructive maintenance in detached dom0 workers.
Commands and limitations are in [machine deployment](../machine-deployment.md).

Local verification: component `make test-quick` passed 400 tests with 21
skipped after the early-purge change. The earlier parent `make test-quick`
passed 397 with 23 skipped. The standalone coordinator built, and its packaged
Python environment successfully imported the Qubes client. Tests cover
interrupted pruning/purge, exact targets, ownership conflicts, retention,
early overrides, and shared objects surviving GC for a second VM or retained
builder generation. Focused checks passed 20 prune/purge cases and 5
maintenance-client cases.

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

The VM was shut down and retired again with `4b09749`. After reboot,
`machine-verify` returned `complete` with active trash record
`1790210515141062431-trash-probe` and expiry 2026-09-25
00:41:57.140376 UTC (01:41:57 BST). Its UUID and private volume still matched
the first trial.
An immediate prune of source release
`/nix/store/7n1pvjahxklmmc5h5wamf3vcsjhk3r03-airlock-machine-release`
refused because it was the running host, the selected previous release, and a
recovery requirement for unexpired trash. Purge refused on retention.

The later committed release `11a7f00` (parent `7dce3d5`) kept the VM absent
and cleared `retireDomains`. Its plan affected no guests and required no
reboot. After apply, `machine-verify` returned `complete`; the retirement
release became the immediate rollback choice. `release-prune-plan` then listed
only the unexpired trash record as a blocker for the source release. A temporary
explicit retain added `explicitly retained rollback choice` to the blockers;
removing that retain restored the single unexpired-trash blocker. `qvm-ls`
showed `trash-probe` halted, other expected VMs running, and the retained
`sys-dev-builder` generation-1 manifest was present.

The early-purge coordinator change `28d0578` (parent `5893585`) was applied as
release `/nix/store/w1n83vx6hhsaigpzgv9wnykygwc7z64i-airlock-machine-release`.
After reboot, `machine-verify` returned `complete` with matching host and guest
generations. The active trash record still had the original VM UUID and a real
deadline of 2026-09-25 00:41:57.140376 UTC.

With `--allow-unexpired-trash 1790210515141062431-trash-probe`, the source
release's prune plan reported no blockers and exactly one waived blocker. It
named roots `1790208418722591516`, `1790209845672888216`, and the active
trash root, plus Nix boot generations 2 and 4. The confirmed prune removed
those targets and recorded `status: pruned`. No other release was pruned.

The exact-record `trash-purge --before-expiry` completed at 2026-09-24
01:36:02 UTC, preserving `purged_before_expiry: true` in its audit record.
Repeating purge without the flag returned the same purged record. `trash-probe`
was absent from `qvm-ls` and `qvm-volume list`; its private Qubes volume was
gone. The older restored trash record remained audit-only. The pool generation's
retention flag became false and its current link was absent.

Supported pool GC returned `status: passed`, with no removed generations,
views, or segments under its default two-generation history policy. This run
therefore proves purge released the VM's retention, but it does not prove
physical reclamation of shared segments. `trash-probe` and `usr-browser`
generation 2 had the same guest system, closure hash, and all 827 store paths.
After GC, `usr-browser`'s store view remained present; sys-dev's pinned builder
generation 1 remained mounted and an existing `coreutils` executable and Nix
path-info query worked through it. `machine-verify` still returned `complete`:
sys-dev, sys-gui, and usr-home matched their selected generations;
usr-browser remained halted as configured. No machine clock or trash metadata
was changed to advance expiry.
