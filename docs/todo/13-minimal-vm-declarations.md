# 13: Add ordinary VMs through one small declaration

Status: **planned** (2026-09-30).

Adding an ordinary `usr-*` VM should require one small, readable Nix declaration.
Shared interfaces and functions should derive its guest system, Qubes descriptor,
deployment integration and optional application launchers. The declaration should
be understandable to a user reviewing the change before it takes effect.

[Completed plan 12](../completed/12-live-vm-additions.md) supplies immediate creation and use with host
activation deferred until a later reboot. Design the declaration alongside that
lifecycle, then remove the duplicated wiring here. Choosing reviewers, approval
rules and the review UI is deferred to a separate task.

## Declaration contract

An ordinary VM declaration should identify its name and applications, with clear
defaults or explicit choices for label, network access, GUI assignment, memory,
CPU count, private-disk size and autostart. Keep behavior visible: an omitted
network setting must not unexpectedly grant connectivity or control authority.

Use typed Nix options and shared builders. Keep escape hatches for guest modules
and specialized roles explicit; an ordinary application VM should not require
the author to understand root images, closure manifests or deployment internals.
Do not create a separate configuration language for this task.

## Current duplication

- [Default domain builders](../../airlock-machine/lifecycle/bootstrap/default-domains/default.nix)
  have a reusable `mkPersonalVmDomainEntry`, but it is internal and wrapped by
  named functions for each existing personal VM. Per-VM defaults and guest
  packages are split across additional files.
- [The component flake](../../airlock-machine/flake.nix) separately lists
  bootstrap domains, machine guest entries, the lab topology and named guest
  configuration outputs.
- [The lab profile](../../airlock-machine/lifecycle/image/profiles/usb-lab.nix)
  repeats admission targets, while
  [single-guest deployment](../../airlock-machine/lifecycle/runtime/tools/sys-ctrl-guest-deploy.py)
  has a hard-coded target set.
- [GUI launchers](../../airlock-machine/lifecycle/misc/modules/sys-gui/base.nix)
  embed guest names, and
  [readiness permissions](../../airlock-machine/lifecycle/misc/modules/dom0.nix)
  name existing personal VMs explicitly.

Not every profile needs every VM. Consolidation must preserve intentionally
minimal GUI and recovery configurations and their authority boundaries.

## Implementation

- [ ] Define one canonical collection of VM declarations and a typed ordinary
  application-VM interface. Choose its authored location in `airlock-machine`.
  Derive profile membership explicitly, without copying each declaration.
- [ ] Generalize the existing personal-VM builder and convert `usr-home` and
  `usr-browser` to the interface. Preserve their network isolation, independent
  private disks, resources, autostart choices and application behavior.
- [ ] Derive guest entries, materialized topology, named build outputs and image
  preparation inputs from the canonical collection. Preserve the parent image
  adapter's reuse of component definitions.
- [ ] Derive supported deployment targets and the bounded admission inputs
  defined by TODO 12. The active or validated candidate configuration must be
  authoritative; an arbitrary caller-supplied name is not authorization.
- [ ] Define application metadata once: package, desktop identity, target VM
  and any fixed RPC needed for activation. Generate or discover launchers and
  readiness permissions through TODO 12's live integration interface. New VM
  declarations must not become dependencies that force `sys-gui` to rebuild.
- [ ] Preserve scoped Qubes policy and explicit custom RPCs such as URL opening.
  Generated launchers must not gain a general shell or dom0 administration.
- [ ] Validate invalid names, missing dependencies, cycles, conflicting desktop
  identities and unsupported combinations with useful errors before activation.
- [ ] Document a short ordinary-VM example and the boundaries of shared defaults.
  Keep specialized hardware and system-VM configuration outside this first
  simplification; reuse existing Qubes architecture throughout.

## Acceptance

- [ ] Add an example ordinary VM by editing or adding one declaration file.
  Its guest build, topology, admission, individual update target and optional
  launcher must require no hand-edited infrastructure lists or new named builder.
- [ ] Inspect the generated configuration: the example's isolation, privileges,
  private storage and autostart behavior must follow its visible declaration.
- [ ] Cover meaningful validation and derivation behavior with pytest. Run
  `make test-quick`; use opt-in Nix checks where needed to verify generated outputs.
- [ ] Verify existing personal VMs and minimal/recovery profile membership retain
  their intended behavior. Adding an ordinary VM must not broaden control access.
- [ ] Exercise the example through TODO 12's physical live-addition, later reboot
  and rollback acceptance. Record results under `.airlock-logs/` and distinguish
  declaration simplification from proof of successful deployment.

Implementation belongs in `airlock-machine`. Commit coherent changes there and
update the parent pin and affected image/tests/docs here. Coordinate with
[TODO 11](11-simplify-internal-compatibility.md) on canonical interfaces, without
making unrelated compatibility cleanup a prerequisite.
