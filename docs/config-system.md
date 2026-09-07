# Config System

This is the first Nixome-owned Qubes topology slice. It keeps the Qubes control
plane in dom0, but moves the desired VM topology into Nix.

## Declared Objects

`machines/config-system.nix` declares:

- `nixome-nixos-config-system`, a shared NixOS TemplateVM source.
- `config-vault`, an offline AppVM based on the config-system template.
- `config-staging`, an offline AppVM based on the config-system template.
- `config-validator`, an offline AppVM based on the config-system template.
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
/var/lib/nixome/qubes-policy.d/90-nixome.policy
```

Static Qubes policy remains assembled by Nix under `/etc/qubes/policy.d`.
Nixome prepares `/var/lib/nixome/qubes-policy.d` as a combined policy view at
boot, symlinks the static rules into it, keeps `90-nixome.policy` mutable, and
starts `qubes-qrexec-policy-daemon` with that runtime policy path.

The initial VM resource settings are sized for the nested 8 GiB developer VM:
`config-vault` starts at 1024 MiB, while `config-staging` and `config-validator`
start at 2048 MiB. A physical Nixome host can raise these declaratively later.

For this POC the three config roles share one NixOS guest TemplateVM. That
shared image carries the vault archive tooling, staging export tooling, and
validator tooling; the AppVM names, Qubes features, offline networking, and
qrexec policy carry the runtime role distinction. Dedicated role-specific guest
closures still evaluate/build and remain a hardening target once the authority
flow is stable.

The staged config now takes the next step for normal workload templates: it
defines `nixome-nixos-work` as a NixOS TemplateVM package output and declares
`nixome-work` as an AppVM based on that template.

`config-staging` owns the editable candidate repository at:

```text
/home/user/nixome-config
```

It can initialize that candidate with:

```bash
nixome-config-staging-init
```

The seeded candidate defines `nixome.config` in `flake.nix`, imports the local
`./nixome/topology.nix` schema helper, `./nixome/config-vms.nix`, and
`./nixome/templates.nix`, and exposes normalized `nixome.topology` plus a
template package output:

```nix
let
  system = "x86_64-linux";
  topologySchema = import ./nixome/topology.nix;
  configVms = import ./nixome/config-vms.nix;
  prebuiltTemplateSources = import ./nixome/prebuilt-template-sources.nix;
  templateSet = import ./nixome/templates.nix {
    inherit system templateBuilders prebuiltTemplateSources;
  };
  config = configVms {
    extraTemplates = templateSet.topology.templates;
    extraDomains = templateSet.topology.domains;
  };
in
{
  packages.${system} = templateSet.packages;
  nixome.config = config;
  nixome.topology = topologySchema.mkTopology config;
}
```

That seed re-declares `config-vault`, `config-staging`, and `config-validator`
as offline AppVMs, so after first boot their Qubes-level runtime topology is
owned by the normal config-staging -> config-validator -> dom0 apply path. It
also declares:

- `packages.x86_64-linux.nixome-nixos-work-template`, a Qubes template source
  directory built from a NixOS guest module list.
- `templates.nixome-nixos-work.source`, normally a prebuilt concrete Nix store
  template source path in the bootstrap image, with fallback support for a
  topology build reference to the flake package output.
- `domains.nixome-nixos-work`, the TemplateVM Qubes object and prefs/features
  for booting the built root image with the dom0-provided Qubes VM kernel.
- `domains.nixome-work`, an offline AppVM based on `nixome-nixos-work`.

The config-system TemplateVM and VM kernel installation remain in the outer
bootstrap dom0 image for now. Normal workload templates can now move into the
staged config path.

The schema currently checks domain classes, labels, Qube/template names,
template and kernel source fields, template build references, JSON-safe
prefs/features, boolean services, and constrained Nixome-owned qrexec policy
rules. Template sources can be concrete directories or references like:

```nix
templates.nixome-nixos-work.source = {
  build = "packages.x86_64-linux.nixome-nixos-work-template";
  subpath = "nixome-nixos-work";
};
```

Managed qrexec service names must start with `nixome.`, bootstrap config
services are reserved, actions are limited to `allow` and `deny`, allow rules
cannot use `@anyvm`, and generated policy cannot target dom0. The schema also
appends a catch-all deny rule for each managed service.

The schema defaults AppVMs to `class = "AppVM"`, `netvm = null`,
`prefs.autostart = false`, `prefs.include_in_backups = false`, empty
`features`/`services`, and an empty generated qrexec policy.

For this first apply proof, the Nix value declares the config AppVMs, the work
TemplateVM/AppVM pair, plus a small observable Qubes feature on
`config-staging`:

```text
nixome-config-applied-message
```

The runtime test then edits `flake.nix` to add `nixome-work-from-config`, an
AppVM based on the validated `nixome-nixos-work` template, and proves that dom0
imports that template and creates the AppVM from the validator-produced
topology. It also declares a generated `nixome.PolicyProbe`
qrexec policy rule and proves dom0 can call that service in `config-vault`
after apply.

`config-validator` validates candidates exported from `config-staging`,
evaluates the schema-normalized `.#nixome.topology` to the `topology.json` apply
artifact, dry-runs each declared template build reference with `nix build
--dry-run`, and writes validation JSON under:

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

That command performs the same staging-to-validator-to-vault flow, resolves
template build references, applies the resolved topology with
`nixome-setup-config --apply`, applies the resolved `nixome.system` dom0 plan,
then asks `config-vault` to archive a deployment record. For each
`templates.<name>.source.build`, dom0 builds the referenced package from the
exact validated Git commit, verifies the built output contains `root.img` and
`template.conf`, rewrites the topology source to that concrete store path, and
passes the resolved `topology.json` to setup. For
`dom0.nixos.system.source.build`, dom0 builds the referenced NixOS system
generation from the same commit, installs it as `/nix/var/nix/profiles/system`,
and runs its `bin/switch-to-configuration` with the declared `switchAction`.
Declared `dom0.systemd.services.<name>.action = "restart"` or `"reload"`
entries are executed after the host generation switch and recorded as
machine-readable JSON. Dom0 transports and applies the artifact, but the
candidate validity decision plus Nix topology/system evaluation happen in
`config-validator`.

dom0 can report the current config-system state as JSON with:

```bash
nixome-config-status
```

That command reads the active topology, latest setup/apply artifacts,
`/var/lib/nixome/system` state, current system profile links, Qubes runtime
domain list when `qvm-ls` is available, and the Nixome-managed qrexec policy
file. It prefers `/var/lib/nixome/system/current-topology.json`, which
`nixome-config-apply` updates after successful topology setup, and falls back to
the bootstrap topology at `/etc/nixome/qubes-topology.json`. After the vault
accepts a deployment record, dom0 also keeps
`/var/lib/nixome/system/current-deployment.json`,
`current-config-apply.json`, `last-deployment.json`, and
`last-config-apply.json` so status remains useful after transient logs are
rotated. Profile and `/run/current-system` symlinks are reported as both raw
link targets and resolved store paths. The top-level `pending_reboot` object
summarizes the active plan's reboot requirement, reasons, and structured
actions for automation.

The current seed uses generated literal `/nix/store` path pins for Nixome,
`nixpkgs`, and `nixos-qubes` support sources. Candidate Nix commands therefore
run with `--impure --offline --no-write-lock-file`: impure is needed to read
those immutable store paths from the candidate flake, while offline keeps the
path from fetching during validation or apply. A production design should
replace this with a pure, content-addressed input mirror or a validator-built
artifact transport.

The staged dom0 NixOS system is built from the Nixome support modules,
`./nixome/dom0-host.nix`, and any modules listed in
`localConfig.dom0.extraModules`. This gives the editable inside config a direct
path to normal NixOS host changes such as packages, systemd units, boot options,
kernel options, and Qubes package overlays without changing the bootstrap
module layout. `nixome.system` records the apply plan and source references;
the NixOS modules define the actual host closure that dom0 builds and switches.

TemplateVM contents are defined in Nix as well. The seeded
`nixome/templates.nix` turns `templates.definitions` from `nixome/local.nix`
into both flake packages and topology entries. A local config can add:

```nix
templates.definitions.nixome-nixos-dev = {
  role = "dev-template";
  modules = [
    ({ pkgs, ... }: {
      networking.hostName = "nixome-dev-template";
      environment.systemPackages = with pkgs; [
        git
        jq
        ripgrep
      ];
    })
  ];
  appVms.nixome-dev = {
    label = "blue";
    prefs.memory = 1536;
    features."nixome-role" = "dev";
  };
};
```

The generated topology then imports the built template through
`qvm-template-postprocess` if it is missing and creates the declared AppVMs from
that TemplateVM. This keeps Qubes' VM lifecycle and qrexec model intact while
moving guest root contents into the inside Nix config.

The plan separates `live_actions` from `reboot_actions`: a normal
`dom0.nixos.system.switchAction = "switch"` is treated as live, while
`switchAction = "boot"` installs the next boot generation and records a pending
reboot. Kernel, Xen, bootloader, and Qubes package-set requests are also
reported as structured reboot actions with `reboot_required = true`.
Consumers of `nixome.system.v1` require both the structured action arrays and
the summary boolean/reason fields, so older candidates fail validation before
dom0 applies them.

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
`topology.json`, `validated-topology.json`, `template-builds.json`,
`validation.json`, `manifest.json`, `systemd-actions.json`, and relevant dom0
logs. Passed deployments update both `latest-deployment` and
`latest-deployment-attempt`; failed setup or system-apply attempts update only
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
delete undeclared domains. Dom0 host system generation switching and explicit
dom0 systemd restart/reload actions are handled by `nixome-config-apply`, not
by the standalone topology setup command.

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
the generated log directory.

Run the current NixOS config-VM runtime proof:

```bash
make vm-config-flow
```

That target builds and boots the nested config dom0 image, explicitly runs the
dom0 topology setup, starts `config-vault`, `config-staging`, and
`config-validator`, runs `uname -a` through qrexec in all three VMs, initializes
the staging repository and validator workspace, edits and commits `flake.nix`
in `config-staging`, evaluates `.#nixome.topology` in `config-validator`,
dry-runs declared template build references when present, archives the source
and evaluated topology into `config-vault`, imports `nixome-nixos-work` from
the validated concrete template source, checks that `nixome-work-from-config`
was created from that template, and runs `uname -a` inside it through qrexec.
It also checks that
`config-vault` wrote a deployment record for the successful apply and that
generated qrexec policy is installed and active. After apply it also runs
`nixome-config-status` inside dom0 and checks that the JSON summary matches the
applied topology, live Qubes domains, qrexec policy, and current system link.
The machine-readable result is copied to:

```text
/run/media/user/data/nixome/logs/vm-gui-config-vms-smoke.json
```

`make vm-config-smoke` remains as a compatibility alias.

## Validated Boundary

The template artifacts are NixOS root images packaged in the shape expected by
`qvm-template-postprocess`. The explicit dom0 setup command has imported the
shared config-system TemplateVM and created the three config AppVMs in the
nested dom0 test environment.
Local tests now also prove that a validated topology can contain a staged
template build reference and that dom0 apply resolves it to a concrete template
source before invoking setup. The full nested proof for the prebuilt work
TemplateVM source and candidate apply path is `make vm-config-flow`.

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

The validated runtime path reaches qrexec in the declared NixOS AppVMs, proves
that `config-vault` has `/home/user/nixome-config-vault` initialized before
qrexec accepts the repository status check, and proves that a staged Nix config
can create a new AppVM that boots and serves qrexec. It also records the
successful apply as a deployment in `config-vault` and proves a staged
Nixome-owned qrexec policy rule by calling `nixome.PolicyProbe` through
`qrexec-client`. The next boundary is hardening the model: split role-specific
guest roots when useful, classify setup changes that require VM restart or dom0
reboot, and validate the same path on physical Xen-capable hardware.
