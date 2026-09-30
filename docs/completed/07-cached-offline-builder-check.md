# Cache offline acceptance before USB image assembly

Status: completed on 2026-09-24. Builds on the
[completed offline builder baseline](06-builder-input-baseline.md).

## Goal and scope

Make `make usb-dev-image` require evidence that the image's declared inputs
support the two documented offline guest changes. Reuse a matching successful
check on unchanged image builds, and stop before image assembly if a required
check fails.

The examples remain an `/etc` marker and an existing service-setting change
in `usr-home`. They build changed guest systems using the seeded software;
passing them does not establish support for arbitrary new packages or versions.

## Build flow

1. Prepare the committed machine source and pinned flake inputs using the
   existing `usb-dev-prepare-inputs` flow.
2. Realize the declared builder input closure through the existing Airlock
   build store. Calculate the acceptance fingerprint from those exact inputs.
3. Reuse a valid passing receipt for that fingerprint, or run both isolated
   offline trials. A failed trial must fail the image prerequisite.
4. Assemble the USB image using the same source, manifest and builder inputs
   that the receipt identifies. Detect changes between checking and assembly
   rather than applying an old receipt to different inputs.

Preparation and closure realization can still take time when dependencies are
missing. A cache hit skips the acceptance trials, not required image inputs.

## Cache identity and receipts

Start with conservative invalidation. The fingerprint must include:

- The clean committed machine source identity and lock-file contents.
- The complete declared builder input closure, including pinned flake sources
  and registration metadata.
- Both example definitions and the code that constructs and runs the trials.
- The Nix version/store identity, target system, and relevant isolation settings
  or tool identities that affect what the check proves.
- A receipt format/version identifier, so changes to validation semantics can
  invalidate old results explicitly.

Use immutable Nix store identities and content hashes where practical. Do not
use file modification times or a generic "last check passed" marker as proof.
Image assembly must still realize its declared inputs if previous store data
has been collected.

Save a receipt only after both examples pass. Write it atomically, bind it to
the fingerprint, and include both scenario results and the evidence locations.
Missing, malformed, incompatible, incomplete, failed or mismatched receipts
must cause a fresh check. A failed forced check must invalidate any passing
receipt for the same fingerprint. Serialize checks for the same fingerprint
so concurrent image builds cannot publish conflicting results.

Keep receipts and logs under the existing configured Airlock cache/log
directories. Emit a clear cache-hit, cache-miss or failure reason and retain
machine-readable results.

## Command behavior and isolation

- `make usb-dev-image` uses the cached acceptance prerequisite automatically.
- `make usb-builder-offline-check` always runs fresh and can publish a passing
  receipt for subsequent image builds. Preserve the existing legacy alias.
- Every actual trial still starts with a separate empty writable store and
  fresh caches, with networking blocked and substitutes disabled. Reuse the
  passing receipt; do not warm a trial with outputs from earlier trials.
- Preserve the current protections around the development host's main
  `/nix/store`. Temporary build outputs remain in the isolated trial stores.

Physical USB acceptance remains separate. Repeat the relevant live checks when
storage or boot behavior changes; this cache does not establish that the pool
mounts correctly, pins survive cleanup, or the machine works after reboot.

## Acceptance

- [x] An image build with no receipt runs both examples before image assembly.
- [x] An unchanged image build reuses its passing receipt without running the
  example builds again.
- [x] Changing any fingerprint input causes a fresh check.
- [x] A failed check stops image assembly and cannot publish or leave a usable
  success receipt for that fingerprint.
- [x] The explicit check runs fresh even when a matching receipt exists.
- [x] Invalid receipts and interrupted or concurrent runs cannot bypass checks.
- [x] The image consumes the exact inputs identified by the accepted receipt.
- [x] Focused pytest coverage exercises cache validity, invalidation, failure,
  forced runs and ordering; existing isolated offline acceptance still passes.
- [x] Update the image-build documentation to explain automatic checking,
  cache reuse, forced checks and the bounded offline capability.

Implementation entry points: `Makefile`,
`lifecycle/image/tools/offline-builder-gate.py`,
`tests/test_offline_builder_gate.py`, and `docs/usb-dev.md`.

## Acceptance record: 2026-09-24

The forced check passed both isolated guest builds using the 1,728-path builder
closure for machine revision `42ce4161617b81a135f638a04e38765d1d64ef3d`.
The next `make usb-dev-image` reported a cache hit, skipped the trials, validated
the prepared guest store, assembled the image, and published its result link.
After adding the snapshot recheck, a second forced check and cached image build
also passed. The current receipt and results are under the configured Airlock
cache and log directories.

Seven focused pytest checks passed, including seed reuse and image snapshot
tampering, and Ruff passed. `make test-quick` reported 513 passed, 23 skipped,
and one unrelated failure in `test_local_check[run-nested-gui-smoke-static]`:
that test expects an import command string absent from an untouched machine
file. The image build is local validation; no new physical USB boot was run for
this cache change.
