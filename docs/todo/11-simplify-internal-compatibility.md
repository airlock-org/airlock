# 11: Simplify internal compatibility before release

Status: **in progress** (2026-09-30). Listed authored option aliases and dom0 module interface
implemented locally; further cleanup and validation are pending.

Airlock is unreleased. The local source contains a meaningful amount of
compatibility code, concentrated in configuration parsing, storage and deployment.
Standardize on one current internal interface and storage format where practical,
while preserving the existing Qubes architecture and required upstream integration.

This document records the read-only investigation of the parent repository and
its `airlock-machine` checkout. It identifies cleanup candidates, not proof that
every candidate can already be deleted. No live-state inventory, build, test or
deployment was performed during the investigation. Counts describe confirmed
examples rather than an exhaustive count of compatibility branches.

## Findings and implementation

### 1. Equivalent configuration option names

The initial inventory found at least nine pairs of equivalent names. Several
branches validate both spellings, reject conflicting values, or accept duplicate
values when they agree. These branches repeat across templates and domain
backends. The table records that inventory; completed removals are tracked below.

| Equivalent names | Location |
| --- | --- |
| `build` / `attr` | `expectFlakeOutputRef` in both libraries; `sourceRefType` in the topology module |
| `sourceRef` / `source_ref` | Source normalization in both libraries, including template normalization |
| `activationCommand` / `activation_command` | `normalizeProfile` in the system library |
| `activationAction` / `activation_action` | `normalizeProfile` in the system library |
| `switchAction` / `switch_action` | `normalizeHostSystem` in the system library |
| `skipStart` / `skip_start` | Template normalization in the topology library |
| `readOnly` / `read_only` | Store and root normalization in the topology library |
| `persistentSize` / `persistent_size` | Persistent-state normalization in the topology library |
| `qrexecPolicy` / `qrexec_policy` | Top-level normalization in the topology library |

Sources: [system.nix](../../airlock-machine/lifecycle/misc/lib/system.nix),
[topology.nix](../../airlock-machine/lifecycle/misc/lib/topology.nix), and
[qubes-topology.nix](../../airlock-machine/lifecycle/misc/modules/qubes-topology.nix).
The topology module initially described `attr` as an alias for `build`.
The bootstrap flake additionally accepts `hostName` / `hostname` in
`mkGuestContentModule`, preferring `hostName` if both appear:
[initial-runtime-config/flake.nix](../../airlock-machine/lifecycle/bootstrap/initial-runtime-config/flake.nix).

These are confirmed alternate interfaces; their original motivation was not
established for every pair. Generated snake_case fields may still be the right
machine-readable representation even if authored configuration uses camelCase.
One spelling per interface does not require renaming every serialized field.

- [x] Choose the canonical authored spelling for each pair after checking callers.
- [x] Update configuration, module option declarations, fixtures and documentation.
- [x] Remove alternate input spellings and duplicate/conflict-resolution branches.
- [x] Keep intentional normalization between authored configuration and generated
  topology/system JSON; retain unknown-option rejection.

Pair-by-pair progress:

- [x] `build` / `attr`: authored references now require `build`; normalized
  references retain `attr`.
- [x] `sourceRef` / `source_ref`: authored source options require `sourceRef`;
  normalized records retain `source_ref`.
- [x] `activationCommand` / `activation_command`.
- [x] `activationAction` / `activation_action`.
- [x] `switchAction` / `switch_action`.
- [x] `skipStart` / `skip_start`.
- [x] `readOnly` / `read_only`.
- [x] `persistentSize` / `persistent_size`.
- [x] `qrexecPolicy` / `qrexec_policy`.
- [x] Additional bootstrap `hostName` / `hostname` pair.

#### First change: require authored `build` (2026-09-30)

Worktree: `/home/user/dev/airlock-org/worktrees/airlock-simplify-build-option`,
branch `simplify-build-option` in the parent and component checkouts.
Component implementation commit: `0cc6883`.

Removed `attr` from accepted input keys in both schema libraries and from the
topology module's source-reference options, along with alternate-name/conflict
branches. Missing references now report `must set build`. Normalized
`source_ref.attr` and existing activation/validation consumers remain unchanged.
The caller inventory found no authored `attr` references needing conversion;
existing `attr` fixtures represent normalized JSON and remain valid.
Both configuration guides document the authored/normalized distinction.

New pytest coverage evaluates actual Nix schemas for system-profile, template
and domain-root references: `build` normalization, rejection of `attr` alone
or alongside `build`, missing `build`, malformed output attributes and absolute
subpaths. These tests are marked `nix` and excluded from the quick loop.

Validation:

- Targeted schema/regression checks: **20 passed** (including the existing
  `system-schema` and `topology-schema` checks).
- Existing NixOS topology integration check (`config-topology-eval`):
  **1 passed** in **17.60 s**.
- `make test-quick`: **629 passed, 58 skipped** in **5.77 s**.
- Evidence: worktree `.airlock-logs/build-option/quick/` and
  `/run/media/user/data/airlock/logs/compat-build-option/nix/` (JUnit and JSON
  reports), plus `/run/media/user/data/airlock/logs/compat-build-option/integration/`.
  No image/package build or live deployment was performed.

Additional finding from validation: the pre-existing subpath regular expression
accepts `../root.img`, despite its diagnostic promising paths without `..`.
This behavior was not changed in the alias cleanup.

- [ ] Investigate relative subpath traversal validation separately in both
  schema libraries and their consumers.

#### Second change: require authored `sourceRef` (2026-09-30)

Same worktree and branch as the first change. Component implementation commit:
`9711c8b`.

Removed `source_ref` from the accepted authored keys for system profiles,
host systems/reboot components, templates and closure backends (system, kernel,
initrd, modules, root and store). Both libraries now resolve `source` /
`sourceRef` without the alternate-spelling branches; template normalization
reuses the existing shared source helper instead of duplicating that logic.
The NixOS module already exposed only `sourceRef`, so its declarations needed
no further change.

The caller inventory found no authored `source_ref` options to migrate.
Normalized `source_ref` records, fixtures, and activation/validation consumers
remain unchanged. Authored `source = { build = "..."; }` references and concrete
`source` paths remain supported; conflicting non-null `source` and `sourceRef`
still fail. Both configuration guides document these interfaces.

Expanded pytest coverage exercises all distinct source-option parsers, including
the shared reboot-component parser through the host kernel. It verifies reference
and concrete-source normalization, null `sourceRef`, rejection of `source_ref`
even when null or supplied alongside `sourceRef`, and source/reference conflicts.
All Nix evaluation tests remain explicitly marked `nix`.

Validation:

- Focused regression suite: **88 passed** in **1.31 s** after strengthening
  assertions to inspect complete normalized source records.
- Existing `system-schema`, `topology-schema` and `config-topology-eval`:
  **3 passed**, included in an earlier **91 passed** run in **18.64 s**.
- `make test-quick`: **629 passed, 58 skipped** in **5.62 s**.
- Evidence: worktree `.airlock-logs/source-ref-option/quick/` and
  `/run/media/user/data/airlock/logs/compat-source-ref/{nix,regression}/`
  (JUnit and JSON reports). No image/package build or live deployment was
  performed.

#### Remaining listed authored aliases (source-only implementation)

Component commit: `c061a21`. Retired the remaining seven snake_case input
aliases and bootstrap `hostname`. Canonical camelCase inputs keep the existing
normalized fields and defaults. The caller inventory found only normalized
snake_case fixtures/consumers, which remain intact. Added pytest contract cases
and bootstrap hostname rejection cases, and updated the component guide.

At the user's request, **no tests, builds, live sessions or deployments are run
for this pass**. Earlier validation above predates these changes. New regression
cases and the quick/Nix checks require a later validation pass. Commits skip
pre-commit execution to honor the instruction not to run tests/check suites.

Further inventory found store inputs `blobDevice` / `blob_device` and
`viewBackend` / `view_backend`, plus accepted-but-unused `chunkSize` /
`chunk_size`. These require an additional local interface review; the initial
nine-pair table was not exhaustive.

### 2. Old dom0 module interface

The example [runtime.nix](../../airlock-machine/lifecycle/bootstrap/initial-runtime-config/airlock/runtime.nix)
explicitly labels `dom0.extraModules` backward-compatible and recommends
`dom0.host` for new configuration. The bootstrap flake accepts `extraModules`,
includes it in automatic boot/switch classification, and appends it alongside
`dom0.host.modules` when constructing host modules.

- [x] Move callers to `dom0.host.modules` and remove `dom0.extraModules`.
- [x] Remove the old field from accepted keys, module composition, example
  configuration and activation-classification comments/logic.
- [x] Preserve the current boot classification for arbitrary host modules.

Source-only implementation commit: `f1e6f02`. The runtime example and closure
smoke fixture use `dom0.host.modules`; the bootstrap no longer accepts or appends
`extraModules`. Added future regression cases for old-option rejection and the
boot classification of a non-empty host module list. **Validation pending**;
these checks have not been run.

### 3. Historical flake output names

[airlock-machine/flake.nix](../../airlock-machine/flake.nix) exports three old
unsuffixed VM configuration names as aliases of explicit development variants:

| Historical name | Explicit name |
| --- | --- |
| `nixosConfigurations.vm` | `nixosConfigurations.vm-dev` |
| `nixosConfigurations.vm-gui` | `nixosConfigurations.vm-gui-dev` |
| `nixosConfigurations.vm-gui-config` | `nixosConfigurations.vm-gui-config-dev` |
| `packages.x86_64-linux.airlock-vm-image` | `packages.x86_64-linux.airlock-vm-dev-image` |
| `packages.x86_64-linux.airlock-vm-gui-image` | `packages.x86_64-linux.airlock-vm-gui-dev-image` |
| `packages.x86_64-linux.airlock-vm-gui-config-image` | `packages.x86_64-linux.airlock-vm-gui-config-dev-image` |

The parent flake forwards component outputs. Existing consumers of the old
configuration names include `sync-nested-dom0-tools.sh`, `tests/test_local.py`,
and `airlock-machine/tests/machine_pychecks.py`. The image README explicitly
describes the historic unsuffixed outputs as development-mode aliases.

The component also exports `nixosConfigurations.usb-dev =
nixosConfigurations.machine`. Review this separately: the parent overrides
`usb-dev` with its actual USB image configuration, so ownership and intended
meaning matter. Standard flake `default` exports and parent forwarding are not
automatically historical compatibility debt.

- [x] Update callers to explicit development/release output names and remove the
  six historical names above.
- [x] Review whether the component's `usb-dev` alias is still useful and remove
  it if its callers can use `machine` directly.
- [x] Update image documentation and assertions that enforce historical aliases.
- [x] Preserve the meaningful development/release image distinction.

Source-only implementation commit: `427141f`. Removed all six historical VM output names
and the component `usb-dev` alias. Component callers use `machine`; the parent's
real USB image configuration remains `usb-dev`. Updated current scripts, image
guides and evaluation fixtures; removed assertions whose sole purpose was the
old aliases. **Tests and Nix evaluations are pending**, per the user's request.

### 4. Python import compatibility facade

The former `airlock_qubes_storage/engine.py` in the
[storage package](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage)
called itself a compatibility facade and wildcard-imports ten `_engine` modules:
core, pool, segments, objects, views, state, generations, artifacts, maintenance
and CLI. This retains the former flat import interface after implementation
was split into modules.

The package's `cli.py` and several methods in `qubes.py` still use the facade;
many other runtime/image tools already import `_engine` modules directly.
Tests and packaging references also need an inventory before removal.

- [x] Replace facade callers with explicit imports from the modules they need.
- [x] Remove wildcard re-exports and the facade once callers are updated.
- [x] Update package/file lists and test imports, preserving installed CLI and
  Qubes storage-plugin behavior.

Source-only implementation commit: `637b9b5`. Updated the installed CLI,
Qubes storage adapter, domain-storage helper and filtered-device test patches to
import the owning `_engine` modules directly. Removed the facade, updated Nix
import checks and the parent Python file list. **Tests/build checks pending**.

### 5. Renamed command and initialization markers

- The parent [Makefile](../../Makefile) retains
  `usb-builder-offline-gate: usb-builder-offline-check`, explicitly to preserve
  the former command name. `docs/usb-dev.md` documents the alias.
- [sys-gui/base.nix](../../airlock-machine/lifecycle/misc/modules/sys-gui/base.nix)
  accepts either `desktop-bootstrap-v1` or the older `desktop-launchers-v1`
  marker and skips desktop bootstrap if either exists.
- [coordinator.py](../../airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/coordinator.py)
  falls back from `/etc/airlock/machine-profile` to `/etc/airlock/usb-lab` or
  `/etc/airlock/usb-gui-lab` for images predating the coordinator.

- [x] Update callers/docs and remove the former offline-builder Make target.
- [ ] Establish the canonical desktop marker on retained user homes before
  deleting old-marker support; preserve one-time bootstrap behavior and user
  customization.
- [ ] Verify supported images/recovery environments provide `machine-profile`,
  then remove old profile discovery.

Source-only changes remove the renamed Make command and use only the current
desktop and machine-profile markers. Current machine-deployment modules emit
`machine-profile`; independent recovery deliberately does not expose whole-machine
deployment. Added a profile-marker regression case; **tests remain unrun**.

The two marker inventory/preparation boxes above remain unchecked: before any
activation, retained user homes must have the current desktop-bootstrap marker
if bootstrap has already occurred, and supported normal host generations must
provide `machine-profile`. No homes, images or live state were changed here.

### 6. Legacy closure-store conversions

[objects.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/objects.py)
contains roughly 170 lines across `migrate_single_blob_to_segments`,
`migrate_legacy_blob_map`, and `migrate_legacy_blob_maps`. They convert
`objects/data.blob` into segment storage, rewrite metadata and blob maps, and
translate `offset`, `central_offset` and `central_blob` into segment fields.

[pool.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/pool.py)
also upgrades old pool-manifest `object_store` fields and invokes the migrations
from `ensure_pool`. [segments.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/segments.py)
retains `legacy_object_blob_path`. Legacy layout detection also appears in view
reuse logic. These are actual conversion paths, not just version labels.

- [ ] Inventory pool manifests, object metadata and blob maps used by the lab,
  retained generations, recovery images and development fixtures.
- [ ] Record whether current-format replacement/reseeding is needed before
  removal. Any destructive reset needs its own explicit authorization.
- [ ] Remove automatic conversions, legacy path helpers and obsolete layout
  handling once supported state is current.
- [ ] Reject unsupported layouts clearly, preserving admission locking,
  durability, object/extent integrity, retention and recovery semantics.

### 7. Older host/controller index metadata

[views.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/views.py)
uses a full index build when the predecessor lacks `integrity` metadata.
[host.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/host.py)
supplies authenticated integrity information for older host indexes using boot
record hashes. Older controller indexes use one full build first. Existing
coverage includes `test_incompatible_predecessor_uses_full_index` in
`airlock-machine/tests/test_incremental_store_index.py` with a `legacy` case.

- [ ] Determine which missing-metadata defaults are solely for older formats
  and whether supported state can require those fields.
- [ ] Remove obsolete accommodation only after checking retained builder bases.
- [ ] Preserve full index construction for fresh images, absent predecessors,
  removed objects and the depth limit; those are functional requirements.
- [ ] Preserve authenticated predecessor reuse and checksum/layout verification.

### 8. Boot-entry and retained-artifact compatibility

[boot_presentation.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/boot_presentation.py)
supports generation discovery from old `version Generation N` fields, labels
systems lacking source provenance as `legacy generation N`, and continues
writing the short version field for previous releases' installer hooks during
rollback.

[host_artifacts.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_artifacts.py)
contains `migrate_presentation`, called while sealing retained entries. It
journals before/after fingerprints in `presentationMigration`, rewrites entry
presentation, and accepts either authenticated identity while resuming an
interrupted transition. Tests explicitly exercise legacy migration and resume.

The deployment documentation records removal of legacy normal menu entries
while preserving recovery. This does not establish that every recovery image,
retained generation or rollback installer has the current metadata contract.
The migration helper can also reformat entries after a future presentation
change, so its broader purpose should be checked before deleting it.

- [ ] Inventory retained normal/recovery entries, provenance, installer hooks
  and incomplete presentation journals before changing supported formats.
- [ ] Retire generation/provenance fallbacks and old installer compatibility
  once all supported entries use the current contract.
- [ ] Decide whether presentation migration remains a current feature or can
  be removed with its alternate-checksum handling.
- [ ] Preserve independent boot artifact verification, recovery availability,
  newest-first ordering and rollback behavior.

### 9. Explicit legacy deployment route: investigate first

The parent [machine-deploy.py](../../lifecycle/misc/tools/scripts/machine-deploy.py),
component [sys-dev-machine-deploy.py](../../airlock-machine/lifecycle/runtime/tools/sys-dev-machine-deploy.py),
and parent Makefile expose `--legacy-dom0` / `DEPLOY_LEGACY_DOM0`.
This maintains additional development build, transfer and direct-dom0 control
paths beside normal sys-dev/sys-ctrl submission. Tests explicitly cover the
legacy route. The runbook documents its development-only administrative access.

This is an available workflow, not merely a renamed flag. Also check profile
routing: the parent currently uses normal controller submission for `usb-lab`,
while other actions/profiles can reach the host-side path without the flag.
Removing the flag alone would not necessarily remove the duplicate workflow.

- [ ] List operations/profiles still depending on the route, including recovery
  and coordinator-update workflows.
- [ ] Record a decision to retain or retire it and the supported replacement
  for each remaining operation. Retirement is not assumed by this TODO.
- [ ] If retired, remove associated dispatch, build/transfer code, flags,
  variables, tests and documentation together.
- [ ] Preserve Qubes policy boundaries and observation/control separation.

## Versioning and aliases that serve current behavior

Most schema checks inspected accept one exact current identifier, rather than
a chain of old versions. Examples include:

- `airlock.sys-ctrl-machine-release.v2`: the machine receiver requires this
  exact transport schema.
- `airlock.closure-store.object.v2`: current object validation requires it.
- `airlock.pool-artifact.full.v2` and `airlock.pool-artifact.v4`: both are accepted
  because full and negotiated incremental transfer are different current modes.
  `airlock.pool-artifact-request.v2` is the current negotiation request schema.
- `.v1` identifiers throughout release, topology, setup-state, observation and
  storage code identify and validate distinct records/protocols.

Keep schema identity and strict validation. Higher version numbers alone do
not establish support for older versions, and renumbering current schemas would
provide little simplification. Package versions, lockfile versions and NixOS
`system.stateVersion` are separate mechanisms with their own purpose.

Keep upstream integration such as the `qubes-core-agent.service` service alias,
Qubes policy paths and the libvirt firewall compatibility setting. The latter
handles nixos-qubes' `none` setting being rejected by current nixpkgs. Review
dependency pins/patches against their actual integration requirements rather
than treating them as Airlock's internal API history.

`host/aliases.json` also serves a current boot requirement: image preparation
binds specialisation init paths to their real pool systems before the store can
be mounted. See [prepare-host-store.py](../../lifecycle/image/tools/prepare-host-store.py)
and [host_boot.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_boot.py).
It is not a historical name alias to remove as part of this cleanup.

## Execution order and acceptance

Start with configuration aliases, old output names, the import facade and the
renamed Make target. Handle persisted formats after state inventory and any
required replacement. Keep the legacy deployment-route decision separate.
Maintain the existing Xen, qrexec, QubesDB, qubesd, libvirt and policy architecture.

- [ ] Update all in-repository consumers of each retired interface; distinguish
  authored options from intentional serialized representations.
- [ ] Preserve or extend meaningful pytest coverage for current behavior,
  malformed/unsupported data rejection, installed imports and interruption/
  recovery where relevant. Remove tests whose sole purpose was an obsolete alias.
- [ ] Run `make test-quick` for implementation changes, with clear results under
  `.airlock-logs/`. Keep Nix evaluation/builds opt-in through `make test-nix`.
- [ ] Commit coherent component changes in `airlock-machine`, then update and
  commit the parent pin and affected integration/docs changes.
- [ ] For runtime/storage/boot changes affecting the lab, follow
  [machine-deployment.md](../machine-deployment.md): commit first, use
  `make machine-plan` / `make machine-apply` with `DEPLOY_TARGET=...`, and verify
  the whole-machine generation, including activation/reboot as required.
- [ ] Record build/test results separately from live acceptance. Inventory and
  cleanup must include retained recovery/rollback state, not just a fresh image.

This is separate from [TODO 08](08-deployment-metadata-validation-reuse.md)'s
performance work and [TODO 09](09-deployment-code-health.md)'s behavior-preserving
cleanup. Preserve completed deployment/storage/boot acceptance records as
historical evidence when updating current interfaces and documentation.
