# Isolated shared-pool build proof

Status: **historical proof**, superseded as an implementation plan by the
[pool-backed build store](sys-dev-build-store.md) and completed
[sys-dev machine build](completed/01-sys-dev-machine-build.md) and
[whole-machine deployment](completed/03-sys-dev-machine-deployment.md).
The probe limits and next steps below apply to that experiment; consult those
follow-ups for implemented behavior and remaining validation gaps.

Live probe on `sys-dev`, image seed `a26b5c0`: passed. The preceding cold
guest build was interrupted before admission. No generation was deployed.

The probe pinned sys-dev's existing generation-1 pool view with an extra
`mark-running` reference. Inside a private mount namespace, Nix's
[experimental local overlay store](https://nix.dev/manual/nix/2.34/store/types/experimental-local-overlay-store)
combined that read-only EROFS view with a disposable writable layer.
Only Nix registration metadata (3,881 bytes, from the development store) was
transferred; no package payloads were transferred.

- A real derivation ran existing Bash and coreutils with substitution disabled.
- All 12 registered input paths passed NAR content verification.
- None appeared in the writable layer: only the new output and derivation did.
- Root could not open the lower Bash binary for writing (`EROFS`); the ordinary
  builder user could not open it for writing through the merged view either.
- Binary hashes remained unchanged. The lower Nix metadata database was mounted
  read-only; new Nix state belonged to the private layer.

Separately, `test_builder_pin_protects_pool_view_until_released` exercises the
real pool engine on disposable data. With ordinary retention disabled and no
current generation, cleanup removes the unpinned generation but preserves the
pinned generation and its exact backing bytes through GC and compaction.
Releasing the pin permits those bytes to be removed. No live pool GC was run.

All live probe mounts, private files, build output and the extra pin were
removed. Existing guest references and cached build dependencies were retained.
Local probe source, metadata and results are under
`.airlock-logs/pool-overlay-proof/`; the prepared-store suite passed 20 tests
with one skipped. `make test-quick` passed 356 tests with 23 skipped.

## Boundary and next step

This proves small-build reuse, not a whole-machine build or a production
security/lifecycle design. A root user in the builder VM can shadow paths in
its private layer, but cannot modify the read-only shared backing.
The probe used `sandbox=false`; it does not validate a hardened build sandbox.

Production needs a supported metadata export and frozen view containing the
desired build inputs, plus a lease acquired before exposure and released only
after detachment. Crash recovery and concurrent cleanup still need testing.
Sharing runtime packages does not supply missing compilers, sources or other
build-only dependencies.

Two integration details surfaced: Nix needs an empty `.links` directory, supplied
here by a temporary read-only overlay; its build hook did not inherit the
experimental read-only-store setting, so the local-only probe disabled that hook.
Neither workaround changed the installed image or the normal build wrapper.
