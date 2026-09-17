# Activate and roll back a guest built in sys-dev

Status: planned. Depends on [building and admitting a guest in sys-dev](sys-dev-machine-build.md).

Take a real guest update built from the standalone `airlock-machine` checkout
in sys-dev, activate its admitted pool generation, and prove that rollback
restores the previous system while preserving private data.

## Scope

- Use `usr-home` for the first trial, with a small committed system marker that
  distinguishes the new build. Record the current release, guest system, and
  pool generation, and retain that baseline for rollback. Never activate the
  synthetic single-object artifacts used for admission smoke tests.
- Connect the admitted generation to the existing machine coordinator's
  plan/apply/verify/rollback workflow. This connection is remaining work; do not
  assume an admission response is already a deployable release. Validate the
  target, system path, source revision, lock hash, and required root/kernel
  artifacts before selecting it.
- Keep whole-machine planning and journalled recovery. The plan must show only
  the intended guest change; reuse its admitted closure without transferring
  the guest packages again. Preserve Qubes lifecycle and dependency handling.
- Document the commands and recovery procedure. Keep this step within the
  existing development administration workflow; creating `sys-ctrl` and a new
  production activation interface belong to the broader plan.

## Acceptance

1. Create a unique sentinel in the guest's persistent home. Apply the update,
   restart the guest through the coordinator, and verify the running system,
   marker, selected pool generation, and unchanged sentinel.
2. Reboot the machine and verify that the selected guest update and sentinel
   survive. Record whole-machine verification separately from admission.
3. Roll back through the coordinator, then verify the prior running system,
   prior marker state, retained generation, and unchanged sentinel. Confirm the
   rollback selection also survives a reboot.
4. Exercise a failed/interrupted activation in automated tests and verify that
   status and resume/rollback remain usable. Admission must never count as a
   successful activation when the guest has not booted the intended system.

Implement and commit runtime changes in `airlock-machine`, then update the
parent submodule pin and lab integration. Use pytest and `make test-quick`,
extend the existing nested flow where practical, and follow
[machine-deployment.md](../machine-deployment.md) for physical testing. Record
machine-readable results under `.airlock-logs/` and remove the test sentinel
after verification; preserve retained generations through the normal pool
retention/GC mechanisms.
