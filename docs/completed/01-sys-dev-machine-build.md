# Build guest updates from airlock-machine in sys-dev

Status: completed; physical build, reuse and inactive admission validated on
2026-09-18 with a freshly flashed image containing machine revision `3b600a1`.

Put a usable standalone `airlock-machine` Git checkout on the physical machine
in `sys-dev`, then build a small guest configuration change there and send its
closure through the [existing pool admission path](../completed/sys-dev-pool-admission.md).
This is a completed step toward [sys-ctrl](../todo/05-sys-ctrl.md).

## Scope

- Provide a repeatable way to seed and update a persistent checkout at
  `/home/user/dev/airlock-machine`, initially at the machine revision pinned by
  the parent repository. USB provisioning includes a depth-one Git checkout,
  restored automatically into sys-dev's persistent home on first boot; no
  development-host transfer is required after boot. The current source has no Git
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
[guest activation and rollback trial](../todo/02-sys-dev-guest-activation-rollback.md).
Creation of `sys-ctrl` remains later work. The checkout must remain a
reproducible copy of committed machine source;
do not leave the only implementation of a fix on the live machine.

The intended operator loop after flashing an image containing this change is:

```bash
ssh sys-dev airlock-sys-dev-machine-source status
ssh sys-dev airlock-sys-dev-machine-build preflight
ssh sys-dev airlock-sys-dev-machine-build build
ssh sys-dev airlock-sys-dev-machine-build admit usr-home
```

Commit a small change in `/home/user/dev/airlock-machine` before building.
`make sys-dev-machine-source DEPLOY_TARGET=sys-dev` remains an optional explicit
source update from the development host, not a first-boot prerequisite.
The last command is for a real, committed `usr-home` configuration change and
returns an inactive pool generation. It does not activate that generation.

## Acceptance

1. On a freshly built/flashed image, follow the documented setup to obtain the
   checkout in sys-dev and verify its revision and required submodules. Confirm
   the checkout and private store survive a whole-machine reboot. This replaces
   the isolated sys-dev restart criterion by agreement with the user.
2. Build a small committed guest change inside sys-dev without depending on
   the parent repository. Demonstrate a second build reuses existing results.
3. Admit that real closure through qrexec and verify the response identifies a
   retained, inactive generation; the current generation stays unchanged.

Implement reusable guest changes in `airlock-machine`, and provisioning/lab
integration in the parent repository. Commit both and update the submodule pin.
Use pytest for relevant regression coverage and `make test-quick`; record live
build/admission evidence and remaining gaps under `.airlock-logs/`. Follow
[machine-deployment.md](../machine-deployment.md) for physical lab work.

## Physical result

The image-provided checkout was shallow and clean. Test commit
`a3ac6fa5d2df6d4c47324f243601e149da574ec3` adds an `/etc` marker for usr-home.
Its first build took 61 seconds; a second returned the identical system path in
about one second with no build/download output. The resulting system is
`/nix/store/vk48zz9l0p3a6vdbvigv00xp9hh2az42-nixos-system-usr-home-26.11`.

Admission created retained, inactive generation 2: 4 objects transferred
(434,176 bytes), 978 reused, 982 total. Dom0's selected generation stayed at 1;
the running guest remained `6ljf4v27s57grp993852yy16hnd9cwxx-nixos-system-usr-home-26.11`.
The receipt records source revision and lock hash; dom0's manifest confirms
retention and committed status. Logs, receipt, baseline and verification are in
`.airlock-logs/pool-trial-20260918T223925Z/`.

Earlier full-machine reboot testing preserved the checkout, builder base and
private build output. That evidence satisfies persistence for this milestone;
the black-screen incident during an isolated sys-dev restart remains unresolved.
The test branch and inactive generation are retained for the next trial.
Activation and rollback have not been tested by this acceptance run.
