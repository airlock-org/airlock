# 10: Simplify the boot menu

Status: **planned** (2026-09-30).

## Agreed behavior

- Normal Airlock entries appear from newest installed generation to oldest.
  The newest is at the top and selected by default on every boot.
- Normal entry titles use `Airlock (<time>-<short-sha-of-commit>)`, for example
  `Airlock (2026-09-30_14:30-a1b2c3d)`.
- Use the release source commit's timestamp, formatted in UTC, and the first
  seven characters of its SHA. Labels remain stable across rebuilds.
- Recovery entries appear below normal generations and are clearly labeled,
  for example `Airlock (recovery, 2026-09-30_14:30-a1b2c3d)`.
- Manually choosing an older generation boots it once. The next boot defaults
  to the newest again; rollback does not establish a persistent older default.

## Implementation

Paths below are relative to `airlock-machine/`.

- [ ] Carry committed source revision and timestamp into boot metadata for image
  creation and normal deployments. Use the corresponding release's metadata,
  not the running generation's identity or the wall clock when formatting titles.
- [ ] Apply the same presentation rules in
  `lifecycle/misc/tools/scripts/boot-menu.py` and
  `lifecycle/runtime/tools/pkgs/airlock-qubes-storage/src/airlock_qubes_storage/host_artifacts.py`.
  Remove the special presentation of the currently selected normal generation.
- [ ] Sort numerically by boot-menu generation, descending, independently of
  titles and source timestamps. Keep one normal entry per retained generation
  and suppress duplicate native/retained entries.
- [ ] Set the default to the first normal entry. Align deployment, rollback,
  recovery selection, and bootloader preference handling with this rule, while
  preserving the ability to manually boot older generations and recovery.
- [ ] Keep generation/role/system identity in explicit internal metadata.
  Update classification that currently depends on `version Generation ...` or
  title text, and prevent automatic long version suffixes from cluttering titles.
- [ ] Format retained entries before sealing their checksums; retain boot artifact
  verification, independent recovery, and generation-specific writable state.

## Acceptance

- [ ] Add pytest coverage for numeric ordering (including generations 2 and 10),
  an older selected generation, labels, default selection, recovery placement,
  duplicate suppression, and repeated menu generation.
- [ ] Cover fresh-image and deployment/retained-entry paths; identical source
  labels must not merge distinct generations or change their order.
- [ ] Pass `make test-quick`, then verify the physical menu and default boot,
  including a manual older-generation boot followed by the newest default.
  Use the [normal deployment workflow](../machine-deployment.md) where possible.
- [ ] Record a brief acceptance summary here.
