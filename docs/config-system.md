# Config System

This is the first Nixome-owned Qubes topology slice. It keeps the Qubes control
plane in dom0, but moves the desired VM topology into Nix.

## Declared Objects

`machines/config-system.nix` declares:

- `nixome-nixos-config-vault`, a NixOS TemplateVM source.
- `nixome-nixos-config-staging`, a NixOS TemplateVM source.
- `nixome-nixos-config-validator`, a NixOS TemplateVM source.
- `config-vault`, an offline AppVM based on the config-vault template.
- `config-staging`, an offline AppVM based on the config-staging template.
- `config-validator`, an offline AppVM based on the config-validator template.
- `default_kernel = ""`, so fresh config-only images can create TemplateVMs
  before a Fedora/Qubes VM kernel has been bootstrapped.
- qrexec policy allowing only dom0 to call `nixome.Config.CandidateExport` on
  `config-staging`, `nixome.Config.ValidateCandidate` on `config-validator`, and
  `nixome.Config.ArchiveCandidate` on `config-vault`.

The initial VM resource settings are sized for the nested 8 GiB developer VM:
`config-vault` starts at 1024 MiB, while `config-staging` and `config-validator`
start at 2048 MiB. A physical Nixome host can raise these declaratively later.

For this POC the TemplateVM source directories reference the same NixOS guest
root image. That shared image carries the vault archive tooling, staging export
tooling, and validator tooling; the AppVM names, Qubes features, offline
networking, and qrexec policy carry the runtime role distinction. Dedicated
role-specific guest closures already evaluate/build and remain the hardening
target once the authority flow is stable.

`config-staging` owns the editable candidate repository at:

```text
/home/user/nixome-config
```

It can initialize that candidate with:

```bash
nixome-config-staging-init
```

The seeded candidate exposes `nixome.topology` from `flake.nix`. For this first
apply proof, that Nix value declares a small observable Qubes feature on
`config-staging`:

```text
nixome-config-applied-message
```

`config-validator` validates candidates exported from `config-staging`,
evaluates `.#nixome.topology` to the `topology.json` apply artifact, and writes
validation JSON under:

```text
/home/user/nixome-config-validation
```

`config-vault` owns the archive repository at:

```text
/home/user/nixome-config-vault
```

dom0 archives a candidate by running:

```bash
sudo nixome-config-archive-candidate
```

That command refuses to proceed unless `config-staging`, `config-validator`, and
`config-vault` are offline. It fetches a clean Git bundle from
`config-staging`, sends it to `config-validator` for `nix flake check
--no-build` and `nix eval .#nixome.topology --json`, checks the validation
artifact, evaluated topology, and bundle commit in dom0, then sends the
validated bundle plus evaluated topology to `config-vault` for archival.
This records what dom0 is about to attempt before deployment logic exists.

dom0 applies a validated candidate by running:

```bash
sudo nixome-config-apply --apply
```

That command performs the same staging-to-validator-to-vault flow, then applies
the validator-produced `topology.json` with `nixome-setup-config --apply`.
Dom0 transports and applies the artifact, but the candidate validity decision
and Nix evaluation happen in `config-validator`.

## Setup Path

The dom0 module writes the desired topology to:

```text
/etc/nixome/qubes-topology.json
```

Apply it from dom0 with:

```bash
sudo nixome-setup-config --apply
```

For a non-mutating preview:

```bash
sudo nixome-setup-config --plan
```

The setup command currently imports missing templates, creates missing domains,
and sets global prefs plus simple domain prefs/features/services. It is
idempotent: if the declared templates, domains, prefs, features, services, and
managed VM kernel files already match, it reports zero changes. It does not
delete undeclared domains or perform full restart/reboot classification yet.

## Developer Commands

Build the NixOS Qube template directory artifacts:

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

Run only the nested control-plane check plus topology setup:

```bash
make vm-config-setup
```

That target uses the config image, skips Fedora template bootstrap, skips the
old qrexec/window smoke tests, and saves setup output under
`.nixome-logs/`.

Run the current NixOS config-VM runtime proof:

```bash
make vm-config-smoke
```

That target builds and boots the nested config dom0 image, explicitly runs the
dom0 topology setup, starts `config-vault`, `config-staging`, and
`config-validator`, runs `uname -a` through qrexec in all three VMs, initializes
the staging repository and validator workspace, edits and commits `flake.nix`
in `config-staging`, evaluates `.#nixome.topology` in `config-validator`,
archives the source and evaluated topology into `config-vault`, applies it from
dom0, and checks that the declared Qubes feature changed.
The machine-readable result is copied to:

```text
.nixome-logs/vm-gui-config-vms-smoke.json
```

## Validated Boundary

The template artifacts are NixOS root images packaged in the shape expected by
`qvm-template-postprocess`. The explicit dom0 setup command has imported those
templates and created the three AppVMs in the nested dom0 test environment.

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

The validated smoke path reaches qrexec in the declared NixOS AppVMs and proves
that `config-vault` has `/home/user/nixome-config-vault` initialized before
qrexec accepts the repository status check. The next boundary is deployment from
the archived commit plus hardening the model: split role-specific guest roots
when useful, classify setup changes that require VM restart or dom0 reboot, and
validate the same path on physical Xen-capable hardware.
