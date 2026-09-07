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
- staged config-defined NixOS TemplateVM artifacts,
- a first declarative Qubes topology setup command,
- a small typed topology schema for staged Nix config candidates,
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

Use `make vm-ssh` to connect. The helper supplies the default password itself
and ignores stale loopback host keys from previous VM images.

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
RPM inputs are cached on the host under the generated data root and copied into
the nested dom0 under `/var/cache/nixome/qubes-rpms/` before import. The
template RPM is about 2 GB. Logs and JSON results are written under the same
generated data root.

For the first NixOS-template topology and config-authority flow:

```bash
make nixos-templates-dry-run
make vm-gui-config-image-dry-run
make vm-config-setup
make vm-config-flow
make vm-config-boot-flow
make vm-config-boot-activate-flow
make vm-config-template-flow
```

`make vm-config-smoke` remains as a compatibility alias.

That path builds a dom0 image whose Nix config declares `config-vault`,
`config-staging`, and `config-validator`, imports one shared bootstrap NixOS
config-system TemplateVM through Qubes' `qvm-template-postprocess` during an
explicit dom0 topology setup, applies simple prefs/features/services with
`nixome-setup-config`, boots the NixOS AppVMs with the normal dom0-provided
Qubes VM kernel, runs `uname -a` through qrexec in each VM, and checks that the
role repositories exist before `config-staging` exports a committed typed Nix
config candidate. The validator evaluates normalized `.#nixome.topology` and
dry-runs any staged template build references. The default seeded candidate uses
a prebuilt concrete Nix store template source path for `nixome-nixos-work`, so
dom0 can import that template during apply without rebuilding it in the nested
VM. dom0 archives the candidate into `config-vault` and applies the resolved
topology with `nixome-config-apply --apply`. The successful apply is then
recorded under `deployments/` in `config-vault`. The runtime test proves the
staged Nix config re-declares the three config AppVMs, defines a staged
`nixome-nixos-work` TemplateVM artifact, creates a `nixome-work-from-config`
AppVM from that template, runs `uname -a` inside it through qrexec, and
preserves the deployment record. See
[docs/config-system.md](/home/user/dev/nixome/docs/config-system.md).
`make vm-config-boot-flow` runs the same candidate path with
`dom0.nixos.system.switchAction = "boot"`: dom0 installs the next NixOS system
generation, reports a pending reboot, and leaves the running generation
unchanged. `make vm-config-boot-activate-flow` then boots the same writable VM
disk a second time and verifies that `/run/current-system` has advanced to the
staged generation and `nixome-config-status` no longer reports a pending
reboot.
Inside `config-staging`, `nixome/local.nix` can add
`templates.definitions.<template-name>` entries. Each entry provides NixOS
modules for a TemplateVM root plus optional Qubes domain/AppVM defaults; the
candidate flake exposes the resulting template package and topology source
reference. `make vm-config-template-flow` runs the heavier version of the
nested proof: the staged config adds a second TemplateVM definition, dom0 builds
that template package during apply, imports it, creates an AppVM from it, and
checks qrexec in that AppVM.
The config topology image keeps local build size down by using one NixOS
config-system TemplateVM for all three config AppVM roles.

When `/run/media/user/data` is mounted, generated writable VM overlays, logs,
temporary files, Qubes RPM/template downloads, and Nixome Nix build outputs
default to `/run/media/user/data/nixome/`. Source code, Nix files, scripts,
docs, and Git history remain in this repository. The Nix store is a chroot
local store rooted at `/run/media/user/data/nixome/nix-root`, so store paths
still have logical names like `/nix/store/...` while the bytes live on the data
disk. Long nested VM proof targets run `make gc-auto` before booting and opt
into a post-run auto-GC pass when the VM is not being kept alive.

If that data path is not available, Nixome refuses to use the host `/nix/store`
by default. Set `NIXOME_DATA_ROOT` to another data-backed path, or set
`NIXOME_USE_HOST_NIX_STORE=1` for an explicit one-off host-store command. Keep
the Qubes asset cache unless you intentionally want to re-download the large
template RPM.

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

`make clean` removes VM work directories and logs, but preserves the download
cache and the Nixome Nix store. `make gc-auto`, `make gc-dry-run`, and
`make gc` operate only on the separate Nixome store under
`/run/media/user/data/nixome/nix-root`; the GC script refuses to run against the
host `/nix/store`. `make gc-auto` runs automatically before heavy VM/image
targets and runs a full guarded GC only when the Nixome store is over
`NIXOME_GC_AUTO_THRESHOLD_GIB`, default `80`. `make gc` refreshes
Nixome-managed GC roots for current result links, VM backing images, and current
JSON deployment artifacts, then deletes all unreachable paths in the Nixome
custom store. Named build results are linked under
`/run/media/user/data/nixome/result-links`. The VM runner keeps only one stale
overlay backup per image by default; set `NIXOME_VM_BACKUP_RETENTION=0` to keep
none, or a larger number if you want rollback overlays while iterating.

On a booted Xen dom0 target with a Qubes template installed:

```bash
NIXOME_RUN_QUBES_SMOKE=1 make test
```

The Qubes smoke test creates `test-nixome-smoke` if needed, starts it, runs
`uname -a` through `qvm-run --pass-io`, validates stdout, writes logs under the
generated log directory, and removes the test qube on success if it created it.

## Current Shape

- `flake.nix` pins `nixpkgs` and consumes the local `vendor/nixos-qubes` fork.
  The `nixos-qubes` `nixpkgs` input follows this repository's `nixpkgs` pin.
- `modules/dom0.nix` enables upstream Qubes-on-NixOS dom0 support and keeps the
  upstream security gap explicit with `virtualisation.qubes.secure = false`.
- `nixome-qube-bootstrap`, `nixome-qubes-smoke`, and
  `nixome-qube-gui-smoke` are development-only commands for progressing the
  VM lifecycle, qrexec, and GUI feedback loop.
- `nixome-config-apply` is the first explicit dom0 command for applying a
  validator-approved config candidate and archiving a deployment record.
- `nixome-config-status` reports the active topology, Qubes runtime domains,
  current config apply artifacts, host generation links, and Nixome qrexec
  policy state as JSON. It prefers the last successfully set up topology from
  `/var/lib/nixome/system/current-topology.json`, falling back to the bootstrap
  image topology, and includes durable current/last deployment records from the
  same state directory. It also reports `pending_reboot` from the active system
  plan and `pending_vm_restarts` from the latest topology setup result, so
  automation can distinguish host reboot work from Qube restart work.
- `modules/qube-guest.nix`, `pkgs/nixome-qubes-template`, and
  `modules/qubes-topology.nix` are the first pieces for NixOS-backed Qubes
  templates and declarative topology setup.
- `lib/nixome/topology.nix` is the shared topology schema/defaulting helper used
  by both dom0 topology generation and staged config candidates, including the
  constrained Nixome-owned qrexec policy schema.
- The bootstrap config-system topology currently uses one NixOS guest TemplateVM
  for all three config AppVM roles to keep nested image builds practical; staged
  config can now define additional NixOS TemplateVM contents in
  `nixome/local.nix` as first-class template definitions.
- `machines/test.nix` is the first concrete NixOS dom0 configuration.
- `tests/` contains layered feedback scripts.
- `docs/` records upstream findings and the next test boundary.

## Status

The Fedora-template nested GUI/qrexec smoke path works on a host with KVM and
nested virtualization. The NixOS-template
`config-vault`/`config-staging`/`config-validator` path now evaluates, builds
the shared bootstrap template artifact, imports/creates the declared TemplateVM
and AppVMs, boots the NixOS AppVMs under nested Xen, proves qrexec with
`uname -a`, and verifies the typed config candidate validation/archive/apply flow. The
staged config now also defines a NixOS `nixome-nixos-work` TemplateVM package;
the fresh Nixome image prebuilds that template source, the validator-approved
topology imports it by concrete Nix store path, and dom0 creates
AppVMs from it. Staged config also generates Nixome-owned qrexec policy into
`/var/lib/nixome/qubes-policy.d/90-nixome.policy`; Qubes' policy daemon is
pointed at that combined runtime policy directory.
The two-boot `make vm-config-boot-activate-flow` path proves the staged inside
config can become the running dom0 NixOS boot generation in the nested VM.
The heavier `make vm-config-template-flow` path additionally proves a staged
TemplateVM definition that has no prebuilt source, so dom0 must build and import
the TemplateVM from the validated candidate before starting its AppVM.
Production security validation still requires a physical Xen-capable machine.
