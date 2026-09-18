# Build guest updates from airlock-machine in sys-dev

Status: implemented locally on 2026-09-17; image build and physical validation
remain pending.

Put a usable standalone `airlock-machine` Git checkout on the physical machine
in `sys-dev`, then build a small guest configuration change there and send its
closure through the [existing pool admission path](../completed/sys-dev-pool-admission.md).
This is the next small step toward [sys-ctrl](05-sys-ctrl.md).

## Scope

- Provide a repeatable way to seed and update a persistent checkout at
  `/home/user/dev/airlock-machine`, initially at the machine revision pinned by
  the parent repository. Transfer Git history; the current source has no Git
  submodules, and setup must fail clearly if that changes until submodule
  transfer is implemented;
  do not depend on development-host filesystem paths or a parent `airlock`
  checkout. Preserve local edits when updating an existing checkout.
- Supply the tools and documented command needed to evaluate and build from
  that checkout using sys-dev's [pool-backed build store](../sys-dev-build-store.md),
  with a persistent private writable layer for additions. Adapt the
  repository's Nix wrapper to this guest environment without weakening the
  development host's external-store guard. Use the committed `flake.lock`.
- Check available space and input/cache access before building. Reuse existing
  store setup and guarded cleanup; do not resize disks as part of this step.
- Build one real guest system and its complete closure from a small committed
  configuration change, then submit it with `airlock-pool-send`. Record the
  source revision, lock hash, system path, and returned pool generation.

Limit this step to checkout, build, and inactive admission. Follow with the
[guest activation and rollback trial](02-sys-dev-guest-activation-rollback.md).
Creation of `sys-ctrl` remains later work. The checkout must remain a
reproducible copy of committed machine source;
do not leave the only implementation of a fix on the live machine.

The intended operator loop after flashing an image containing this change is:

```bash
make sys-dev-machine-source DEPLOY_TARGET=sys-dev
ssh sys-dev airlock-sys-dev-machine-source status
ssh sys-dev airlock-sys-dev-machine-build preflight
ssh sys-dev airlock-sys-dev-machine-build build
ssh sys-dev airlock-sys-dev-machine-build admit usr-home
```

The last command is for a real, committed `usr-home` configuration change and
returns an inactive pool generation. It does not activate that generation.

## Acceptance

1. On a freshly built/flashed image, follow the documented setup to obtain the
   checkout in sys-dev and verify its revision and required submodules. Confirm
   the checkout and private store survive a sys-dev restart.
2. Build a small committed guest change inside sys-dev without depending on
   the parent repository. Demonstrate a second build reuses existing results.
3. Admit that real closure through qrexec and verify the response identifies a
   retained, inactive generation; the current generation stays unchanged.

Implement reusable guest changes in `airlock-machine`, and provisioning/lab
integration in the parent repository. Commit both and update the submodule pin.
Use pytest for relevant regression coverage and `make test-quick`; record live
build/admission evidence and remaining gaps under `.airlock-logs/`. Follow
[machine-deployment.md](../machine-deployment.md) for physical lab work.
