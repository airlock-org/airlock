# Testing

Nixome's test interface is layered so `make test` remains useful before a real
Xen/Qubes target exists.

## Local Layer

```bash
make test
```

Runs fast local checks:

- `nix flake check --no-build`
- topology and system schema tests,
- setup/apply/status helper tests,
- staged candidate validation tests,
- host capability detection,
- shell syntax and static runner checks.

The local layer should pass on ordinary NixOS development hosts. It does not
prove that Xen boots or that Qubes services run.

## Build Layer

```bash
make build
```

Builds:

```text
.#nixosConfigurations.test.config.system.build.toplevel
```

This proves the NixOS dom0 closure can be built. A real target should replace
or extend `machines/test.nix` with its hardware-specific disk layout.

## Nested GUI Dom0

```bash
make vm-gui-config-image
make vm-gui-config-run
```

Builds and launches the current nested GUI dom0 image. The launcher uses
QEMU/KVM, OVMF, `-cpu host`, and a writable qcow2 overlay under the generated
VM directory. Defaults:

- 8192 MiB RAM
- 4 vCPUs
- SSH forwarded on `127.0.0.1:2222`
- user `nixome`
- password `nixome`

Useful overrides:

```bash
NIXOME_VM_MEMORY=12288 NIXOME_VM_SMP=6 make vm-gui-config-run
NIXOME_VM_SSH_PORT=2223 make vm-gui-config-run
NIXOME_KEEP_VM=1 make vm-config-flow
```

Connect with:

```bash
make vm-ssh
```

## Config Authority Flow

```bash
make vm-config-flow
```

This is the main cold nested integration path for the current architecture. It
starts from a fresh writable nested dom0 disk, waits for Qubes services, runs
the setup command, and verifies:

- `config-vault`, `config-staging`, and `config-validator` exist.
- each config VM is a Qubes `StandaloneVM`.
- each uses `backend.type = "nix-closure"`.
- each has a separate read-only `store` volume attached at `/dev/xvde`.
- `/nix/store` is mounted read-only at runtime.
- qrexec works in each config VM.
- GUI appmenus can be synchronized for the config VMs.
- private state under `/home/user` survives restart.
- `config-staging` can modify and commit a candidate.
- `config-validator` validates and evaluates the candidate.
- dom0 applies the resolved topology/system plan.
- `config-vault` records candidate and deployment history.
- generated Nixome-owned qrexec policy is active.
- a staged Nix-native domain can be created and booted through qrexec.

Results are copied to:

```text
/run/media/user/data/nixome/logs/vm-gui-config-vms-smoke.json
```

The script name still contains `smoke` for historical reasons; the current
flow is an end-to-end runtime integration proof.

## Dom0 Generation Flow

```bash
make vm-config-boot-flow
make vm-config-boot-activate-flow
```

`make vm-config-boot-flow` runs the same candidate path but requests
`switch-to-configuration boot`, proving dom0 can install a next-boot NixOS
generation and report a pending reboot without changing the running generation.

`make vm-config-boot-activate-flow` preserves the same writable nested disk,
boots it again, and runs `nixome-config-boot-verify` in dom0. The verifier
checks that `/nix/var/nix/profiles/system` and `/run/current-system` resolve to
the staged generation and that status no longer reports a pending reboot.

## Closure Domain Flow

```bash
make vm-nix-closure-flow
```

Runs a narrower real Xen/Qubes test for one representative `nix-closure`
domain. It verifies the small root image, separate read-only `/nix/store`
volume, early boot mount path, qrexec, restart behavior, and absence of a full
duplicate Nix store in the root image.

## Conventional Qubes Guests

```bash
make vm-gui-smoke
```

Runs the broader GUI path that can download/import conventional Qubes Fedora
kernel/template RPMs, set Qubes defaults, run qrexec against a Fedora-backed
AppVM, and try to detect a Qube GUI window. Fedora/Debian TemplateVM support is
kept separate from NixOS `nix-closure` domains and should continue to pass this
path.

The Fedora template RPM is large. When `/run/media/user/data` is mounted,
generated VM disks, logs, caches, temp files, and Nixome Nix build outputs
default to:

```text
/run/media/user/data/nixome/
```

Inside nested dom0, cached Qubes RPM/template inputs are copied to:

```text
/var/cache/nixome/qubes-rpms/
```

## Generated State

The source tree, Nix code, scripts, docs, and Git history stay in
`/home/user/dev/nixome`. The Nixome Nix store is a chroot local store at:

```text
/run/media/user/data/nixome/nix-root
```

Logical store paths remain `/nix/store/...`; the physical files live under the
data-disk root. Named build roots that should survive Nixome-only GC live under:

```text
/run/media/user/data/nixome/result-links
```

If local builds run low on disk:

```bash
make disk-usage
make gc-auto
make gc-roots
make gc-dry-run
make gc
make clean-vm-disks
make clean-vm-workdirs
make clean-logs
```

`make gc` refreshes Nixome-managed GC roots and deletes unreachable paths only
from the separate Nixome store. It refuses to operate on the host `/nix/store`.

## Physical Target

Nested Xen/KVM is useful for development, but production security validation
requires a physical Xen-capable test machine that can boot a Nixome dom0 image,
run Qubes control services, import conventional templates, boot closure-backed
NixOS domains, and archive logs/results for inspection.
