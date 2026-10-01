# 12: Apply VM additions without a host reboot

Status: **implementation in progress; physical acceptance pending** (2026-09-30).

A released Airlock machine must accept a new ordinary `usr-*` VM, create it
with persistent storage, and make it usable immediately. The same release can
select an updated host generation for the next boot. Neither a preparatory
host deployment nor a host reboot may be required before using the new VM.

This plan implements the lifecycle. [TODO 13](13-minimal-vm-declarations.md)
makes its declaration small and removes repeated wiring. Sketch the declaration
and lifecycle together, then implement this plan first. User review and approval
of changes are a separate future task; do not design that workflow here.

## Required behavior

1. Build and prepare a committed release containing the new VM declaration.
2. Admit its artifacts and validate its dependencies and required permissions.
3. Apply the VM configuration through Qubes and create its private disk.
4. Start it on explicit request, including when `autostart = false`. Graphical
   applications must be launchable through the existing GUI VM immediately.
5. Select the release's host generation for a later boot. Keep the current host
   running and report VM readiness separately from pending host activation.
6. On a later reboot, retain the VM's configuration, private data and autostart
   setting through the normal selected-release boot path.

Apply must not implicitly start every new VM. Autostart controls automatic
startup; an explicit start or application launch remains available immediately.

## Initial mechanisms and gaps

- The [coordinator](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/coordinator.py)
  already applies Qubes topology before selecting the host boot generation.
  It records the selected release durably and distinguishes `reboot-required`
  from a fully active host. Reuse this whole-machine release model.
- The [planner](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/planning.py)
  accepts new Nix-closure domains and orders their dependencies. The existing
  [pytest coverage](../../airlock-machine/tests/test_machine_deploy.py) includes
  VM addition followed by rollback into recoverable retirement.
- The original [pool admission](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
  embedded an explicit target list. The original
  [controller sender](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-guest-deploy.py)
  admitted guest artifacts before submitting the new host release, blocking a
  previously unknown name even when the next host generation allowed it.
- GUI startup mostly used Qubes GUI-VM tags, but
  [session readiness policy](../../airlock-machine/lifecycle/misc/modules/dom0.nix)
  named the two existing personal VMs. Installed
  [desktop launchers](../../airlock-machine/lifecycle/misc/modules/sys-gui/base.nix)
  also name those VMs. New applications need live discovery or installation
  through a reusable interface.

These initial findings describe source inspection, not successful live acceptance.
Airlock is unreleased: implement the required interfaces in the release baseline.
A migration for previously released hosts or a two-deployment workaround is
outside this plan.

## Implementation

- [x] Preserve Xen, qrexec, QubesDB, qubesd, libvirt, Qubes policy and the existing
  storage/convergence machinery. Nix and committed source remain authoritative.
- [x] Define a bounded preparation interface that authorizes artifact admission
  for domains in a validated candidate, including names not yet selected.
  Bind preparation to candidate identity, caller, domain and artifact identity.
  Preparation must leave the VM inactive and must not grant arbitrary dom0
  command execution or turn the new application VM into a control authority.
- [x] Make required per-VM permissions available during live activation through
  Qubes policy, with durable state, rollback and reload behavior. Any derived
  policy must agree with the configuration used after reboot.
- [x] Reuse selected-release state and journalled setup for creation, private
  storage identity, explicit start, autostart and later boot convergence.
  Do not introduce a separate hand-maintained VM registry or configuration history.
- [x] Ensure graphical readiness and application launching work before host
  reboot. Avoid making routine additions depend on rebuilding or restarting
  `sys-gui` merely to install another launcher; define the interface with TODO 13.
- [ ] Keep unaffected running VMs running. Report genuine dependency restarts
  in the plan, and reject unsupported host changes that prevent immediate use.
- [x] Cover interruption and retry across admission, creation, permissions,
  selection and startup. An incomplete transaction must not appear successful
  or cause boot to silently restore the image's original topology.
- [x] Preserve recoverable retirement when rolling back an addition. Reapplying
  the declaration must restore the same private storage; rollback must not
  silently delete user data or claim to undo application data migrations.

## Acceptance

- [x] Add meaningful pytest coverage for unknown-name admission, inactive
  preparation, candidate/caller rejection, explicit start versus autostart,
  failure recovery, policy reconciliation and rollback/reapply.
- [x] Run `make test-quick`; keep Nix evaluation/build checks opt-in. Record
  machine-readable results under `.airlock-logs/`.
- [ ] On the lab machine, apply an ordinary new `usr-*` VM through the normal
  [deployment workflow](../machine-deployment.md), with no preliminary host
  apply or reboot. Record the unchanged running host and newly selected host.
- [ ] Start the new VM and launch an application before any reboot. Stop/start
  it again and verify an exact sentinel in its persistent home. Confirm
  unaffected running VMs were not restarted just because a domain was added.
- [ ] Reboot later and verify the selected whole-machine generation, the VM's
  private storage identity and sentinel, and its declared autostart behavior.
- [ ] Verify rollback and reapply preserve the sentinel in recoverable storage.
  Record build, live activation, later reboot and rollback evidence separately.

Implementation belongs in `airlock-machine`; commit coherent component changes,
then update and commit the parent pin and integration/docs changes. Successful
builds alone do not complete this plan.

## Implementation checkpoint

Implementation was developed in
`/home/user/dev/airlock-org/worktrees/airlock-live-vm-additions`,
on parent and component branches `feat/live-vm-additions`, now merged and retired.

The controller now imports the host/transport closure through
`airlock.Ctrl.MachinePrepare` before sending guest payloads. The installed
coordinator validates the candidate and records a root-owned admission token.
`airlock.Pool.AdmitCandidate+TOKEN.DOMAIN` accepts only the prepared domain's
system, closure, root, kernel and source identity from sys-ctrl. Preparation
does not create a VM or change boot selection. Tokens are rejected after a
boot, selection or deployment-state change, or release pruning. Existing
static admission remains available for the existing individual update path.

Readiness policy now uses `@tag:guivm-sys-gui`. Upstream Qubes appmenu handling
is retained, with NixOS paths, the graphical user, and Airlock's session barrier
for generated launchers. In sys-gui, `airlock-sync-qube-appmenus VM` explicitly
starts the guest, waits for readiness, and imports its menus into the persistent
GUI user's profile. The declaration's existing `menu-items` feature selects
which desktop entries appear. This needs no per-VM GUI rebuild or restart.
TODO 13 will expose the declaration and derive the remaining duplicated lists.

Automated checks cover inactive candidate preparation, artifact substitution,
caller/stale-state rejection, retries, pool protocol output, explicit start
with autostart disabled, boot convergence and rollback/reapply storage identity.
`make test-quick` passed with **706 passed, 58 skipped**. Three targeted Nix tests
passed, building the menu package and checking the installed scripts and policy.
The coordinator package also built in the guarded Airlock store. Evidence is
under this worktree's `.airlock-logs/`: `final-quick/pytest-report.json`,
`live-vm-nix/report.json`, and `live-vm-nix/engine-build.log`.

Component commits: `9e78e25` (candidate admission), `48a981a` (live menus),
and `ae6dabb` (wire protocol regression coverage and lifecycle documentation).
Parent main includes these changes through `4989092` and pins the last component
commit. Component main also includes all three commits.

Rebased onto parent main `4581f17` and its component baseline `38886d6`.
After rebase, `make test-quick` passed with **724 passed, 59 skipped** and the
three targeted Nix interface tests passed. Reports are in
`.airlock-logs/rebase-quick/pytest-report.json` and
`.airlock-logs/rebase-nix/report.json`.

At the implementation checkpoint, no whole-machine release or physical deployment
had been performed. The no-reboot application launch, exact home sentinel, later host boot
and physical rollback/reapply checks remain open. Implementations checked above
are not a claim of physical acceptance; this document remains in `todo`.

Next, build a committed whole-machine candidate adding one ordinary VM using
the existing declaration wiring, then run the lab acceptance sequence above.
The larger declaration simplification belongs to TODO 13.

## Worktree retirement (2026-09-30)

Verified that the parent and component changes were fully on main and that both
working trees were clean. Removed the worktree and its local feature branches.
Continue from `/home/user/dev/airlock-org/airlock`.

Worktree-local evidence is preserved under
`.airlock-logs/retired-worktrees/airlock-live-vm-additions/` in the main checkout.
This archive is the current location for the earlier `.airlock-logs/` paths;
their relative suffixes are unchanged. Physical acceptance remains pending.

## Live baseline installation attempt (2026-09-30)

Reached the live development VM through Tailscale at `user@100.104.92.101` and
confirmed its Qubes name is sys-dev. Dom0 was running host
`dyy6c32zx6fanzfrxyzgriw4rchihhwy`, boot ID
`4513ac2c-980a-4cd7-8c35-27edb6a2e83b`. Neither `MachinePrepare` nor
`Pool.AdmitCandidate` was installed in that running host. This prerelease
baseline needs installation and an initial host boot before exercising the
released no-reboot addition interface. This installation is separate from the
ordinary VM-addition acceptance test.

Normal `make machine-plan` built component revision `ae6dabb` through sys-ctrl.
The plan changes sys-ctrl, sys-dev and sys-gui, restarting their running
usr-home/usr-browser dependents; sys-net and sys-firewall are unaffected.
Normal `make machine-apply` reused the prepared candidate with zero builds and
transfers and submitted detached worker `airlock-machine-deploy-5b42fb9f7eb7`.
Candidate: `/var/lib/airlock/machine/candidates/b03198349a6455711665c24f8e87e77646fe78b07641bc7761284b1fd88442ee`.

**Failure confirmed (2026-10-01):** inspection of the laptop USB recovered the
worker diagnostics for transaction `1790786804188630165`. The worker stopped
the affected guests and failed in `host-boot-selection`: `seal-boot` tried to
refresh a retained entry without source provenance and raised `ValueError`.
The compatibility cleanup had removed the previous formatter fallback.

Boot installation had already installed host `frp44hh7h9g5bv54khr4x5kys3mhgsnn`.
After the operator hard restarted, boot `647aff8d-4033-49a9-a2c6-13a5bb9db9c6`
validated pool generation 22 and assembled the store, then refused to restore
the bound failed transaction. The initrd entered emergency mode; the last
visible EROFS message was a successful mount, not the cause. Filesystem checking
reported clean after recovering the ext4 journal. The operator has since booted
the earlier `h8v0fz89xmdxjz5sh6ppnr7cyciv71j8` generation. Normal typed verification
reports transaction `1790781773748219122` **complete** and all six running guests
matching their selections; usr-browser is stopped. The new dom0 boot ID is
`dc910c03-fb3f-4bab-bae6-baa80e649d4f`.

Machine commit `e0f9081` preserves the bytes of checksum-verified retained
entries when their provenance is unavailable. New entry formatting remains
strict, and corruption still fails verification. Commit `0935cde` moves boot
installation and sealing ahead of guest shutdown, preserves the previous default
through upstream loader regeneration, and publishes the target host binding and
default only after guest verification. Preparation explicitly restores the
booted host as default so a failed newer entry left by manual recovery cannot
remain selected.
The quick suite passed **735 tests, 59 skipped**; targeted tests and lint passed.

Normal plan built revision `0935cde` in 235 s and prepared candidate
`f19c7f8b6de2c372d7de26a362087b20f6af4adf78405dd23456e931ada39c80` in 117 s.
Normal apply reused the candidate without builds or transfers. Worker
`airlock-machine-deploy-2d09764d26d0` exited with `Result=success`, status 0.
Typed verification reports transaction `1790847116434715227` **reboot-required**:
all six running guests match their selections, usr-browser is stopped, selected
host is `4v44y6607wp1c087dgkl8id5pj8cxnrw`, and running host is still
`h8v0fz89xmdxjz5sh6ppnr7cyciv71j8`. Boot ID is unchanged. The worker journal confirms
successful boot preparation before stopping guests, and final boot selection
after verification. This is a successful live deployment with host boot
verification still pending. The initial host boot also loads the new admission
RPCs, which are absent from the currently running host.

Tests cover installer failure and interrupted menu publication without changing
the prior default, boot-preparation failures without stopping guests or changing
machine selection, and guest-start failure without replacing the prior host
binding. Final binding, boot selection and transaction completion still require
separate writes; abrupt power loss in that window remains a recovery case.

No host reboot, live overlay, service override or diagnostic configuration
mutation was performed. Observation used the existing unprivileged development
dom0 PTY. Later USB inspection required operator-granted read ACLs on boot logs
and the failed transaction directory. Those diagnostic ACL changes remain;
deployment state was not edited. No new control authority was added.

The separate committed acceptance candidate is in
`/home/user/dev/airlock-org/worktrees/airlock-live-vm-acceptance`, on parent and
component branches `trial/live-vm-acceptance`. After rebasing onto the recovery
fixes, parent `23b310d` pins component `4e36175`. It adds an offline purple
`usr-notes` VM with Text Editor, a 2-GiB
private disk, sys-gui as its GUI VM and `autostart = false`. Evaluated topology
differs from the original baseline only in that domain. Original trial quick
tests passed **724 passed, 59 skipped**. The complete rebased release built locally:
`/nix/store/cv365ckhd9cvbnqc21lv78zf3kn4w81b-airlock-machine-release`.
This trial has not been submitted to the live machine or merged into main.

Evidence is in the main checkout's `.airlock-logs/live-vm-trial/`: baseline
`baseline-before.json`, `baseline-plan.log`, `baseline-plan.json`,
`baseline-apply.log` and `baseline-submission.json`; trial `trial-topology.json`,
`trial-topology-diff.json`, `trial-quick.log`, `trial-quick/pytest-report.json`,
`trial-build.log` and `trial-release.json`.

Boot-failure evidence, including raw initrd logs, the failure traceback and
transaction command output, is in `boot-hang-20261001/` below that directory.

Next: boot and verify the installed baseline.
After confirming the generic host RPCs and GUI tools are running, use normal
plan/apply from the trial worktree and perform the no-reboot acceptance sequence.
