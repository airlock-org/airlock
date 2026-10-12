# Calculator launch failures, 2026-10-12

The operator reproduced a refused launch during boot and a separate delayed
window after closing and reopening Calculator with the desktop unlocked.
No tests were run. No runtime configuration overlays or process-memory writes
were used. The diagnostic launch used a collected transient user unit.

## Boot-time refusal

At 08:22:12 UTC, the existing launcher recorded Request refused at exactly the
same time dom0 stopped qubesd. The policy daemon could not obtain system info
while qubesd was unavailable. Qubesd returned at 08:22:15. The GUI watcher
reconnected and attached usr-home at 08:23:35. Usr-home's agent and Xorg were
running by inspection; the selected khaki host had booted successfully.
Whole-machine verification reports complete. No GPU fence/flip timeout matches
were observed in this boot.

The launcher previously exited on its first failed feature lookup/readiness
request, converting a transient control-service outage into a permanent user
error. Component cda910c retries only feature lookup and session readiness
within a shared 90-second budget. StartApp remains a single submission, avoiding
duplicate application launches. Shell syntax parsing succeeded; no automated
checks were executed. The diagnostic launch attempted while GNOME was locked
was correctly refused by workspace preparation and is a separate condition.

## Custom-label window identity

Khaki is Qubes label index 9. The focused-window identity reader accepted only
indices 1 through 8 despite first verifying the supplying client's executable
against the installed GUI daemon. A live read of Calculator XID 0x1600089
returned unknown. Its X11 properties identify Calculator in usr-home, with a
viewable window on workspace 0. The workspace extension hides windows pending
identity resolution and retries unknown identities, then falls back to an
isolated neutral workspace. Thus custom-label windows enter the wrong handling
path; process existence and even X11 viewability do not establish physical
visibility. Both manual launches eventually appeared and the operator confirmed
Calculator input worked. Exact compositor timing was not traced; GPU-renderer
probe warnings alone do not establish the cause of the delay.

Component 293bea5 accepts positive custom label indices within the one-byte
property range (1 through 255), retaining executable provenance and all property
validation. Component 8619ca9 updates invalid-label coverage and adds custom-label
cases proving untrusted executable provenance remains rejected. Coverage was
written but not run, respecting the operator instruction.

Both fixes are committed and pinned. Live deployment and physical acceptance
of the new generation are pending. The initial readiness-only build succeeded, but planning failed
at dom0 inventory admission: insufficient dom0 store space for release import.
Available space was 2,092,072,960 bytes against a 2,147,483,648-byte reserve,
before accounting for missing release objects. No apply was submitted.
Read-only release inventory lists 87 retained releases, 75 without retention
blockers; none were pruned. Guarded development-host gc-auto was invoked, but
does not resolve this separate physical dom0 capacity constraint. The final
combined revision must be built/planned after controlled storage cleanup. Evidence and controller logs are under
.airlock-logs/calculator-launch-2026-10-12/.

## Authorized six-candidate cleanup

The operator requested a before/after baseline and pruning of exactly the six
unused prepared candidates identified in the review. Fresh coordinator previews
showed no blockers and no boot-profile generations for each. The coordinator
pruned 04169e377881, 6df4ead09e6e, 8616596dd273, cd9a473ecdeb, cf3529386ee5 and
ed07925de8fa, retaining its audit records. All six pending roots are absent.
Release inventory now contains 81 records rather than 87. Current khaki selection,
immediate gray rollback and running host selection remain unchanged.

Available filesystem bytes immediately before prune: 2,107,490,304.
Available bytes immediately after: 2,109,140,992.
Observed increase: 1,650,688 bytes (1.57 MiB), subject to ordinary live-system
writes during the measurement. No broader pool GC, compaction, journal cleanup,
host Nix GC or VM disk deletion was performed. Dropping these retention roots
alone does not establish the bulk artifacts' reclaimable size. The deployment
space reserve is still unmet. No tests were run.

Full previews/results and baseline measurements are retained in dom0 at
/var/log/airlock/candidate-cleanup-2026-10-12.json. Updated release and selection
inventories are under .airlock-logs/calculator-launch-2026-10-12/.

## Manifest-based pool GC

After operator authorization to reclaim unused closure-store artifacts/views,
the existing airlock-closure-store gc ran with keep-generations=2. A read-only
compaction preview was stopped before any compaction because its full payload
verification was unnecessary for this initial pass. The existing GC also
verifies live payloads; it finished successfully in approximately 465 seconds,
hashing 17,096,320,512 bytes with 4,542 verification cache hits. No validation
checks were bypassed and no replacement maintenance mechanism was introduced.

Available bytes before GC: 2,105,470,976; after: 12,773,150,720.
Observed gain: 10,667,679,744 bytes (9.94 GiB), subject to live-system writes.
The closure store's allocated usage fell from about 30 GiB to 20 GiB;
the filesystem is now approximately 78% used, with about 12 GiB available.
GC removed 70 stored generations: 16 dom0, 13 sys-agent-builder and
41 sys-ctrl-builder. It removed 57 unused store views, zero shared-object
metadata entries and zero backing segments. It retained 198 generations and
recognized 36 running references, with no in-progress storage generations.
This did not prune additional machine-release records, delete VM private disks,
clear logs or run host Nix-store GC. No reboot or tests were required.

Normal machine verification after cleanup reports complete for the current
khaki machine generation. The disk-space blockage is resolved; the committed
Calculator reliability fixes still require a fresh combined deployment plan
and live activation. Audit files in dom0:
/var/log/airlock/pool-gc-baseline-2026-10-12.json,
/var/log/airlock/pool-gc-2026-10-12.json and
/var/log/airlock/pool-gc-summary-2026-10-12.json.
Verification: .airlock-logs/calculator-launch-2026-10-12/verify-after-pool-gc.log.
