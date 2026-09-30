# 12: Apply VM additions without a host reboot

Status: **planned** (2026-09-30).

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

## Existing mechanisms and gaps

- The [coordinator](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/coordinator.py)
  already applies Qubes topology before selecting the host boot generation.
  It records the selected release durably and distinguishes `reboot-required`
  from a fully active host. Reuse this whole-machine release model.
- The [planner](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/planning.py)
  accepts new Nix-closure domains and orders their dependencies. The existing
  [pytest coverage](../../airlock-machine/tests/test_machine_deploy.py) includes
  VM addition followed by rollback into recoverable retirement.
- [Pool admission](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
  embeds an explicit target list. The
  [controller sender](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-guest-deploy.py)
  admits guest artifacts before submitting the new host release, so the current
  host cannot accept a previously unknown name merely because the next host
  generation allows it.
- GUI startup mostly uses Qubes GUI-VM tags, but
  [session readiness policy](../../airlock-machine/lifecycle/misc/modules/dom0.nix)
  names the two current personal VMs. Installed
  [desktop launchers](../../airlock-machine/lifecycle/misc/modules/sys-gui/base.nix)
  also name those VMs. New applications need live discovery or installation
  through a reusable interface.

These findings describe source inspection, not successful live acceptance.
Airlock is unreleased: implement the required interfaces in the release baseline.
A migration for previously released hosts or a two-deployment workaround is
outside this plan.

## Implementation

- [ ] Preserve Xen, qrexec, QubesDB, qubesd, libvirt, Qubes policy and the existing
  storage/convergence machinery. Nix and committed source remain authoritative.
- [ ] Define a bounded preparation interface that authorizes artifact admission
  for domains in a validated candidate, including names not yet selected.
  Bind preparation to candidate identity, caller, domain and artifact identity.
  Preparation must leave the VM inactive and must not grant arbitrary dom0
  command execution or turn the new application VM into a control authority.
- [ ] Make required per-VM permissions available during live activation through
  Qubes policy, with durable state, rollback and reload behavior. Any derived
  policy must agree with the configuration used after reboot.
- [ ] Reuse selected-release state and journalled setup for creation, private
  storage identity, explicit start, autostart and later boot convergence.
  Do not introduce a separate hand-maintained VM registry or configuration history.
- [ ] Ensure graphical readiness and application launching work before host
  reboot. Avoid making routine additions depend on rebuilding or restarting
  `sys-gui` merely to install another launcher; define the interface with TODO 13.
- [ ] Keep unaffected running VMs running. Report genuine dependency restarts
  in the plan, and reject unsupported host changes that prevent immediate use.
- [ ] Cover interruption and retry across admission, creation, permissions,
  selection and startup. An incomplete transaction must not appear successful
  or cause boot to silently restore the image's original topology.
- [ ] Preserve recoverable retirement when rolling back an addition. Reapplying
  the declaration must restore the same private storage; rollback must not
  silently delete user data or claim to undo application data migrations.

## Acceptance

- [ ] Add meaningful pytest coverage for unknown-name admission, inactive
  preparation, candidate/caller rejection, explicit start versus autostart,
  failure recovery, policy reconciliation and rollback/reapply.
- [ ] Run `make test-quick`; keep Nix evaluation/build checks opt-in. Record
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
