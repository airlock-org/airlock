# Deploy a whole-machine release from sys-dev

Status: planned. Depends on [building and admitting a guest in sys-dev](../completed/01-sys-dev-machine-build.md)
and [guest activation/rollback](02-sys-dev-guest-activation-rollback.md).

Extend the existing development-only sys-dev-to-Dom0 coordinator transport so
a clean, committed `airlock-machine` checkout in sys-dev can build a complete
machine release, submit it for plan/apply, and retain the existing journalled
status, verification, recovery, and rollback behavior. Dom0 remains the
authority for validation and activation; this does not introduce a general
Dom0 shell or production control interface.

## Acceptance

1. A release built from sys-dev records its source revision and lock hash and
   produces a no-activation plan through the existing coordinator.
2. Apply, reboot-required reporting, post-reboot verification, and rollback
   retain their current semantics.
3. A test release changes only one declared target, without requiring a parent
   `airlock` checkout or development-host build artifacts on sys-dev.
