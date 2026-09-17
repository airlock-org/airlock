# Airlock

Airlock is a proof-of-concept path toward a Nix-native Qubes OS architecture.
The first milestone is narrow: run the Qubes control stack with NixOS as Xen
dom0 while retaining Qubes' Xen, qrexec, QubesDB, policy, libvirt, and VM
lifecycle machinery.

This is the `airlock-dev` repository. It pins the independently buildable
`airlock-os` repository as the real Git submodule at `os/`. The OS repository
contains the local [`CertainLach/nixos-qubes`](https://github.com/CertainLach/nixos-qubes)
source input and all deployable host, guest, bootstrap, and runtime code.
This parent repository supplies USB image assembly, provisioning, lab tooling,
and cross-repository integration tests.

- a POC NixOS dom0 configuration,
- closure-backed NixOS Qubes domains,
- conventional Fedora/Debian TemplateVM/AppVM topology support,
- typed Nix topology and system-plan schemas,
- explicit dom0 setup/apply/status commands,
- config authority VMs for staging, validation, and archival,
- local checks and nested Xen/Qubes integration feedback.

## Repository Layout

- `os/`: pinned `airlock-os` submodule; the deployable OS source of truth.
- `lifecycle/image/`: prepared-store USB assembly and image-only tools.
- `lifecycle/misc/tools/scripts/`: development-host and lab orchestration.
- `docs/`: design notes, config authority details, and test documentation.
- `tests/`: cross-repository, image, and deployment integration tests.

See [repository workflow](docs/repositories.md) for clone, edit, and submodule
update commands.

## Bootstrap

Bootstrap prepares a fresh Airlock system through its first ready-to-use
`sys-gui` desktop. It creates the initial Qubes topology, including the
`sys-config-*` authority domains and user domains, establishes the first
authoritative runtime configuration, and applies initial desktop defaults.
Later configuration changes use the normal declarative apply path rather than
re-running bootstrap.

## Commands

For physical USB-lab iteration without reflashing, use the repository-owned
[whole-machine deployment workflow](docs/machine-deployment.md):

```bash
make machine-plan DEPLOY_TARGET=sys-dev-2
make machine-apply DEPLOY_TARGET=sys-dev-2
make machine-status DEPLOY_TARGET=sys-dev-2
make machine-verify DEPLOY_TARGET=sys-dev-2
```

Commit first. This builds one release for dom0 and the complete VM topology;
only affected VMs restart. Host changes require reboot. The first transfer needs
substantial spare disk space; see the guide's prerequisites and recovery limits.

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
make vm-gui-minimal-image
AIRLOCK_VM_DISPLAY=gtk make vm-gui-minimal-run
make vm-gui-minimal-ssh
```

Builds and launches a separate dom0 + `sys-gui` image with the existing hybrid
XFCE desktop. It omits the other VM images and application launchers, uses a
dedicated disk directory and SSH port `2332`, and does not assign physical
GPUs. See [sys-gui baseline](docs/sys-gui.md) for scope, test credentials and
the later AMD direct-display milestone. This new profile has not been tested.

```bash
make vm-gui-config-image
make vm-gui-config-run
```

Builds and launches the current nested GUI dom0 image. The nested image uses a
throwaway `airlock` user with password `airlock` and loopback-only SSH
forwarding on port `2222`.

```bash
make vm-gui-config-network-debug-run
```

Builds and launches an isolated network-debug image. It creates the usual
domain topology but starts only `sys-net`; NetworkManager is available for a
manual start inside that guest.

```bash
make vm-gpu-lab-test
make vm-gpu-lab-run
```

Tests or launches an isolated image with an on-demand `sys-gpu` diagnostic
guest. XFCE stays in place and physical GPUs remain unassigned. See
[sys-gpu experiments](docs/sys-gpu.md) for the test boundaries and the planned
AMD iGPU → `sys-gui`, RTX 3090 → `sys-gpu` split.

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
fresh nested dom0 disk, runs the dom0 setup command, boots `sys-config-vault`,
`sys-config-staging`, and `sys-config-validator`, proves qrexec, modifies and validates
a staged config, has dom0 apply the validator-approved topology/system plan,
and records the deployment in `sys-config-vault`.

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
sys-config-vault
sys-config-staging
sys-config-validator
```

Each config VM is a Qubes `StandaloneVM` with:

- an explicit NixOS system closure,
- a small writable root image,
- a separate read-only `store` volume mounted at `/nix/store`,
- private persistent state,
- normal Qubes qrexec/policy/network controls.

`sys-config-staging` and `sys-config-validator` run candidate Nix checks through
`airlock-config-nix`, backed by private writable state, so their own system
`/nix/store` remains read-only.

The old NixOS TemplateVM path for config domains has been removed. Generic
Qubes TemplateVM support remains for Fedora, Debian, and other conventional
guests.

See [docs/config-system.md](docs/config-system.md) for the config authority
model and [docs/testing.md](docs/testing.md) for the test layers.

## Generated Data

When `/run/media/user/data` is mounted, generated writable VM overlays, logs,
temporary files, Qubes RPM/template downloads, and Airlock Nix build outputs
default to:

```text
/run/media/user/data/airlock/
```

Source code, Nix files, scripts, docs, and Git history stay in this repository.
The Airlock Nix store is a chroot local store rooted at:

```text
/run/media/user/data/airlock/nix-root
```

Logical store paths still look like `/nix/store/...`; the bytes live under the
data-disk root. If `/run/media/user/data` is not available, Airlock refuses to
use the host `/nix/store` by default. Set `AIRLOCK_DATA_ROOT` to another
data-backed path, or set `AIRLOCK_USE_HOST_NIX_STORE=1` for an explicit
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

`make gc` operates only on the separate Airlock store and refuses to run against
the host `/nix/store`.

## Status

The nested GUI/Qubes path has proven the Qubes control services, Fedora
TemplateVM qrexec, closure-backed NixOS domain boot, read-only store volume
mounting, config staging/validation/promotion, Airlock-owned qrexec policy
generation, and dom0 generation switching under nested Xen.

Production security validation still requires a physical Xen-capable machine.
