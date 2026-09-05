# Config System

This is the first Nixome-owned Qubes topology slice. It keeps the Qubes control
plane in dom0, but moves the desired VM topology into Nix.

## Declared Objects

`machines/config-system.nix` declares:

- `nixome-nixos-config-vault`, a NixOS TemplateVM source.
- `nixome-nixos-config-mod`, a NixOS TemplateVM source.
- `sys-config-vault`, an offline AppVM based on the config-vault template.
- `sys-config-mod`, an offline AppVM based on the config-mod template.
- `default_kernel = ""`, so fresh config-only images can create TemplateVMs
  before a Fedora/Qubes VM kernel has been bootstrapped.
- qrexec policy allowing only `sys-config-mod` to call
  `nixome.Config.Export` on `sys-config-vault`.

The initial VM resource settings are sized for the nested 8 GiB developer VM:
`sys-config-vault` starts at 1024 MiB and `sys-config-mod` starts at 2048 MiB. A
physical Nixome host can raise these declaratively later.

For this POC both TemplateVM source directories reference the same NixOS guest
root image. That shared image carries both the vault export service and the
config-mod fetch tooling; the AppVM names, Qubes features, offline networking, and
qrexec policy carry the role distinction. Splitting the guest root into
role-specific templates remains compatible with the topology model once the
boot path is stable.

`sys-config-vault` owns the authoritative Git repository at:

```text
/home/user/nixome-config
```

`sys-config-mod` gets a read-only snapshot by running:

```bash
nixome-agent-fetch-config
```

That command calls `nixome.Config.Export` over qrexec, receives a Git bundle,
clones it to `/home/user/nixome-config-readonly`, and removes write bits from
the snapshot. It does not grant `sys-config-mod` a qrexec service that writes back
to the vault.

## Reconcile Path

The dom0 module writes the desired topology to:

```text
/etc/nixome/qubes-topology.json
```

Apply it from dom0 with:

```bash
sudo nixome-qubes-reconcile --apply
```

For a non-mutating preview:

```bash
sudo nixome-qubes-reconcile --plan
```

The reconciler currently imports missing templates, creates missing domains,
and sets global prefs plus simple domain prefs/features/services. It does not
delete undeclared domains or perform full restart/reboot classification yet.

## Developer Commands

Build the two NixOS Qube template directory artifacts:

```bash
make nixos-templates
```

Dry-run the same builds:

```bash
make nixos-templates-dry-run
```

Build or run a nested GUI dom0 image with this topology included:

```bash
make vm-gui-config-image
make vm-gui-config-run
```

Run only the nested control-plane check plus topology reconcile:

```bash
make vm-config-reconcile
```

That target uses the config image, skips Fedora template bootstrap, skips the
old qrexec/window smoke tests, and saves reconcile output under
`.nixome-logs/`.

Run the current NixOS config-VM runtime proof:

```bash
make vm-config-smoke
```

That target builds and boots the nested config dom0 image, applies the declared
Qubes topology, starts `sys-config-vault` and `sys-config-mod`, runs `uname -a`
through qrexec in both VMs, initializes the vault Git repository, and verifies
that `sys-config-mod` can fetch a read-only snapshot through `nixome.Config.Export`.
The machine-readable result is copied to:

```text
.nixome-logs/vm-gui-config-vms-smoke.json
```

## Verified Boundary

The template artifacts are NixOS root images packaged in the shape expected by
`qvm-template-postprocess`. `make vm-config-reconcile` has imported those
templates and created the two AppVMs in the nested dom0 test environment.

The NixOS guests now boot in nested Xen with the normal Qubes dom0-provided VM
kernel/initramfs and kernel options such as `root=/dev/mapper/dmroot`. The
guest profile declares that root path and the template image includes a small
Nix-owned `/sbin/init` wrapper. The wrapper sets up the normal NixOS activation
state in the writable Qubes root, then execs the normal NixOS systemd init.
Guest users and groups are declarative (`users.mutableUsers = false`) so early
systemd services such as QubesDB can resolve their configured service groups
before qrexec is available. The template explicitly opts out of NixOS'
passworded-wheel-user assertion instead of carrying a shared guest password.
It also loads the Xen vchan support modules used by Qubes guests
(`xen-evtchn`, `xen-gntalloc`, `xen-gntdev`, and `xen-privcmd`) before
QubesDB starts.

The verified smoke path reaches qrexec in both declared NixOS AppVMs. The next
boundary is hardening the model: split role-specific guest roots when useful,
classify reconcile changes that require VM restart or dom0 reboot, and validate
the same path on physical Xen-capable hardware.
