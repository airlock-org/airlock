# Nixome

Nixome is a proof-of-concept path toward a Nix-native Qubes OS architecture.
The first milestone is narrow: run the Qubes control stack with NixOS as Xen
dom0 while retaining Qubes' Xen, qrexec, QubesDB, policy, libvirt, and VM
lifecycle machinery.

This repository vendors a local source fork of
[`CertainLach/nixos-qubes`](https://github.com/CertainLach/nixos-qubes) under
`vendor/nixos-qubes` and consumes it as a flake input. Nixome adds a thin
integration layer for:

- a POC NixOS dom0 configuration,
- closure-backed NixOS Qubes domains,
- conventional Fedora/Debian TemplateVM/AppVM topology support,
- typed Nix topology and system-plan schemas,
- explicit dom0 setup/apply/status commands,
- config authority VMs for staging, validation, and archival,
- local checks and nested Xen/Qubes integration feedback.

## Repository Layout

- `lifecycle/`: image, bootstrap, runtime, and shared lifecycle support.
- `lifecycle/image/`: image build profiles, overlays, and image-only tools.
- `lifecycle/bootstrap/`: first-boot wiring and the starting runtime config bundled into an image.
- `lifecycle/bootstrap/initial-runtime-config/`: starter runtime config that `config-vault` turns into the first current baseline.
- `lifecycle/bootstrap/tools/`: bootstrap-only first-boot and handoff tools.
- `lifecycle/runtime/`: code used after bootstrap to operate the running system.
- `lifecycle/runtime/tools/`: runtime-only operational packages and scripts.
- `lifecycle/misc/`: shared schemas, NixOS modules, libraries, cross-stage tools, and local test helpers.
- `docs/`: design notes, config authority details, and test documentation.
- `vendor/`: upstream source inputs carried locally for the POC.

## Commands

```bash
make test-quick
make test-warm
make test-slow
```

Runs the test suites. `test-quick` is the local feedback loop and runs
parallel-safe checks by default. `test-warm` reuses one warm nested dom0 and
runs ordered staged VM coverage. `test-slow` runs cold VM coverage with
parallel VM lanes where safe, followed by serial boot-activation coverage.

```bash
make build
```

Builds `.#nixosConfigurations.test.config.system.build.toplevel`.

```bash
make vm-gui-config-image
make vm-gui-config-run
```

Builds and launches the current nested GUI dom0 image. The nested image uses a
throwaway `nixome` user with password `nixome` and loopback-only SSH
forwarding on port `2222`.

```bash
make vm-ssh
```

Connects to the nested dom0. The helper supplies the default password itself
and ignores stale loopback host keys from previous images.

```bash
make vm-config-flow
make vm-config-boot-flow
make vm-config-boot-activate-flow
```

Runs the config authority workflow in nested Xen/Qubes. The flow starts from a
fresh nested dom0 disk, runs the dom0 setup command, boots `config-vault`,
`config-staging`, and `config-validator`, proves qrexec, modifies and validates
a staged config, has dom0 apply the validator-approved topology/system plan,
and records the deployment in `config-vault`.

`make vm-config-boot-flow` installs the validated dom0 system as the next boot
generation instead of switching the running generation. `make
vm-config-boot-activate-flow` then boots the same nested disk a second time and
verifies that the staged generation became active.

```bash
make vm-nix-closure-flow
```

Runs the narrower real Xen/Qubes proof for a representative closure-backed
NixOS domain.

## Current Architecture

The normal domain split is:

```text
NixOS domain
    -> nix-closure backend

Fedora/Debian/conventional Qubes guest
    -> existing TemplateVM/AppVM backend
```

The config authority VMs are now independent closure-backed NixOS domains:

```text
config-vault
config-staging
config-validator
```

Each config VM is a Qubes `StandaloneVM` with:

- an explicit NixOS system closure,
- a small writable root image,
- a separate read-only `store` volume mounted at `/nix/store`,
- private persistent state,
- normal Qubes qrexec/policy/network controls.

`config-staging` and `config-validator` run candidate Nix checks through
`nixome-config-nix`, backed by private writable state, so their own system
`/nix/store` remains read-only.

The old NixOS TemplateVM path for config domains has been removed. Generic
Qubes TemplateVM support remains for Fedora, Debian, and other conventional
guests.

See [docs/config-system.md](docs/config-system.md) for the config authority
model and [docs/testing.md](docs/testing.md) for the test layers.

## Generated Data

When `/run/media/user/data` is mounted, generated writable VM overlays, logs,
temporary files, Qubes RPM/template downloads, and Nixome Nix build outputs
default to:

```text
/run/media/user/data/nixome/
```

Source code, Nix files, scripts, docs, and Git history stay in this repository.
The Nixome Nix store is a chroot local store rooted at:

```text
/run/media/user/data/nixome/nix-root
```

Logical store paths still look like `/nix/store/...`; the bytes live under the
data-disk root. If `/run/media/user/data` is not available, Nixome refuses to
use the host `/nix/store` by default. Set `NIXOME_DATA_ROOT` to another
data-backed path, or set `NIXOME_USE_HOST_NIX_STORE=1` for an explicit
one-off host-store command.

Useful cleanup commands:

```bash
make disk-usage
make gc-auto
make gc-roots
make gc-dry-run
make gc
make clean-vm-disks
make clean-vm-workdirs
make clean-logs
make clean-cache
```

`make gc` operates only on the separate Nixome store and refuses to run against
the host `/nix/store`.

## Status

The nested GUI/Qubes path has proven the Qubes control services, Fedora
TemplateVM qrexec, closure-backed NixOS domain boot, read-only store volume
mounting, config staging/validation/promotion, Nixome-owned qrexec policy
generation, and dom0 generation switching under nested Xen.

Production security validation still requires a physical Xen-capable machine.
