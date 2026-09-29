# 08: Reuse deployment metadata and validation work

Status: **planned** (2026-09-30).

Reduce repeated reads and checks during changed-release preparation. This is a
performance follow-up to completed [direct pool deployment](../completed/06-direct-pool-machine-deployment.md)
and [unified storage](../completed/07-unified-host-guest-store.md); their acceptance
remains recorded separately.

## Baseline

The latest verifier/builder update took about **135 seconds to build** and
**293 seconds to prepare**. Host publication and controller builder-base
preparation accounted for **217 seconds** of preparation. These broad timings
identify the target, but do not establish how much time repeated checks consume.
Evidence: `.airlock-logs/boot-verification-fix/final-deploy-plan/receipt.json`.

## Implementation

### 1. Read shared metadata once per operation

- [ ] Add an operation-scoped cache for segment metadata, object records, and
  backing-file identity/size where publication repeatedly examines the same data.
- [ ] Share it across applicable preparation helpers, extending existing local
  caches rather than introducing a separate storage format or service.
- [ ] Invalidate entries after writes, segment growth/sealing, replacement, or
  other mutations within the operation; discard the cache on success or failure.

### 2. Reuse validation results within the operation

- [ ] Carry successful checks forward between host publication, controller
  builder-base preparation, and binding wherever they share an operation scope.
  Extend the existing scoped payload-verification pattern where appropriate.
- [ ] Reuse a result only for the same backing-file identity, extent/content,
  and validation requirements. Preserve registration, generation, pin, and
  provenance checks; a weaker check cannot satisfy a stronger one.
- [ ] Keep reuse within the appropriate admission lock and process. Existing
  subprocess boundaries must retain their checks; do not add a persistent cache
  or pass unverified cached results across a trust boundary.

File paths alone are insufficient cache keys. Use backing-file identity checks
(including device, inode, size, and change timestamps) and account for mutations
made by the operation itself. Failed or partial validation must never be cached.
Each new operation starts with fresh state.

## Acceptance

- [ ] Record metadata reads/parses and validation cache hits/bytes hashed so the
  reduction in work can be attributed, alongside preparation time and disk I/O.
- [ ] Add pytest coverage for reuse, mutation/invalidation, corrupt metadata or
  payload rejection, and a fresh cache after failure. Cached and uncached paths
  must enforce the same validation requirements.
- [ ] Pass `make test-quick`, then verify a changed deployment and an unchanged
  repeat using the [normal deployment workflow](../machine-deployment.md).
  Confirm generation bindings and retained data after activation/reboot as needed.
- [ ] Record a brief measured result and remaining costs; do not promise a
  specific speedup from the baseline timings alone.

RAM-backed temporary index staging is **out of scope**. This work preserves the
storage format, generation behavior, and deployment authority.
