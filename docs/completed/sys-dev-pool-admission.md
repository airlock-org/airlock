# Send guest packages from sys-dev to the shared pool

Status: implemented in the pinned `airlock-machine` component on 2026-09-17.
Physical admission was validated on 2026-09-17; the nested flow remains
available as `vm-pool-admission-flow` for broader activation and rollback tests.

A small first step toward [sys-ctrl](../todo/sys-ctrl.md): let `sys-dev` prepare and
send guest package artifacts to the existing Python `airlock-qubes-storage`
manager in dom0. Later, `sys-ctrl` can take over this sender role.

## Scope

- Add an artifact preparation command in `sys-dev`, using built or imported
  guest packages. Record the source commit, lock-file hash, exact guest system,
  package identities, references, and content checksums.
- Add a versioned qrexec admission service, initially restricted by Qubes policy
  to `sys-dev` and explicitly permitted guest targets. Accept package artifacts,
  not shell commands or caller-selected filesystem destinations.
- Extend the existing pool manager to validate transferred contents and closure
  completeness, reuse admitted objects, and publish a retained, inactive
  generation only after validation succeeds. Return its identity and result.
- Reuse existing pool storage, VM views, retention, GC, and compaction. Make
  interrupted transfers/admissions safe to retry and clean up temporary data
  after success or recovery.
- Use the existing machine coordinator for activation and rollback. Admission
  alone must not restart a VM or report a deployment as complete.

Keep this step to artifact preparation, transport, and admission. Creating
`sys-ctrl`, moving the full build/configuration workflow, and retiring
`sys-config-*` belong to the broader plan. Existing development shell access
is separate from this service and is not its transport.

## Acceptance tests

1. **Pytest with a tiny temporary pool:** verify transferred contents, closure
   completeness, and object reuse on retry. Reject unauthorized callers,
   malformed requests, corrupt or missing packages, and unsafe archive paths.
   Interrupt transfer and admission; no incomplete generation becomes usable.
2. **One nested Qubes/Xen flow:** send an update from `sys-dev` through qrexec,
   activate it with the coordinator, verify the guest system, and roll back.
   Check that a sentinel in the guest's private data survives.
3. **Cleanup and retention:** after success and interrupted-operation recovery,
   temporary files are removed. GC preserves running and retained rollback
   generations; resending identical packages adds no duplicate pool objects.

Use `make test-quick` and record integration evidence under `.airlock-logs/`.
Implement runtime changes in `airlock-machine`; keep lab integration here and
update its submodule pin. Follow [machine-deployment.md](../machine-deployment.md)
for a later physical trial; local/nested results do not establish live success.
