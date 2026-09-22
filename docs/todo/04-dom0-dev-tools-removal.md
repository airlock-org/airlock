# Remove Dom0 development tools through sys-dev deployment

Status: implementation committed; physical acceptance pending. Depends on
[sys-dev guest activation/rollback](../completed/02-sys-dev-guest-activation-rollback.md)
and [sys-dev whole-machine deployment](../completed/03-sys-dev-machine-deployment.md).

Remove the development-only Dom0 Nix packages (`codex`, `git`, and `zellij`)
from `airlock-machine/lifecycle/misc/modules/dom0.nix`. Make and commit that
small source change in sys-dev's standalone `airlock-machine` checkout, build
the whole-machine release there, and submit it through the existing Dom0
coordinator using the new narrow sys-dev deployment capability.

Do not remove mutable user-home installations (such as npm-global Codex) as
part of this Nix-generation change.

## Acceptance

1. The sys-dev checkout is clean, committed, and is the recorded source of the
   planned release.
2. The plan shows only the intended Dom0 package removal and requires a reboot;
   no guest generation is activated unnecessarily.
3. After reboot, Dom0 is running the selected generation and its Nix system
   path no longer contains those packages. The separate whole-machine trial
   already physically verified host rollback, so this trial does not repeat it.
