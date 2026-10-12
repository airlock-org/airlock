# Consistent machine generation terminology

Status: proposed; no updater interfaces or stored records renamed yet.

The operator-facing rollback unit is a whole-machine snapshot: the dom0 system,
the selected VM systems and the Qubes configuration connecting them. Calling
this a "generation" in some places and a "machine release" in others caused
confusion about what rollback restores and how many rollback choices exist.

## Proposed vocabulary

- **Machine generation:** the whole-machine snapshot and rollback unit.
- **Dom0 system generation / VM system generation:** a Nix system version for
  the named component. Several machine generations can reuse it.
- **Prepared candidate:** a built or admitted proposed machine generation that
  has not been selected. Preparation does not establish a successful deployment.
- **Deployment transaction:** an attempt to apply, resume or roll back a machine
  generation; it is not another generation.
- **Internal retention record:** a record/reference keeping artifacts available.
  Its count is not the number of distinct machine generations or rollback choices.

Avoid "release" for the whole-machine snapshot in operator-facing language.
Reserve it for an actual software release when that meaning is intended.

## Tasks

- Inventory terminology in the updater CLI, status output, notifications,
  boot menu, documentation and cleanup reports.
- Use "machine generation" consistently in operator-facing explanations and
  commands. Identify component generations explicitly rather than using an
  unqualified "generation" for both scopes.
- Report prepared candidates, distinct machine generations, retained references
  and selectable rollback choices separately; do not present an internal record
  count as a count of rollback choices. Likewise, a distinct dom0 system count
  is not a distinct machine-generation count.
- Review internal names such as `release`, `release-list` and release manifests.
  Decide whether to rename them or retain implementation names behind consistent
  presentation. Document any intentional implementation terminology explicitly.
- Plan compatibility for existing CLI consumers, JSON schemas, on-disk records,
  retention roots and rollback bindings before changing those interfaces.
- Update current documentation and examples. Preserve historical command names
  where needed to interpret incident evidence, marking them as historical.

## Acceptance

An operator can identify what a machine generation contains, distinguish it from
a VM system generation and a prepared candidate, and understand exactly which
rollback choices a cleanup would remove. Terminology changes preserve existing
selection, verification, retention, rollback and recovery behavior.
