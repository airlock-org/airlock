# 10: Simplify the boot menu

Status: **in progress** (2026-09-30).

## Agreed behavior

- Normal Airlock entries appear from newest installed generation to oldest.
  The newest is at the top and selected by default on every boot.
- Normal entry titles use `Airlock (<time>-<short-sha-of-commit>)`, for example
  `Airlock (2026-09-30_14:30-a1b2c3d)`.
- Use the release source commit's timestamp, formatted in UTC, and the first
  seven characters of its SHA. Labels remain stable across rebuilds.
- Use the standard 80x25 UEFI console mode to make menu text larger.
- Prune unneeded historical normal generations through the deployed retention
  workflow; preserve independent recovery and protected current/rollback hosts.
- Recovery entries appear below normal generations and are clearly labeled,
  for example `Airlock (recovery, 2026-09-30_14:30-a1b2c3d)`.
- Manually choosing an older generation boots it once. The next boot defaults
  to the newest again; rollback does not establish a persistent older default.

## Implementation

Paths below are relative to `airlock-machine/`.

- [x] Carry committed source revision and timestamp into boot metadata for image
  creation and normal deployments. Use the corresponding release's metadata,
  not the running generation's identity or the wall clock when formatting titles.
- [x] Apply the same presentation rules in
  `lifecycle/misc/tools/scripts/boot-menu.py` and
  `lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_artifacts.py`.
  Remove the special presentation of the currently selected normal generation.
- [x] Sort numerically by boot-menu generation, descending, independently of
  titles and source timestamps. Keep one normal entry per retained generation
  and suppress duplicate native/retained entries.
- [x] Set the default to the first normal entry. Align deployment, rollback,
  recovery selection, and bootloader preference handling with this rule, while
  preserving the ability to manually boot older generations and recovery.
- [x] Keep generation/role/system identity in explicit internal metadata.
  Update classification that currently depends on `version Generation ...` or
  title text, and prevent automatic long version suffixes from cluttering titles.
- [x] Format retained entries before sealing their checksums; retain boot artifact
  verification, independent recovery, and generation-specific writable state.

## Acceptance

- [x] Add pytest coverage for numeric ordering (including generations 2 and 10),
  an older selected generation, labels, default selection, recovery placement,
  duplicate suppression, and repeated menu generation.
- [x] Cover fresh-image and deployment/retained-entry paths; identical source
  labels must not merge distinct generations or change their order.
- [ ] Pass `make test-quick`, then verify the physical menu and default boot,
  including readable text after pruning the unneeded older generations.
  Use the [normal deployment workflow](../machine-deployment.md) where possible.
- [ ] Record a brief acceptance summary here.

## Implementation notes (2026-09-30)

The image embeds the parent image source commit; standalone releases embed the
machine source commit. Both normal and independent recovery carry the same
`boot-source.json` for their corresponding source. Menu entries carry explicit
boot-menu generation, role, system and source comments. Short `Generation N` versions
only distinguish equal labels and preserve old installer compatibility; ordering uses an ascending, fixed-width
complement of the boot-menu generation, separately for normal and recovery.

Retained legacy entries are verified before a journalled presentation migration
updates their entry checksums. Interrupted migrations accept either authenticated
entry file and resume; Xen, kernel and initrd hashes stay fixed. The selected
pool generation remains independent of the persistent menu default. Rollback
and recovery selection keep the newest default; manually choose an older entry
to boot that host. Saved firmware default and preferred entries are cleared when EFI variables are
available; Xen systems without EFI variable access use `loader.conf`.

Compatibility limitation: pre-existing image/deployment generations did not
embed a source timestamp. They receive `Airlock (legacy generation N)` (or the
recovery equivalent), rather than attributing the new source identity to old
artifacts. The operator explicitly declined provenance backfill and requested pruning
unneeded older generations instead. Independent legacy recovery stays available.
New images and releases use the agreed commit labels.

Focused pytest coverage includes generations 2 and 10, identical source labels,
older selection, native/retained duplicates, repeated formatting/sealing,
independent recovery, distinct pool/menu generation numbers, corrupt artifacts,
interrupted migration and stored firmware preference handling. The live release is applied; physical menu/reboot acceptance remains pending.

## Acceptance so far (2026-09-30)

- Machine source **`8048650`** is committed and pinned by the parent repository.
  **`make test-quick`: 733 passed, 25 skipped.** Nix evaluation covers both fresh
  image source metadata and standalone release metadata.
- Normal `make machine-plan DEPLOY_TARGET=sys-dev` built and admitted the release.
  Normal apply reused its prepared token with zero builds and payload transfers.
  Only sys-dev and sys-ctrl restarted. Superseded prepared candidates were not
  activated. No ad-hoc live configuration fix or overlay was used.
- Transaction **`1790759936024504141`** passed typed verification with status
  **`reboot-required`**. All six running guests match their selections;
  usr-browser remains stopped. Selected host is `0hhav9125qp9smac62ys6zmb3qhzp0c1`,
  while running host remains `1r34d0azzpnc70kw5qmppwm3g30hlfcf`.
- Read-only ESP inspection and `bootctl list` confirm normal order
  **12, 11, 10, 8, 6, 5, 4, 3, 2, 1**, followed by independent recovery.
  There is one entry per retained normal generation and no native duplicates.
  `Airlock (2026-09-30_09:08-8048650)` is first/default; `showTitle` contains no
  added version suffix. All previous entries have short legacy-generation
  labels. The normal apply completed its artifact sealing/verification steps.
- Guarded `make gc-auto` completed and skipped collection below its 150 GiB
  threshold. It did not collect the development host's `/nix/store`.
- Follow-up: the operator booted generation 12. Typed verification now reports
  **complete**, with the selected host running and guest generations matching.
  The operator requested pruning old generations and larger menu text;
  provenance backfill is no longer required. Physical confirmation of the
  larger text remains pending.

Evidence: `.airlock-logs/boot-menu-presentation/` contains the final quick-test
reports, plan/apply JSON, menu snapshot, bootctl output, status, and typed
verification before host reboot. The task remains **in progress**.


## Pruning and readable console follow-up (2026-09-30)

The operator declined provenance backfill and requested testing the deployed
release-pruning workflow. Generation 12 booted and typed verification first
reported **complete**. Thirteen eligible release records were then pruned,
including unused prepared candidates and the image's normal baseline. Normal
profile generations 2 through 11 were removed where they remained registered;
retained legacy normal boot artifacts were removed independently. Recovery
remains pinned and present. No ad-hoc live configuration changes were used.
Pruning removes retention references and boot artifacts; it does not claim that
all unreferenced store contents were garbage-collected.

The current and immediate rollback releases stayed protected. After deploying
machine revision **`e196ef8`**, generation 11 became eligible and was pruned.
Repeating generation 2's completed prune preserved the identical audit record.
One plan attempt overlapped that repeat check and was rejected by the deployment
lock; the sequential retry succeeded. Pruning operations otherwise finished.

The larger-text setting is committed in `machine-host.nix` as systemd-boot
`consoleMode = "0"`, the standard 80x25 UEFI console mode. The existing Nix
image/release evaluation tests check this setting in both construction paths.
[Upstream console-mode documentation](https://github.com/systemd/systemd/blob/main/man/loader.conf.xml).
**Quick tests: 733 passed, 25 skipped.** Guarded development-store GC ran and
skipped collection below its threshold.

Normal plan/apply reused the prepared release with zero builds/transfers during
apply and no guest restarts. Transaction **`1790762690675027745`** is verified
**reboot-required**: six running guests match their selections, usr-browser is
stopped, and the selected host `mq1z3vjikp3ynnc4ryrx64f3csplc26y` differs from
the running generation 12 host `0hhav9125qp9smac62ys6zmb3qhzp0c1`.

Final ESP inspection and actual `bootctl list` confirm installed normal order
**13, 12**, then independent recovery. Generation 13 is first/default with title
`Airlock (2026-09-30_09:51-e196ef8)`. There are no legacy normal entries. The
loader contains `console-mode 0`; no saved firmware console override exists.
Bootctl also reports the firmware's stale last-boot entry inventory; those
reported/absent entries have no installed menu files and refresh on reboot.

Still pending: physically confirm the larger text and newest default, boot
generation 13, then run `make machine-verify DEPLOY_TARGET=sys-dev`. Generation
12 remains the protected rollback/running host. Evidence and machine-readable
acceptance: `.airlock-logs/boot-menu-cleanup/`.


Generation 13 follow-up: before the store-preparation work, typed verification
reported transaction `1790762690675027745` **complete**. Running and selected host
both resolve to `mq1z3vjikp3ynnc4ryrx64f3csplc26y`, and six running guests match
their selections; usr-browser is stopped. Evidence:
`.airlock-logs/store-preparation/verify-before.json`. Physical confirmation of
the larger text and manual older-generation/default behavior remains pending.
