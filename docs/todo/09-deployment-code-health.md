# 09: Simplify deployment cleanup and record handling

Status: **planned** (2026-09-30).

Three focused cleanups in `airlock-machine`, preserving deployment behavior,
storage formats, and lock boundaries. Keep this separate from
[TODO 08's performance changes](08-deployment-metadata-validation-reuse.md).

## Implementation

### 1. Preserve startup errors during builder cleanup

`lifecycle/runtime/tools/scripts/sys-dev-build-store.py`: startup cleanup
currently stops at the first failed unmount, obscuring the original failure.

- [ ] Attempt every tracked unmount in reverse order, reporting individual
  cleanup failures, then re-raise the original startup error. Follow the existing
  host-boot cleanup pattern without silently ignoring failures.

### 2. Write saved deployment records consistently

`lifecycle/runtime/tools/sys-ctrl-guest-deploy.py` has a durable `record()` helper,
but fresh build results use plain `write_text()`.

- [ ] Write the fresh `build.json` through the existing atomic record helper.
- [ ] Align prepared-record writes in `sys-ctrl-machine-receive.py` with the
  same durability sequence: flush/fsync the temporary file, replace the target,
  and fsync the parent directory. Preserve token calculation and record contents.
  Reuse available helpers where packaging permits; avoid a new utility framework.

### 3. Extract the shared host-publication finishing steps

`lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/_engine/host.py`:
both `publish_live()` branches seed generation metadata, refresh the controller
builder base, and collect timings.

- [ ] Extract that sequence into one small helper. Preserve each branch's
  registration input, timing fields, admission-lock lifetime, and temporary
  Nix GC root protection.

## Acceptance

- [ ] Add pytest coverage showing that a cleanup failure does not prevent later
  cleanup attempts or replace the original startup exception.
- [ ] Verify atomic record replacement/durability and both publication branches,
  including their registration inputs and ordering, using focused pytest coverage.
- [ ] Pass `make test-quick`; record a brief result summary here.

Generation-reuse eligibility/receipt refactoring is out of scope. Commit each
coherent change and retain the completed acceptance summaries for TODOs 05–07.
