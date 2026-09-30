# 11: Simplify internal compatibility before release

Status: **local implementation complete; quick tests passed; remaining validation
and live acceptance pending** (2026-09-30). Sections 1–8 have local changes;
section 9 has a documented retention decision. Latest quick suite: **645 passed,
59 skipped**. No live changes have been performed for this cleanup.

Airlock is unreleased. The local source contains a meaningful amount of
compatibility code, concentrated in configuration parsing, storage and deployment.
Standardize on one current internal interface and storage format where practical,
while preserving the existing Qubes architecture and required upstream integration.

This document preserves the initial read-only findings and records subsequent
implementation in the parent repository and its `airlock-machine` checkout.
The initial investigation performed no live-state inventory, build, test or
deployment. Earlier build/source-reference cleanup has test evidence below;
later implementation was initially source-only at the user's request. The user
subsequently authorized quick tests; the latest results are recorded below.
Counts describe confirmed examples rather than an exhaustive count of
compatibility branches. Implementation subsections preserve their initial
source-only status; references to unrun tests below describe that implementation
pass. See the latest validation record for current coverage. Persisted-state
changes must not be activated until
the pending inventory, validation and acceptance steps are completed.

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

- [x] Investigate relative subpath traversal validation in both schema
  libraries and their consumers; implement rejection locally.

Source-only follow-up: component `1474686` rejects `.` and `..` components in
both source-reference parsers and system activation commands. Normal dotted
filenames and hidden directories remain valid. Added positive/negative pytest
cases; **not run**. No file/path rewriting or live state changes occurred.

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

Additional local interface inventory and implementation:

- [x] Store `blobDevice` / `blob_device` and `viewBackend` / `view_backend`:
  component `dd3e557` requires authored camelCase. Existing normalized store
  fields remain intact for their current consumers. Added rejection cases.
- [x] Store `chunkSize` / `chunk_size`: neither accepted input ever affected
  normalization or storage. Removed both from accepted authored keys and removed
  obsolete hand-written normalized fixture fields (`dd3e557`, `0fa7f66`).
- [x] Runtime private-size readers previously accepted normalized
  `persistent_size` or authored `persistentSize`. Setup reconciliation and parent
  image preparation now consume `persistent_size`; the parent image fixture
  uses that current normalized field (`0fa7f66` plus parent integration).
- [x] Setup's normalized store readers now consume `blob_device` and
  `view_backend` directly, without camelCase fallback (`cf6d525`).

The initial nine-pair table was not exhaustive. Both configuration guides now
include the additional authored store spellings and relative-path contract.
All new regression cases and changes in this subsection remain **unvalidated**.

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
  activating current-only marker support; preserve one-time bootstrap behavior
  and user customization.
- [ ] Verify supported normal deployment environments provide `machine-profile`
  before activating current-only profile discovery.

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
- [x] Remove local automatic conversions, legacy path helpers and obsolete
  cached-layout conversion handling; activation depends on the inventory above.
- [x] Implement clear unsupported-layout rejection while retaining admission
  locking, durability, object/extent checks, retention and recovery code paths.
- [ ] Validate those invariants with the new regression cases and existing
  storage/admission/maintenance suites before activation.

Source-only implementation: component `c334658` deletes all three migration
functions and the old blob-path helper. Existing pool manifests must describe
current segmented storage; a single-blob pool or unsupported manifest fails
before `ensure_pool` creates directories or rewrites metadata. Cached blob maps
with `central_blob` or extents lacking segment identity/offset are rejected,
instead of converted or silently rebuilt. New pytest cases cover read-only
refusal and reopening a current pool; **none have been run**.

Current extent-location handling remains: newly appended segmented objects can
use their top-level current extent before a location-history list exists, and
compaction maintains both current and historical locations. That is current
storage behavior, not a single-blob migration alias. No lab pool, retained
metadata, recovery image or object payload was inspected or changed. Any
required reseeding/replacement remains a preactivation task; no reset is
implicitly authorized by this source cleanup.

### 7. Older host/controller index metadata

[views.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/views.py)
uses a full index build when the predecessor lacks `integrity` metadata.
[host.py](../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/host.py)
supplies authenticated integrity information for older host indexes using boot
record hashes. Older controller indexes use one full build first. Existing
coverage includes `test_incompatible_predecessor_uses_full_index` in
`airlock-machine/tests/test_incremental_store_index.py` with a `legacy` case.

- [x] Audit current metadata writers and remove missing-integrity/depth
  accommodation from the local index contract.
- [ ] Inventory retained host/controller/builder bases and cached view manifests
  before activating the stricter contract; replace/reseed unsupported state
  only with separately authorized preparation.
- [x] Preserve full construction for fresh images, absent predecessors,
  removed objects and the depth limit.
- [x] Preserve authenticated reuse and checksum/layout verification in code.
- [ ] Run index, host-publication, artifact-admission and recovery coverage.

Source-only implementation: `34bd7e6` removes host integrity synthesis and the
missing-integrity full-build fallback. An eligible runtime predecessor requires
its recorded integrity and a non-negative integer `index_depth`. Cached view
manifests require depth too. Current fresh/depth-limited/non-additive index
construction is retained; additive reuse still verifies the predecessor.
Replaced the historical full-build expectation with explicit metadata-refusal
coverage, including predecessor-byte preservation; **tests are unrun**.

Writer audit found an additional current omission: received-artifact generation
manifests wrote integrity but omitted `index_depth`. Component `3b2f013` now
persists the materialized view's depth and updates its pytest fixture/assertion.
Ordinary admission already writes both fields. Artifact negotiation's decision
to reuse only a proven matching generation, otherwise perform normal admission,
remains a current protocol behavior; it is distinct from predecessor-index
format accommodation. No retained bases were read from a live system.

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
- [x] Remove local retained-generation/provenance fallbacks and old short
  installer-version output; activation requires the inventory above.
- [x] Retain presentation refresh and its resumable checksum transition as
  current behavior, rather than historical-format support.
- [x] Preserve independent boot verification, recovery, newest-first ordering
  and rollback code paths, including operation without a mounted old system.
- [ ] Run boot-menu/artifact/interruption coverage and verify retained normal
  and independent recovery choices before activation.

Source-only implementation: component `2860f59` requires explicit
`# airlock-generation` for retained entries and committed provenance when
formatting entries. It removes `legacy generation N` labels and emission of
`version Generation N` for historical Airlock installer hooks. Current native
NixOS/Xen entries still require discovery from upstream's version field; only
native-entry call sites opt into that discovery. This upstream interface is
retained deliberately. Current machine normal and independent recovery
configurations both embed `boot-source.json`.

Recorded revision/timestamp on a retained entry remains usable when its original
system path is unavailable; recovery must not depend on mounting that system.
`refresh_presentation` (formerly `migrate_presentation`) still journals verified
before/after entry bytes in `presentationMigration`. A current presentation
change can be interrupted too, so its alternate authenticated fingerprint is
retained. Updated coverage models a current authenticated entry refresh, plus
missing identity/provenance rejection and recorded-source reuse. **All unrun**.
No EFI partition, firmware variables, boot entry, recovery profile or retained
journal was modified. Older rollback installer hooks and provenance-free entries
are outside the new local contract; inventory/replacement is still pending.

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

- [x] Audit source dispatch for operations/profiles depending on the route.
- [x] Record the retention decision and outstanding replacement requirements.
- [x] Preserve existing policy boundaries and observation/control separation;
  this investigation introduces no additional control route.

**Decision: retain the development route.** No dispatch/transport code or flags
are removed in this pass. Its retirement is not an outstanding local change
selected by this TODO.

| Current source path | Why it remains |
| --- | --- |
| Parent `usb-lab` plan/apply/status/verify/resume/rollback | Defaults to sys-dev/sys-ctrl submission; explicit `--legacy-dom0` still selects the separate administrative development transport. |
| Parent `build` | Local host-side build does not require a remote session or the controller route. |
| Parent `usb-gui-lab` | Outside the parent controller-forwarding condition. The profile normally lacks a sys-dev bridge; provisioning/transport and a supported controller workflow must be established before removing the host-side route. |
| Parent `guest-plan` / `guest-apply` | Uses an existing sys-dev admission receipt and the direct development coordinator channel; removing the flag alone would not remove this path. |
| Parent guest `--update-coordinator` | Builds/transfers a committed standalone coordinator for already flashed development images. No equivalent update path is supplied by the ordinary whole-machine submission dispatch. |
| Component sys-dev CLI | Defaults to typed controller build/plan/apply and qrexec machine control; `--legacy-dom0` still selects direct development build/transfer/control. |
| Independent recovery | Does not expose normal whole-machine deployment or `machine-profile`; its explicit recovery/boot selection functionality must be preserved. It is not evidence that the controller workflow covers all administrative recovery operations. |

Removing the route requires a separate workflow decision and replacements for
these dependencies, followed by removal of flags, Make variables, dispatch,
transfer/build code, tests and runbook text together. Current development shell
and Qubes policy constraints remain intact. This is a source audit, not a live
claim that every alternative profile/transport is deployed or operational.

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
renamed Make target. Persisted-format local implementation is complete, but activation must wait
for state inventory and any required replacement. Keep the documented
development-route retention decision separate.
Maintain the existing Xen, qrexec, QubesDB, qubesd, libvirt and policy architecture.

- [x] Update in-repository consumers found for each retired interface; distinguish
  authored options from intentional serialized representations.
- [x] Write or update meaningful pytest coverage for current behavior,
  malformed/unsupported data rejection, installed imports and interruption/
  recovery where relevant. Remove tests whose sole purpose was an obsolete alias.
- [x] Run `make test-quick` for implementation changes, with clear results under
  `.airlock-logs/`. Keep Nix evaluation/builds opt-in through `make test-nix`.
- [x] Commit coherent component changes in `airlock-machine`, then update and
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

## Latest quick validation (2026-09-30)

The user authorized `make test-quick` after the source-only implementation pass.
Ran the parent target in the same worktree, covering component tests and parent
integration checks: **645 passed, 59 skipped in 5.68 s**; exit status **0**.
No implementation changes were needed after this run.

Selected coverage included:

| Suite | Result |
| --- | --- |
| Current storage format/rejection | 10 passed |
| Boot menu and retained identity/provenance | 13 passed |
| Host publication, boot artifacts and resumable presentation | 29 passed |
| Transferred/incremental artifact admission | 50 passed |
| Machine coordinator, including current profile discovery | 122 passed |

These are selected suite counts within the overall result, not additional runs.
The quick target excludes Nix-marked authored-option/source-reference/bootstrap
schema tests. The ten real incremental-index cases skipped because patched
`airlock-erofs-utils` is unavailable in the quick environment. Other skipped
cases retain their existing environment prerequisites. These skips do not
establish Nix integration, real EROFS index correctness or live acceptance.

Evidence in the worktree:

- `.airlock-logs/compat-cleanup-quick.log`: complete command/output.
- `.airlock-logs/compat-cleanup/quick/pytest-junit.xml`: JUnit results.
- `.airlock-logs/compat-cleanup/quick/pytest-report.json`: machine-readable results.

No Nix evaluations/builds, live session changes, resets, deployment, activation,
reboots or live tests were performed by this validation run.

## Current handoff

All selected local code work is committed in
`/home/user/dev/airlock-org/worktrees/airlock-simplify-build-option`, branch
`simplify-build-option`, including component commits and parent pin/docs updates.
The cleanup did not edit the original checkout. Initial implementation commits
bypassed hooks to honor the then-current instruction to defer tests/check suites.
The later authorized quick run passed as recorded above.

Remaining: opt-in Nix schema/integration validation, real EROFS index checks,
and persisted-state inventory/preparation before any separately authorized
activation and live acceptance. The successful quick run does not complete
these remaining checks. No further test/build or live operation is implied by
this handoff.
