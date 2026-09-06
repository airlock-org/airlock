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
- the `nixome-setup-config` package build,
- evaluation of the config-system topology that declares `config-vault` and
  `config-staging` and `config-validator`,
- typed topology schema/defaulting checks for staged config candidates,
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
QEMU/KVM, OVMF, `-cpu host`, and a writable qcow2 overlay under the generated
VM directory.

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
NIXOME_VM_VGA=virtio make vm-gui-run
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
The launcher defaults to standard VGA for nested Xen because virtio VGA can
boot and run X while leaving the GTK display black on this host.

The graphical image keeps its writable qcow2 overlay separate from the headless
image under the generated VM directory.

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

For an automated version of the boot/control-plane check:

```bash
make vm-gui-check
```

This starts the GUI image, waits for SSH, verifies `libvirtd`, `qubesd`,
QubesDB, and the qrexec policy daemon, saves `nixome-observe-dom0` JSON, and
then stops the VM. It does not download templates or create Qubes.

For the current full nested smoke path:

```bash
make vm-gui-smoke
```

This does the control-plane check and then, by default:

- runs `nixome-qube-bootstrap` in dom0 as root,
- caches the Qubes R4.3 RPM signing key on the host,
- caches the Fedora 41 Qubes VM kernel RPMs on the host,
- caches the Fedora 43 XFCE template RPM on the host,
- copies cached assets into the nested dom0 and imports them there,
- sets `default_kernel`, `default_template`, `default_dispvm`, and `updatevm`,
- runs `nixome-qubes-smoke`,
- runs `nixome-qube-gui-smoke`.

The Fedora 43 XFCE template RPM is about 2 GB. When `/run/media/user/data` is
mounted, generated VM disks, logs, caches, and temp files default to:

```text
/run/media/user/data/nixome/
```

The source tree, Nix code, scripts, docs, and Git history stay in
`/home/user/dev/nixome`. Without the data mount, generated files fall back to
repo-local `.nixome-vm/`, `.nixome-logs/`, and `.nixome-cache/rpms/`.

Inside the nested dom0, cached inputs are copied to:

```text
/var/cache/nixome/qubes-rpms/
```

To run only selected layers:

```bash
NIXOME_BOOTSTRAP_QUBES=0 make vm-gui-smoke
NIXOME_RUN_QUBES_SMOKE=0 make vm-gui-smoke
NIXOME_RUN_GUI_SMOKE=0 make vm-gui-smoke
NIXOME_KEEP_VM=1 make vm-gui-smoke
NIXOME_CACHE_QUBES_ASSETS=0 make vm-gui-smoke
```

Default bootstrap inputs come from the current `nixos-qubes` README examples:

```text
template: fedora-43-xfce
kernel:   6.12.64-1.fc41
key:      https://keys.qubes-os.org/keys/qubes-release-4.3-signing-key.asc
```

Override them with `NIXOME_QUBES_TEMPLATE`,
`NIXOME_QUBES_TEMPLATE_URL`, `NIXOME_QUBES_KERNEL_NAME`,
`NIXOME_QUBES_KERNEL_VERSION`, `NIXOME_QUBES_KERNEL_FEDORA`,
`NIXOME_QUBES_KERNEL_URL`, and `NIXOME_QUBES_KERNEL_MODULES_URL`.
Override the host cache location with `NIXOME_QUBES_CACHE_DIR`.
RPM signature verification is enabled by default. Set
`NIXOME_QUBES_VERIFY_RPMS=0` only for throwaway development debugging.

This VM path is not a production-supported Qubes deployment. It is a development
probe for whether the NixOS guest can boot Xen and reach Qubes control services
under nested virtualization.

The GUI image closure is large enough to OOM the default NixOS image-copy
helper in this environment. Nixome applies a narrow overlay to `pkgs.lkl` so
`cptofs` uses `mem=1024M` while assembling disk images.

## NixOS Template Topology

```bash
make nixos-templates-dry-run
make nixos-templates
```

These targets build the NixOS Qube template source directories for
`nixome-nixos-config-vault`, `nixome-nixos-config-staging`, and
`nixome-nixos-config-validator`. The outputs are Nix store directories containing
`root.img`, `template.conf`, and Qubes appmenu list files. They are not RPMs.
For now, all three template directories symlink `root.img` to one shared NixOS
guest root image to reduce local build size.

To build or boot a nested GUI dom0 image that includes the declared
`config-vault`, `config-staging`, and `config-validator` topology:

```bash
make vm-gui-config-image
make vm-gui-config-run
```

To run just the nested control-plane check plus the topology setup command:

```bash
make vm-config-setup
```

This skips the Fedora template bootstrap and old qrexec/window smoke tests,
then runs `sudo nixome-setup-config --apply` in the nested dom0. Setup
logs are written under `.nixome-logs/` on the host and `/var/log/nixome/` in
the nested dom0.

To run the current NixOS config-VM boot/qrexec proof:

```bash
make vm-config-smoke
```

This starts from a fresh writable VM disk, explicitly runs the dom0 topology
setup, then starts `config-vault`,
`config-staging`, and `config-validator`, runs `uname -a` through qrexec in all
three VMs, checks that `/home/user/nixome-config-vault` and
`/home/user/nixome-config` already exist as clean Git repositories, modifies
and commits `flake.nix`, `nixome/config-vms.nix`, `nixome/templates.nix`, and
`nixome/topology.nix` in `config-staging`, has
`config-validator` run `nix flake check --no-build` and evaluate the
schema-normalized `.#nixome.topology`, has dom0 archive that validated source
plus evaluated topology into `config-vault`, and applies the validated topology
with `nixome-config-apply --apply`. It then checks that the applied topology
re-declared `config-vault`, `config-staging`, and `config-validator`, checks
that the Nix-declared `nixome-work-from-config` AppVM exists, and verifies that
`uname -a` runs inside it through qrexec. It also checks that `config-vault`
contains a successful deployment record for that apply, and that the staged
config generated
`/var/lib/nixome/qubes-policy.d/90-nixome.policy` with an active `nixome.PolicyProbe`
rule. The result JSON is written to:

```text
.nixome-logs/vm-gui-config-vms-smoke.json
```

The config topology image keeps local build size down by sharing one NixOS
guest root image between the three TemplateVM source directories. The VM runner
can use either qcow2 or raw Nix-built base images and creates a writable qcow2
overlay for runtime state.
The nested config image currently has 32 GiB of virtual disk capacity.

The current NixOS guest template profile uses QubesDB, qrexec-agent, Qubes
meminfo writer, and `/dev/mapper/dmroot` as the root device to match Qubes'
normal Linux VM boot path. Import/create setup is automated, and
`make vm-config-smoke` currently proves that the declared NixOS AppVMs boot and
serve qrexec under nested Xen, and that a staged Nix topology can add one more
AppVM, generate Nixome-owned qrexec policy, and record the deployment in
`config-vault`.
The config profile sets `default_kernel = ""` so fresh config-only images can
create TemplateVMs before any Fedora/Qubes VM kernel bootstrap has run.

The VM runner keeps generated writable qcow2 overlays and Nix build out-links
under the generated VM directory. They are safe to regenerate. Old repo-local
experimental VM work directories match `.nixome-vm-*`. The host-side Qubes
RPM/template download cache lives under the generated cache directory and
should normally be kept.

If a local image build fails with `No space left on device`, inspect and clean
generated state with:

```bash
make disk-usage
make clean-vm-disks
make clean-vm-workdirs
make clean-logs
```

`make clean` removes VM work directories and logs while preserving the download
cache. `make clean-cache` removes the generated cache too, which means the next
full GUI smoke may need to download the Qubes kernel RPMs and template RPM
again. The runner keeps only one stale writable-overlay backup per image by
default; adjust that with `NIXOME_VM_BACKUP_RETENTION`.

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
