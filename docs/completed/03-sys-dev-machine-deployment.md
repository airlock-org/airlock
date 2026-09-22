# Deploy a whole-machine release from sys-dev

Status: completed and physically accepted on 2026-09-22. Depends on
[building and admitting a guest in sys-dev](01-sys-dev-machine-build.md)
and [guest activation/rollback](02-sys-dev-guest-activation-rollback.md).

Extend the existing development-only sys-dev-to-Dom0 coordinator transport so
a clean, committed `airlock-machine` checkout in sys-dev can build a complete
machine release, submit it for plan/apply, and retain the existing journalled
status, verification, recovery, and rollback behavior. Dom0 remains the
authority for validation and activation; this does not introduce a general
Dom0 shell or production control interface.

The image now provides `airlock-sys-dev-machine-deploy`. From a clean committed
checkout in sys-dev, use `build` for a local release, `plan` for a no-activation
Dom0 plan, and `apply` to start the journalled coordinator after confirmation.
Use `status`, `verify`, `resume`, and `rollback` for the existing coordinator
workflow. Only closure paths missing from Dom0's Nix store cross the qrexec
bridge.

## Acceptance

1. A release built from sys-dev records its source revision and lock hash and
   produces a no-activation plan through the existing coordinator.
2. Apply, reboot-required reporting, post-reboot verification, and rollback
   retain their current semantics.
3. A test release changes only one declared target, without requiring a parent
   `airlock` checkout or development-host build artifacts on sys-dev.

Local tests cover committed provenance, private-overlay access, missing-path
transfer, and the no-activation plan boundary.

## Physical acceptance

A clean checkout in fresh-image sys-dev built revision `a82bdd1`, produced a
no-activation Dom0 plan, applied it through the detached coordinator, and
verified the selected Dom0 and guest generations after reboot. Transaction
`1790084751462767355` then restored the booted-image baseline and verified it
after a second reboot.

Sys-dev subsequently committed `ba8aa94`, adding only an acceptance marker to
usr-home. Against the managed `a82bdd1` release, its plan reported only
usr-home in `changed`, `affected`, `stage`, and `stop`. Transaction
`1790086780417620020` applied that release, verified the exact usr-home system,
and completed after reboot; the marker was read successfully from the running
guest. No parent checkout or development-host build artifact was used.
