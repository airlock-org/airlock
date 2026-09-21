# Activate and roll back a guest built in sys-dev

Status: completed, 2026-09-22. Build, admission, activation, reboot persistence,
rollback, rollback persistence and sentinel cleanup passed. An intermittent
boot stall remains unresolved; see the physical results below.
Prerequisite [building and admitting a guest in sys-dev](01-sys-dev-machine-build.md) is complete.

Take a real guest update built from the standalone `airlock-machine` checkout
in sys-dev, activate its admitted pool generation, and prove that rollback
restores the previous system while preserving private data.

## Starting point

The trial must be reproducible from a freshly flashed image. Use its included
shallow `airlock-machine` checkout in sys-dev to commit a small `usr-home`
marker change, build it, and admit it through the completed 01 workflow.
Record the returned generation and receipt; do not hard-code a generation
number or depend on a test branch, receipt, or pool state from an earlier boot.

An existing test admission may be reused after checking its source revision,
lock hash, system path, retention and inactive status against the live pool.
It is optional: after a reflash, recreate the change and admission, and capture
the new machine's baseline for rollback.

## Scope

- Use `usr-home` for the first trial, with a small committed system marker that
  distinguishes the new build. Record the current release, guest system, and
  pool generation, and retain that baseline for rollback. Never activate the
  synthetic single-object artifacts used for admission smoke tests.
- Connect the admitted generation to the existing machine coordinator's
  plan/apply/verify/rollback workflow. The coordinator derives a persistent
  whole-machine candidate from the currently selected release and changes only
  the receipt's target guest. Validate the
  target, system path, source revision, lock hash, and required root/kernel
  artifacts before selecting it.
- Keep whole-machine planning and journalled recovery. The plan must show only
  the intended guest change; reuse its admitted closure without transferring
  the guest packages again. Preserve Qubes lifecycle and dependency handling.
- Document the commands and recovery procedure. Keep this step within the
  existing development administration workflow; creating `sys-ctrl` and a new
  production activation interface belong to the broader plan.

System-only admission now inherits the target's root and kernel identity from
its current committed pool generation. The coordinator compares the receipt
with the dom0-owned generation manifest, requires sys-dev/qrexec approval,
passed verification and retention, and binds the exact generation into the
candidate. Boot setup preserves that binding, so it does not need the original
closure descriptor in sys-dev after activation.

After recording the receipt path printed/stored by the build workflow, preview
and apply it from the development checkout:

```bash
make machine-guest-plan DEPLOY_TARGET=sys-dev \
  ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
make machine-guest-apply DEPLOY_TARGET=sys-dev \
  ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
make machine-status DEPLOY_TARGET=sys-dev
make machine-verify DEPLOY_TARGET=sys-dev
```

`machine-guest-plan` copies only the small receipt, validates it against the
pool and shows the complete machine plan. `machine-guest-apply` asks before
launching the existing detached, journalled coordinator. Use the normal
`machine-resume` and `machine-rollback` commands for recovery.

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

Local pytest coverage exercises exact-generation binding, rejected identity,
provenance, verification and retention, persistence of the binding at boot,
and both resume and rollback after an interrupted activation. Physical evidence
for the acceptance criteria is recorded below.

Physical trial, 2026-09-21: the fresh image built committed usr-home marker
change `aa19d1e` in sys-dev; a repeat build reused the result. Admission created
retained generation 2, transferring 4 objects (434,176 bytes) and reusing 978.
The guest plan correctly limited changes to usr-home, but apply failed during
configuration: the derived candidate updated `backend.system.source` while
leaving `airlock.system=` in the kernel arguments pointing at the baseline
system. Setup rejected the mismatch after pool selection and before guest
startup. This required a candidate boot-argument fix and regression coverage
before retrying. Evidence is under
`.airlock-logs/activation-trial-20260921T221830Z/`. This initial failed attempt
did not establish successful activation or reboot persistence.

Coordinator rollback recovered the original running usr-home system and pool
generation 1. Whole-machine verification returned `complete`, and the persistent
home sentinel hash matched the baseline. The marker was absent as expected.
The test checkout, retained admission, and sentinel were preserved for the retry.

The fix in machine commit `6785324` updates the explicit `airlock.system=` boot
argument, rejects inconsistent baseline arguments, and gives corrected candidates
their own identity. `UPDATE_COORDINATOR=1` on the guest plan/apply commands built
and transferred the committed coordinator (only two missing Nix paths) without
reflashing. Its package is rooted against GC and recorded in the candidate.
The retry reused generation 2 and reached `complete`; the running system was
`rhz04xhp8cvi31vp9711qs0rjzlja0pd-nixos-system-usr-home-26.11`, the marker was
present, and the sentinel hash matched. Evidence is in
`fixed-activation-observation.json` in the trial directory. Subsequent checks
used the same installation without reflashing.

The subsequent normal `Airlock` boot preserved the selected update. On boot
`f0a51e62-ad34-4129-96d9-d2d69fb4599e`, whole-machine verification returned
`complete`, usr-home ran generation 2 with its marker, the sentinel matched,
and dom0 had no failed units. See `activation-reboot.json` in the trial directory.

Coordinator rollback from that successfully booted update restored generation 1
and the original running system, with the marker absent and the sentinel
unchanged. Verification returned `complete`, dom0 had no failed units, and
generation 2 remained retained but inactive. See `rollback-before-reboot.json`.

Rollback persistence passed on boot `61453301-1808-4452-9d28-31486da9cb52`:
generation 1 and the original running system matched, the marker was absent,
the sentinel hash was unchanged, generation 2 remained retained and inactive,
and whole-machine verification returned `complete` with no failed dom0 units.
See `rollback-after-reboot.json`. The sentinel was then hash-checked, removed,
and confirmed absent. The test commit and retained generations remain available.
Local checks passed: 112 machine quick tests and 375 parent quick tests
(23 skipped).

One intervening boot stalled during sys-dev/sys-gui startup and required a
forced shutdown. Its saved topology setup result passed using the rollback
baseline and generation 1; the dom0 journal ended about 85 seconds into boot,
and the guest console logs contained no readable output from that boot. The
user reported waiting 5–10 minutes. A subsequent cold boot succeeded without
configuration changes. Cause is unknown; neither a rollback-induced failure
nor its exclusion has been established. The successful trial demonstrates the
update/rollback workflow, not general boot reliability. Diagnostic copies are
under `.airlock-logs/usb-boot-inspection/`.
