# Testing

Nixome's test interface is layered so `make test` remains useful before a real
Xen/Qubes target exists.

## Local Layer

```bash
make test
```

Runs:

- `nix flake check --no-build`
- the Nixome module contract check,
- the lightweight `nixome-observe-dom0` package build,
- host capability detection into `.nixome-logs/host-capabilities.json`,
- a skipped Qubes smoke-test result unless `NIXOME_RUN_QUBES_SMOKE=1`.

The local layer should pass on ordinary NixOS development hosts. It does not
prove that Xen boots or that Qubes services run.

## Full System Build

```bash
make build
```

Builds:

```text
.#nixosConfigurations.test.config.system.build.toplevel
```

This proves the NixOS dom0 closure can be built. It may build substantial Qubes
and Xen dependencies if substitutes are unavailable.

The current `machines/test.nix` assumes a root filesystem available as
`/dev/disk/by-label/nixome-root`. A real target should replace or extend this
machine file with its hardware-specific disk layout.

## Nested VM Trial

```bash
make vm-image
make vm-run
```

This builds `.#nixome-vm-image` from `machines/vm.nix` and starts it with
QEMU/KVM, OVMF, `-cpu host`, and a writable qcow2 overlay under `.nixome-vm/`.

Defaults:

- 8192 MiB RAM
- 4 vCPUs
- SSH forwarded on `127.0.0.1:2222`
- user `nixome`
- password `nixome`
- headless serial display

Useful overrides:

```bash
NIXOME_VM_MEMORY=12288 NIXOME_VM_SMP=6 make vm-run
NIXOME_VM_DISPLAY=gtk make vm-run
NIXOME_VM_SSH_PORT=2223 make vm-run
```

## Graphical Nested VM

```bash
make vm-gui-image
make vm-gui-run
```

This builds `.#nixome-vm-gui-image` from `machines/vm-gui.nix` and starts it
with `NIXOME_VM_DISPLAY=gtk`. The VM profile enables X11, LightDM, XFCE,
autologin for the throwaway `nixome` user, and Qubes GUI-related dom0 packages.

The graphical image keeps its writable qcow2 overlay separate from the headless
image under `.nixome-vm/`.

After it boots:

```bash
ssh -p 2222 nixome@127.0.0.1
nixome-observe-dom0 | jq
cat /proc/xen/capabilities
systemctl status qubes-db-dom0 qubesd qubes-qrexec-policy-daemon
```

After a Qubes VM kernel and template are installed, run `nixome-qubes-smoke`
inside the VM to create/start the test qube and execute `uname -a` through
qrexec.

This VM path is not a production-supported Qubes deployment. It is a development
probe for whether the NixOS guest can boot Xen and reach Qubes control services
under nested virtualization.

The GUI image closure is large enough to OOM the default NixOS image-copy
helper in this environment. Nixome applies a narrow overlay to `pkgs.lkl` so
`cptofs` uses `mem=1024M` while assembling disk images.

## Runtime Observation

The POC module installs:

```bash
nixome-observe-dom0
```

It emits JSON with host, Xen, service, and Qubes command state. It is a
development observation tool only; it does not grant new control authority.

## Qubes/qrexec Smoke Test

On a booted Xen dom0 target with Qubes services active and a template installed:

```bash
NIXOME_RUN_QUBES_SMOKE=1 make test
```

Optional environment:

```bash
NIXOME_TEST_TEMPLATE=fedora-43-xfce
NIXOME_TEST_QUBE=test-nixome-smoke
NIXOME_TEST_CLEANUP=1
```

The smoke test:

- requires Xen control-domain status,
- requires `qubes-db-dom0`, `qubesd`, and `qubes-qrexec-policy-daemon`,
- creates an AppVM if `NIXOME_TEST_QUBE` does not already exist,
- starts the qube,
- runs `uname -a` with `qvm-run --pass-io`,
- writes stdout and JSON results under `.nixome-logs/`,
- removes the test qube on success if it created it and cleanup is enabled.

Failures intentionally preserve logs and leave failed state for inspection.

## Next Integration Layer

Once the POC system builds, the next target is a physical Xen-capable test
machine or lab host that can:

- boot `.#nixosConfigurations.test`,
- install/import a Qubes VM kernel and one template,
- run `nixome-observe-dom0`,
- run `NIXOME_RUN_QUBES_SMOKE=1 make test`,
- archive `.nixome-logs/` for agent inspection.
