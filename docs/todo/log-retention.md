# Bounded log retention

Status: proposed; no cleanup or configuration change applied.

The 2026-10-12 physical lab inspection found approximately 569 MiB of logs in
dom0, including 252 MiB of system journals and 296 MiB of Airlock logs. Logs need
bounded retention, although removing them will not resolve the larger buildup
of retained deployment generations.

## Proposed policy

- Keep routine system and deployment logs for 14 days, with explicit size caps.
- Preserve evidence for failed deployments and unresolved incidents until the
  investigation is complete; define how that evidence is marked and released.
- Preserve release manifests, rollback bindings, pruning receipts and required
  deployment audit records separately from disposable diagnostic logs.
- Keep the Calculator launch evidence and unresolved AMD GPU incident logs
  while those investigations remain open.

## Implementation tasks

- Inventory log locations in dom0, application VMs, the controller and the
  development workspace. Distinguish diagnostic logs from operational state.
- Choose size caps and decide whether resolved incident evidence should be
  archived before expiry. Account for deployment collectors' existing limits.
- Declare journal limits and scheduled Airlock log cleanup in Nix, using the
  existing systemd mechanisms and deployment route.
- Make cleanup bounded and observable, with a reviewable preview. Protect logs
  being written by active deployments and explicitly retained investigations.
- Document retention defaults, evidence preservation and recovery procedures.

## Acceptance

Expired routine logs are removed automatically and storage stays within the
chosen limits. Active deployment logs and retained incident evidence survive
cleanup. Release verification, rollback and recovery remain functional after
diagnostic logs expire. Record reclaimed bytes separately from release/store
cleanup; neither operation should silently delete the other's protected data.
