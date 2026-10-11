# 09: Simplify deployment cleanup and record handling

Status: **implemented and locally verified** (2026-10-11). No live deployment
or physical acceptance was performed for these changes.

Three focused cleanups in `airlock-machine`, preserving deployment behavior,
storage formats, and lock boundaries. Keep this separate from
[TODO 08's performance changes](08-deployment-metadata-validation-reuse.md).

## Implementation

### 1. Preserve startup errors during builder cleanup

`lifecycle/runtime/tools/scripts/sys-agent-build-store.py`: startup cleanup
currently stops at the first failed unmount, obscuring the original failure.

- [x] Attempt every tracked unmount in reverse order, reporting individual
  cleanup failures, then re-raise the original startup error. Follow the existing
  host-boot cleanup pattern without silently ignoring failures.

### 2. Write saved deployment records consistently

`lifecycle/runtime/tools/sys-ctrl-guest-deploy.py` has a durable `record()` helper,
but fresh build results use plain `write_text()`.

- [x] Write the fresh `build.json` through the existing atomic record helper.
- [x] Align prepared-record writes in `sys-ctrl-machine-receive.py` with the
  same durability sequence: flush/fsync the temporary file, replace the target,
  and fsync the parent directory. Preserve token calculation and record contents.
  Reuse available helpers where packaging permits; avoid a new utility framework.

### 3. Extract the shared host-publication finishing steps

`lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/host.py`:
both `publish_live()` branches seed generation metadata, refresh the controller
builder base, and collect timings.

- [x] Extract that sequence into one small helper. Preserve each branch's
  registration input, timing fields, admission-lock lifetime, and temporary
  Nix GC root protection.

## Acceptance

- [x] Add pytest coverage showing that a cleanup failure does not prevent later
  cleanup attempts or replace the original startup exception.
- [x] Verify atomic record replacement/durability and both publication branches,
  including their registration inputs and ordering, using focused pytest coverage.
- [x] Pass `make test-quick`; record a brief result summary here.

Generation-reuse eligibility/receipt refactoring is out of scope. Commit each
coherent change and retain the completed acceptance summaries for TODOs 05–07.

## Local validation

Focused pytest coverage passed: 70 tests, 5 Nix tests deselected. Coverage
includes failed-unmount continuation with the original startup exception,
file/replace/directory durability order for both record writers, and both host
publication branches under the admission lock and temporary GC root. The final
quick-suite result is recorded in the public-readiness report.
