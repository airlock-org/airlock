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
  `config-staging`, `nixome.Config.ValidateCandidate` on `config-validator`,
  `nixome.Config.ArchiveCandidate` on `config-vault`, and
  `nixome.Config.ArchiveDeployment` on `config-vault`.

That config-access policy is bootstrap policy and remains dom0/NixOS-owned in
`30-nixome-config.policy`, so staged config changes cannot lock dom0 out of the
fetch/validate/archive/apply path. Additional Nixome-owned qrexec policy is
declared in staged topology and rendered by `nixome-setup-config` to:

```text
/etc/qubes/policy.d/90-nixome.policy
```

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

The seeded candidate defines `nixome.config` in `flake.nix`, imports the local
`./nixome/topology.nix` schema helper, and exposes normalized
`nixome.topology`:

```nix
let
  topologySchema = import ./nixome/topology.nix;
  config = {
    domains.config-staging = {
      template = "nixome-nixos-config-staging";
      label = "gray";
      prefs.include_in_backups = true;
      features."nixome-config-applied-message" = "initial";
    };
    qrexecPolicy.rules = [
      {
        service = "nixome.PolicyProbe";
        source = "dom0";
        target = "config-vault";
        action = "allow";
      }
    ];
  };
in
{
  nixome.config = config;
  nixome.topology = topologySchema.mkTopology config;
}
```

The schema currently checks domain classes, labels, Qube/template names,
template and kernel source fields, JSON-safe prefs/features, boolean services,
and constrained Nixome-owned qrexec policy rules. Managed qrexec service names
must start with `nixome.`, bootstrap config services are reserved, actions are
limited to `allow` and `deny`, allow rules cannot use `@anyvm`, and generated
policy cannot target dom0. The schema also appends a catch-all deny rule for
each managed service.

The schema defaults AppVMs to `class = "AppVM"`, `netvm = null`,
`prefs.autostart = false`, `prefs.include_in_backups = false`, empty
`features`/`services`, and an empty generated qrexec policy.

For this first apply proof, the Nix value declares a small observable Qubes
feature on `config-staging`:

```text
nixome-config-applied-message
```

The runtime smoke then edits `flake.nix` to add `nixome-test-from-config`, an
AppVM based on `nixome-nixos-config-staging`, and proves that dom0 creates it
from the validator-produced topology. It also declares a generated
`nixome.PolicyProbe` qrexec policy rule and proves dom0 can call that service in
`config-vault` after apply.

`config-validator` validates candidates exported from `config-staging`,
evaluates the schema-normalized `.#nixome.topology` to the `topology.json` apply
artifact, and writes validation JSON under:

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
This records what dom0 is about to attempt before deployment.

dom0 applies a validated candidate by running:

```bash
sudo nixome-config-apply --apply
```

That command performs the same staging-to-validator-to-vault flow, applies the
validator-produced `topology.json` with `nixome-setup-config --apply`, then asks
`config-vault` to archive a deployment record. Dom0 transports and applies the
artifact, but the candidate validity decision and Nix evaluation happen in
`config-validator`.

`config-vault` now has two durable artifact classes:

```text
candidates/<candidate-commit>/
deployments/<timestamp>-<candidate-prefix>/
latest-candidate
latest-deployment
latest-deployment-attempt
```

`candidates/` records validated source snapshots and evaluated topology before
deployment. `deployments/` records what dom0 attempted after setup ran,
including `deployment.json`, `apply.json`, `archive.json`, `setup.json`,
`topology.json`, `validation.json`, `manifest.json`, and relevant dom0 logs.
Passed deployments update both `latest-deployment` and
`latest-deployment-attempt`; failed setup attempts update only
`latest-deployment-attempt` when the vault can still be reached.

## Setup Path

The dom0 module also normalizes `nixome.qubes.*` through the same schema helper
and writes the desired topology to:

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
dom0, checks that `nixome-test-from-config` was created, and runs `uname -a`
inside it through qrexec. It also checks that `config-vault` wrote a deployment
record for the successful apply and that generated qrexec policy is installed
and active.
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

The validated smoke path reaches qrexec in the declared NixOS AppVMs, proves
that `config-vault` has `/home/user/nixome-config-vault` initialized before
qrexec accepts the repository status check, and proves that a staged Nix config
can create a new AppVM that boots and serves qrexec. It also records the
successful apply as a deployment in `config-vault` and proves a staged
Nixome-owned qrexec policy rule by calling `nixome.PolicyProbe` through
`qrexec-client`. The next boundary is hardening the model: split role-specific
guest roots when useful, classify setup changes that require VM restart or dom0
reboot, and validate the same path on physical Xen-capable hardware.
