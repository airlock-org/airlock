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

## Related store-index improvement (2026-09-30)

Machine implementation `121ba4d`, hardened by `d291864`, extends an authenticated
predecessor EROFS index with only new objects. It preserves logical blob offsets,
copies the predecessor image, and retains the existing admission and publication
checks. Removed objects, missing integrity metadata, and depth 16 fall back to a
full build. Pytest covers real index extraction, interruption/retry, corruption,
changed object identity and physical extent relocation. This optimization is
separate from the operation-scoped metadata caches still planned above.

The first live migration needed full host and controller indexes. Controller
index construction took **129.37 s**, and controller preparation **149.68 s**.
The follow-up at machine source `3efe570` inherited **2,660 objects**, appended
**4**, and built its controller index in **0.62 s**. Controller preparation
fell to **17.04 s**. Total follow-up preparation was **94.21 s**, versus the
previous small boot-menu update's **274.13 s**; its build took **53.39 s** and
is reported separately. All seven guest generations were reused, with zero
guest/root payload transfers; four small host/transport paths transferred
111,126 compressed bytes. These are observed runs with different cache states,
not a controlled cold-cache benchmark or a guaranteed future speedup.

The host still used a full index build (**35.35 s**) in this follow-up. Remaining
costs include controller metadata publication beyond index construction, host
publication, and ordinary release evaluation/building. New host/controller
timing fields separate object verification/admission, view preparation and index
construction, and count inherited/new objects. Existing payload verification remains intact. Evidence: `.airlock-logs/store-preparation/live-performance.json`,
`plan-initial.json`, `plan-incremental.json`, and the regression/benchmark logs.

The regular feedback loop now excludes Nix checks and automatic tool preparation:
**665 passed, 22 skipped in 6.00 s**, compared with the previous full run's
**742 passed, 25 skipped in 220.92 s**. Nix evaluators were blocked during the
final quick run; there were no evaluator calls. Installed EROFS tools were on
`PATH`. Nix checks remain available explicitly through `make test-nix` and are
not a routine deployment gate.

At this historical checkpoint, normal apply reused the prepared candidate with
zero builds/transfers. Typed verification of transaction `1790766764373029803`
reported **reboot-required**: six running guests matched their selections and
usr-browser was stopped. Controller builder generation 17 was mounted and
active; inherited and appended host init files were readable through the real
kernel mount. Selected host was `fhrlj5cxg5lnc0xjw5rwv7bzn7zrbalh`, while the
running host remained `mq1z3vjikp3ynnc4ryrx64f3csplc26y`. Host reboot verification
was pending at that point.
Evidence: `verify-after.json`, `apply.json`, and `controller-mounted-check.json`
in `.airlock-logs/store-preparation/`. No diagnostic mutation was made live.

Later on 2026-09-30, the boot-menu follow-up at machine revision `5bd3a92`
was applied and physically rebooted. Typed verification of transaction
`1790768363786110491` reported **complete**: running and selected host both
resolved to `dyy6c32zx6fanzfrxyzgriw4rchihhwy`, all six running guests matched
their selections, and usr-browser remained stopped. Evidence:
`.airlock-logs/boot-screen-consistency/verify-after-reboot.json` and
`acceptance.json`; see the [boot-menu final acceptance](../completed/10-boot-menu-presentation.md#final-acceptance-2026-09-30).
The earlier performance-test host generation was superseded before reboot and
was not separately boot-verified. This later acceptance does not complete the
operation-scoped metadata and validation cache work planned above.
