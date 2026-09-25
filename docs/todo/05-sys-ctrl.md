# Move configuration work into sys-ctrl

Status: controller migration remains in progress. The first `sys-ctrl` guest
bootstrap is merged on main and its guest system evaluates locally, but it has
not been tested on the physical machine. Direct pool admission remains
physically smoke-tested only from `sys-dev`.

Local implementation update (2026-09-25): `sys-ctrl` now has the same pinned,
pool-backed builder layout as `sys-dev`. A typed qrexec guest request sends a
committed `airlock-machine` source bundle from `sys-dev` to `sys-ctrl`; the
controller imports it, builds with pinned offline inputs, admits the guest
closure, and asks dom0's constrained guest plan/apply service to act on the
receipt. The USB profile changes the pool admission caller to `sys-ctrl`.
The normal whole-machine `build`/`plan`/`apply` commands now also send that
source to `sys-ctrl`. The controller builds the release and streams its closure
to a restricted dom0 receiver for provenance checking and journalled
plan/apply. These changes are not yet physically tested. Whole-machine updates
still import guest packages into dom0's ordinary store; status/recovery
commands still use the development shell. Direct whole-machine pool admission,
candidate history migration, and `sys-config-*` retirement remain outstanding.

Since the original September 17 plan, the [pool-backed builder](../sys-dev-build-store.md),
[guest activation/rollback](../completed/02-sys-dev-guest-activation-rollback.md),
and [sys-dev whole-machine deployment](../completed/03-sys-dev-machine-deployment.md)
have been implemented and physically accepted within their stated boundaries.
Reuse these capabilities; they do not implement sys-ctrl or eliminate the
whole-machine path's imports into dom0's ordinary Nix store.

Repository boundary established 2026-09-17: deployable configuration now lives
in the standalone `airlock-machine` repository. The parent `airlock` repository pins it as the `airlock-machine/`
Git submodule and owns USB/lab integration. The future controller will check out
`airlock-machine` directly rather than treating the development repository or USB
image source as its configuration repository.

This document is an implementation handoff. Creating this document does not
change the running machine. Read [machine-deployment.md](../machine-deployment.md)
before implementing or deploying, and follow the repository's `AGENTS.md`.

## Goal and motivation

Make dom0 do less by introducing one `sys-ctrl` VM to own configuration
processing, release preparation, and the main writable Nix store. Consolidate
the responsibilities of `sys-config-staging`, `sys-config-validator`, and
`sys-config-vault` into it for the POC. `sys-dev` talks to `sys-ctrl` for normal
deployment work.

Keep the shared guest package pool at `/var/lib/airlock/closure-store` in dom0.
Keep Qubes control authority and privileged activation in dom0. Preserve Xen,
qrexec, QubesDB, qubesd, libvirt, and Qubes policy; this is a redistribution of
Airlock work around those mechanisms, not a replacement architecture.

The motivation is to remove Nix evaluation, builds, and candidate processing
from dom0's responsibilities, simplify the currently underused config-VM
workflow, and avoid retaining duplicate guest package payloads on dom0. Keeping
the existing storage backend avoids a separate storage-domain migration and
means running guests do not depend on `sys-ctrl` remaining available.

`sys-ctrl` will be highly trusted: it prepares executable systems and deployment
requests. Consolidation gives up isolation between editing, validation, and
archival. Moving this work does not make a compromised controller harmless.

## Current behavior and the duplication problem

The current `make machine-*` flow builds the release from the exact
`airlock-machine` commit pinned at `airlock/airlock-machine`, records both repository
revisions, transfers artifacts through `sys-dev`, and uses its development-only
dom0 administrative access to run the machine coordinator. It bypasses the
separate `sys-config-*` candidate workflow. Do not describe those VMs as the
current authority for machine deployments.

The narrower [sys-dev pool admission step](../completed/sys-dev-pool-admission.md)
is now implemented. `sys-dev` has a private writable Nix store and
`airlock-pool-send`, which streams `airlock.pool-artifact.v1` artifacts through
`airlock.Pool.Admit+TARGET`. The dom0 receiver validates artifacts and closure
completeness, reuses pool objects, and publishes retained, inactive generations.
The physical image permits `sys-dev` to submit for `sys-gui`, `sys-dev`,
`usr-home`, and `usr-browser`.

The receiver uses a narrowly permitted sudo helper to access the root-owned
pool, with caller/target checks in the handler and helper. Qubes policy remains
the remote caller authority; the helper's caller argument is not independent
proof of caller identity. A policy `user=root` option did not elevate this dom0
service in the live test and is not the implemented fix.

After rebuilding and flashing the fix, a tiny synthetic object was admitted
through qrexec as retained generation 2 while generation 1 remained current
(`activated=false`). The quick suite passed 329 tests, with 23 skipped.
That original trial established admission only. Later guest activation and
rollback evidence is linked above. It does not establish that the separate
nested `vm-pool-admission-flow` was executed.

There is no `sys-ctrl` in the tested physical topology: it contains `sys-dev`,
`sys-gui`, `usr-home`, and `usr-browser`. The host `make machine-*` workflow remains available, and releases can now
also be built/submitted directly from sys-dev through the narrow deployment
interface. The whole-machine release path still imports missing closures into
dom0's ordinary store; direct guest admission does not yet eliminate that
duplication for whole-machine updates.

There are two distinct stores on dom0:

1. The ordinary `/nix/store`, into which machine deployment imports Nix closures.
2. `/var/lib/airlock/closure-store`, a shared package pool serving filtered,
   read-only guest `/nix/store` views.

The existing machine deployment admission path reads guest packages from the first store and writes their
contents into the second representation. The imported source closures remain
retained, so guest package payloads can occupy space in both. The pool shares
objects across guests; this is not a full additional package copy for every VM.
Prepared USB images already omit guest-only source packages from the ordinary
dom0 store, but machine updates use ordinary closure imports.

Direct artifact admission now provides the pool-side capability needed to avoid
this duplication. The remaining work is to integrate it into controller-prepared
releases and machine deployment without requiring a retained guest-only source
closure in dom0's ordinary `/nix/store`.

**Removing that required duplicate copy is part of this goal, not an optional
future optimization.** Temporary transfer/staging space is acceptable. Packages
needed by dom0 itself may legitimately appear in its store as well as in the
guest pool; eliminating that overlap is not required.

## Agreed ownership

| Responsibility | Owner |
| --- | --- |
| Deployed configuration source, candidate history and validation | `sys-ctrl` |
| Nix evaluation, release builds/imports, main writable package store | `sys-ctrl` |
| Release preparation and proposed deployment plan | `sys-ctrl` |
| Shared guest package pool, admission, GC and compaction | Dom0 |
| Filtered guest storage views and volume attachments | Dom0 |
| Authoritative Qubes lifecycle, device and policy enforcement | Dom0 |
| Privileged activation, actual deployment state and recovery records | Dom0 |
| Dom0 boot/recovery system and independently bootable controller system | Available without a running `sys-ctrl` |

The intended request path is:

```text
development repository / sys-dev
  -> sys-ctrl: receive candidate, validate, prepare release and artifacts
  -> dom0: check request, admit artifacts, activate through Qubes, record result
  -> sys-ctrl: record the observed deployment outcome
```

Use qrexec and Qubes policy for inter-domain requests. A controller proposal must
not override dom0's checks of actual machine state. Keep Nix as the configuration
source of truth and retain existing Qubes runtime state where required.

Implement OS changes in `airlock-machine` and commit them there first; use
the parent `airlock` repository for USB, lab, and cross-repository integration changes and its
pinned submodule update. Specify how an approved `airlock-machine` commit reaches
the persistent controller repository, preserving its source revision and
lock-file provenance. Do not create an independently maintained configuration
on the live machine.

## Implementation stages

Bootstrap history (2026-09-24): the machine topology declared an offline,
autostarting `sys-ctrl` with a persistent private volume and a writable Nix
store. USB assembly supplies a direct `airlock-machine` checkout from the same
revision- and lock-checked source archive as `sys-dev`; the source-seeding
module is shared between the two guests. A standalone machine release does
not yet supply that seed, so existing installations need a source import path
before the controller could own deployments. The existing guest construction
and pool sender were reused. The 2026-09-25 local changes described above add
the pool-backed builder, controller services, and normal sys-dev submission
route. No candidate history has been migrated and the `sys-config-*`
declarations remain.

### 1. Specify boundaries and bootstrap sys-ctrl

- Reuse existing guest construction and upstream Qubes facilities. Give
  `sys-ctrl` persistent configuration/history and writable Nix storage separate
  from its immutable running system store.
- Define typed/versioned deployment requests and responses, permitted callers,
  artifact validation, target restrictions, and failure behavior. Do not expose
  an arbitrary dom0 shell as the normal controller interface.
  Reuse the existing artifact schema and admission response; deployment and
  activation requests still need their own contract.
- Decide controller network/fetch access explicitly; do not inherit `sys-dev`'s
  physical NIC or networking role merely because it is the submission endpoint.
- Ensure dom0 can boot, start `sys-ctrl`, and recover without asking a running
  controller for its own executable system. Dom0 still needs its own store.

### 2. Move configuration and release preparation

- Route normal deployment submissions from `sys-dev` to `sys-ctrl`. The
  controller's candidate repository is a direct `airlock-machine` checkout; it does
  not require a parent `airlock` checkout or USB assembly sources.
- Move evaluation, validation, builds/imports, candidate archives, and release
  preparation into `sys-ctrl`; reuse existing implementations where practical.
  Move the existing pool sender and private writable store setup into the
  controller role. Change the admission policy and configured caller to
  `sys-ctrl`, with explicit guest targets, when that path is ready.
- Retain dom0-side checks and the existing journalled whole-machine activation,
  dependency ordering, persistent guest data handling, and generation checks.
- Preserve plan/apply/status/verify/resume/rollback capabilities and provenance.
  Update the `make machine-*` entry points and documentation coherently.
- Record deployment success only after observing dom0's outcome. Preserve the
  distinction between a prepared release, guest activation, and a host reboot
  into the selected generation.
- The existing import-to-dom0 admission path may be an intermediate checkpoint.
  Do not mark the overall goal complete at this point.

### 3. Integrate direct pool admission into whole-machine deployment

Tracked in [Direct pool admission for whole-machine updates](06-direct-pool-machine-deployment.md).
Completing that TODO remains required for this controller migration.

### 4. Retire sys-config-* from the active topology

- Preserve needed candidate history, baseline/current-release records, and
  rollback information in `sys-ctrl` before retiring the old roles.
- Remove their active declarations and obsolete policy/service wiring once the
  replacement path works. Retain useful implementation code and tests where
  they can be reused.
- Handle existing installations explicitly. The current machine coordinator
  rejects domain deletion and some storage/topology changes; simply deleting
  Nix declarations is not a migration. Provide a reviewed migration path and
  retain old private volumes until their data is safely accounted for.
- Replace normal dependence on `sys-dev`'s unrestricted dom0 administrative
  transport. Any retained development/recovery access must be documented as
  separate from normal deployment authority.

## Completion criteria and evidence

All of these are required; record results and remaining gaps against this list.

- [ ] Cold boot starts the required machine topology, including `sys-ctrl`,
  without a controller/store boot dependency cycle.
- [ ] A committed guest configuration change submitted through `sys-dev` is
  prepared by `sys-ctrl` and applied through the constrained dom0 interface.
- [ ] Normal deployment performs no Nix evaluation or builds in dom0.
- [ ] A guest update reaches the shared pool without retaining its guest-only
  source closure in dom0's ordinary `/nix/store`. Evidence includes store
  references and allocated disk usage, not just a successful boot.
- [ ] Reboot and rollback to a retained prior release work with direct admission;
  guest private/home data survive.
- [ ] Stopping or restarting `sys-ctrl` leaves running guests' storage usable.
- [ ] A controller update, failed controller update, and recovery have tested
  procedures that do not depend on the unavailable controller.
- [ ] Unauthorized/malformed requests, invalid or incomplete artifacts,
  interrupted transfer/admission, and retry fail safely or recover as designed.
- [ ] Retention/GC preserve running and rollback generations after multiple
  deployments; temporary artifacts do not grow without bound.
- [ ] Host updates preserve the existing reboot-required/complete distinction
  and whole-machine verification.
- [ ] `sys-config-*` are no longer required by the active deployment path, and
  existing history/private data have an explicit migration disposition.
- [ ] Operator docs explain the new path, recovery, authority, and store layout.

Use pytest for new tests and `make test-quick` as the primary feedback loop.
Use relevant existing integration tests before physical deployment. Capture
machine-readable evidence under `.airlock-logs/`. Report source committed,
release built, guests verified, and host verified separately; local tests alone
do not prove a live deployment. Commit coherent changes and use the documented
`machine-plan`/`machine-apply` workflow for the lab, adapting it in-repository as
part of this change. Use only guarded Airlock GC targets during build loops;
never collect the development host's `/nix/store` or manually delete live pool
segments to make space.

## Starting points

- [Completed sys-dev admission step and validation scope](../completed/sys-dev-pool-admission.md)
- [Admission policy, handler, and root helper](../../airlock-machine/lifecycle/misc/modules/pool-admission.nix)
- [Artifact sender and receiver CLI](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/artifact_cli.py)
- [Artifact validation and pool admission](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/artifacts.py)
- [Machine deployment contract and current limitations](../machine-deployment.md)
- [Config VMs, storage format, admission and bootstrap](../config-system.md)
- [Repository boundary and workflow](../repositories.md)
- [Development-side machine transport](../../lifecycle/misc/tools/scripts/machine-deploy.py)
- [Dom0 machine coordinator](../../airlock-machine/lifecycle/runtime/tools/machine-deploy.py)
- [Machine boot integration](../../airlock-machine/lifecycle/misc/modules/machine-deployment.nix)
- [Release construction](../../airlock-machine/lifecycle/misc/modules/machine-release.nix)
- [Prepared guest store](../../lifecycle/image/prepared-guest-store.nix)
- [Storage engine](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/)
- [Bootstrap config system](../../airlock-machine/lifecycle/bootstrap/config-system.nix)
- [Initial runtime configuration](../../airlock-machine/lifecycle/bootstrap/initial-runtime-config/flake.nix)
- [Machine deployment tests](../../airlock-machine/tests/test_machine_deploy.py)

## Outside this goal

Moving the shared closure pool into `sys-ctrl`, introducing a new Xen storage
backend, making dom0 boot from controller-served storage, replacing Qubes control
mechanisms, and full GUI completion are outside this goal. Deployment request
schemas, controller network access, and migration mechanics remain implementation
decisions. The artifact transport format and admission service already exist;
reuse them and version any necessary extensions.
