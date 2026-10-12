# Prevent undeployed candidate accumulation

Status: proposed; automatic expiry and reclamation are not implemented.

Prepared candidates should be temporary, rather than accumulating as permanent
retention roots when an update is never applied. Preserve the useful plan/apply
handoff: apply should reuse the exact candidate reviewed in the plan.

## Tasks

- Define candidate lifecycle states: under review, deploying, selected,
  cancelled, abandoned and awaiting recovery after an interrupted deployment.
- Release cancelled candidates' retention references immediately through the
  existing coordinator. Expire abandoned candidates after a short, configurable
  review period; choose and document the default.
- Protect active deployments and interrupted operations requiring recovery.
  Never expire selected, running, rollback or recovery generations, or override
  existing retention protections.
- Cover every reference created during preparation, including pending roots,
  transport/admission references, retained pool generations and builder views.
  Dropping only a candidate record can leave its artifacts retained elsewhere.
- Reclaim exclusively unused artifacts through the existing managed storage
  maintenance mechanisms. Preserve objects shared with remaining generations.
- Preserve compact audit receipts separately from heavyweight retained artifacts.
- Make expiry bounded, coordinated with deployment locking and observable;
  provide a preview and record why protected candidates were skipped.
- Reject apply of an expired candidate clearly and require a fresh plan.

## Acceptance

Repeated plan/cancel and plan/abandon cycles do not cause unbounded retained
storage growth. Plan/apply reuse works during the review period, and running,
rollback, recovery and interrupted-deployment artifacts survive expiry. Report
both removed retention references and actual reclaimed bytes.

On 2026-10-12, pruning six unused candidates removed their pending roots but
increased available disk space by only 1.57 MiB; no broader artifact collection
was performed. This illustrates why the complete retention/reference lifecycle
and physical reclamation must be addressed together.

Related: [machine generation terminology](machine-generation-terminology.md),
[log retention](log-retention.md), and the
[Calculator incident](../incidents/calculator-launch-2026-10-12.md).
