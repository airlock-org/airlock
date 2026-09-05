# Nixome

Nixome is a proof-of-concept path toward a Nix-native Qubes OS architecture.
The first milestone is intentionally narrow: run the Qubes control stack with
NixOS as Xen dom0 while retaining the existing Qubes architecture and code.

This repository currently vendors a local source fork of
[`CertainLach/nixos-qubes`](https://github.com/CertainLach/nixos-qubes) under
`vendor/nixos-qubes` and consumes it as a flake input. Nixome adds a thin layer
for:

- a POC NixOS dom0 configuration,
- local module/evaluation checks,
- host capability reporting,
- a development-only dom0 observation command,
- an opt-in qrexec smoke test for real Xen dom0 targets.

## Commands

```bash
make test
```

Runs the local feedback loop. It evaluates the flake/module contract, builds
the lightweight observation package, writes host capability JSON, and skips the
real Qubes smoke test unless explicitly enabled.

```bash
make build
```

Builds the `.#nixosConfigurations.test.config.system.build.toplevel` closure.
This is the first full-system build target for a Xen dom0 test machine.

```bash
make vm-image
make vm-run
```

Builds and launches a nested QEMU/KVM qcow2 image. This is for development only:
it uses a throwaway `nixome` user with password `nixome` and loopback-only SSH
forwarding on port `2222`.

Inside the VM, use `nixome-observe-dom0` for JSON status and
`nixome-qubes-smoke` after installing a Qubes VM kernel/template.

For a normal graphical desktop in a QEMU window:

```bash
make vm-gui-image
make vm-gui-run
```

This builds the GUI VM profile with X11, LightDM, XFCE, and Qubes GUI-related
dom0 packages enabled. It is still a development VM, not a production Qubes
desktop.

For an automated nested GUI boot/control-plane check:

```bash
make vm-gui-check
```

For the current full nested smoke path:

```bash
make vm-gui-smoke
```

This starts the GUI image, checks core services over SSH, downloads/imports the
default Qubes R4.3 Fedora VM kernel and Fedora 43 XFCE template, runs qrexec
`uname -a`, then tries to detect a Qube GUI window in dom0. The template RPM is
about 2 GB. Logs and JSON results are written under `.nixome-logs/`.

On a booted Xen dom0 target with a Qubes template installed:

```bash
NIXOME_RUN_QUBES_SMOKE=1 make test
```

The Qubes smoke test creates `test-nixome-smoke` if needed, starts it, runs
`uname -a` through `qvm-run --pass-io`, validates stdout, writes logs under
`.nixome-logs/`, and removes the test qube on success if it created it.

## Current Shape

- `flake.nix` pins `nixpkgs` and consumes the local `vendor/nixos-qubes` fork.
  The `nixos-qubes` `nixpkgs` input follows this repository's `nixpkgs` pin.
- `modules/dom0.nix` enables upstream Qubes-on-NixOS dom0 support and keeps the
  upstream security gap explicit with `virtualisation.qubes.secure = false`.
- `nixome-qube-bootstrap`, `nixome-qubes-smoke`, and
  `nixome-qube-gui-smoke` are development-only commands for progressing the
  VM lifecycle, qrexec, and GUI feedback loop.
- `machines/test.nix` is the first concrete NixOS dom0 configuration.
- `tests/` contains layered feedback scripts.
- `docs/` records upstream findings and the next test boundary.

## Status

The local development host is not Xen dom0, so `make test` can validate
evaluation and host detection only. The qrexec milestone requires a physical or
otherwise suitable Xen-capable test machine booted into the generated NixOS
dom0 system with a Qubes-compatible template installed.
