# Nixome

Nixome is a proof-of-concept path toward a Nix-native Qubes OS architecture.
The first milestone is intentionally narrow: run the Qubes control stack with
NixOS as Xen dom0 while retaining the existing Qubes architecture and code.

This repository currently vendors a local source fork of
[`CertainLach/nixos-qubes`](https://github.com/CertainLach/nixos-qubes) under
`vendor/nixos-qubes` and consumes it as a flake input. Nixome adds a thin layer
for:

- a POC NixOS dom0 configuration,
- NixOS Qube template artifacts for config-system experiments,
- a first declarative Qubes topology reconciler,
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
`uname -a`, then tries to detect a Qube GUI window in dom0. By default, Qubes
RPM inputs are cached on the host under `.nixome-cache/rpms/` and copied into
the nested dom0 under `/var/cache/nixome/qubes-rpms/` before import. The
template RPM is about 2 GB. Logs and JSON results are written under
`.nixome-logs/`.

For the first NixOS-template topology:

```bash
make nixos-templates-dry-run
make vm-gui-config-image-dry-run
make vm-config-reconcile
make vm-config-smoke
```

That path builds a dom0 image whose Nix config declares `config-vault` and
`config-mod`, imports their NixOS template sources through Qubes'
`qvm-template-postprocess` during boot-time topology reconcile, applies simple
prefs/features/services with `nixome-qubes-reconcile`, boots both NixOS AppVMs
with the normal
dom0-provided Qubes VM kernel, runs `uname -a` through qrexec in each VM, and
checks that both role repositories exist before `config-mod` validates and
exports a committed config candidate which dom0 then archives into
`config-vault`. See
[docs/config-system.md](/home/user/dev/nixome/docs/config-system.md).
The config topology image keeps local build size down by sharing one NixOS
guest root image between the two TemplateVM source directories.

Generated writable VM overlays and VM build out-links live under `.nixome-vm/`.
If local image builds fail because the builder runs out of temporary disk
space, remove only those generated VM artifacts with:

```bash
make clean-vm-disks
```

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
- `modules/qube-guest.nix`, `pkgs/nixome-qubes-template`, and
  `modules/qubes-topology.nix` are the first pieces for NixOS-backed Qubes
  templates and declarative topology reconciliation.
- The two config-system TemplateVM source directories currently share one
  NixOS guest root image to keep nested smoke builds practical; AppVM roles and
  qrexec policy provide the current separation.
- `machines/test.nix` is the first concrete NixOS dom0 configuration.
- `tests/` contains layered feedback scripts.
- `docs/` records upstream findings and the next test boundary.

## Status

The Fedora-template nested GUI/qrexec smoke path works on a host with KVM and
nested virtualization. The NixOS-template `config-vault`/`config-mod` path
now evaluates, builds template artifacts, imports/creates the declared
TemplateVMs and AppVMs, boots both NixOS AppVMs under nested Xen, proves qrexec
with `uname -a`, and verifies the config candidate validation/archive flow.
Production security validation still requires a physical Xen-capable machine.
