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
- a boot-time `nixome-setup-config.service` that ensures this topology exists
  after the Qubes dom0 control services are up.

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

For this POC the three config roles default to one NixOS guest TemplateVM. That
shared image carries the vault archive tooling, staging export tooling, and
validator tooling; the AppVM names, Qubes features, offline networking, and
qrexec policy carry the runtime role distinction. The staged config can switch
to separate role-specific TemplateVMs with:

```nix
configVms.templateMode = "dedicated";
```

That emits dedicated `nixome-nixos-config-vault`,
`nixome-nixos-config-staging`, and `nixome-nixos-config-validator` TemplateVM
imports whose source packages are built from the same validated candidate
flake. The default remains `"shared"` to keep nested developer image builds and
first-boot setup fast.

`configVms.sharedGuest` customizes the shared config-system TemplateVM.
`configVms.guests.config-vault`, `configVms.guests.config-staging`, and
`configVms.guests.config-validator` customize the dedicated role TemplateVMs.
Those guest entries support the same high-level content fields as normal
template definitions: `hostName`, `packages`, `etc`, `systemd`,
`configuration`, and `modules`. Staged template definitions and config-role
guest definitions reject unknown option names during Nix evaluation, so common
typos fail in `config-validator` instead of being ignored.

The staged config now takes the next step for normal workload templates: it
defines `nixome-nixos-work` as a NixOS TemplateVM package output and declares
`nixome-work` as an AppVM based on that template.

## Domain Construction Model

Nixome intentionally supports two domain construction families.

Traditional Qubes domains keep the mature TemplateVM/AppVM model:

```nix
domains.sys-net = {
  backend.type = "qubes-template";
  template = "fedora-44-xfce";
  label = "red";
  netvm = null;
};
```

These domains are ordinary Qubes domains. Fedora, Debian, and other existing
Qubes templates should stay on this path unless there is a concrete reason to
move them. The POC uses this path for the config VMs and the initial
NixOS-built workload template.

Nix-native domains are represented as a declared system identity plus state and
runtime policy:

```nix
domains.work = {
  backend = {
    type = "nix-closure";
    system.sourceRef.build = "nixosConfigurations.work.config.system.build.toplevel";
    kernel.name = "6.12.64-1.fc41";
    store = {
      mode = "closure-image";
      sourceRef.build = "packages.x86_64-linux.work-store-image";
    };
  };
  label = "green";
  netvm = null;
};
```

The intended long-term invariant is:

```text
Nix-native domain =
    Nix system closure
    + persistent state
    + volatile state
    + VM resources
    + devices
    + networking
    + qrexec policy
```

The closure is the immutable system identity of that domain. The eventual
storage target is a per-domain read-only `/nix/store` closure image or view,
with physical sharing below the VM boundary rather than a live shared
filesystem service. That preserves independent VM runtime behavior while
allowing common immutable Nix store objects to be deduplicated by the storage
backend.

This is not the active boot path yet. The schema and validator can describe and
check `nix-closure` domain references, but `nixome-setup-config --apply`
currently refuses those domains before mutating Qubes state with:

```text
nix-closure domain materializer is not implemented yet
```

That fail-fast behavior keeps the current POC honest: TemplateVM-backed domains
continue to work, while closure-backed domains cannot be silently created as
the wrong Qubes object type.

`nixome-config-apply --apply` performs the same check immediately after
candidate validation and archival. If a candidate declares a `nix-closure`
domain today, dom0 records a failed deployment attempt in `config-vault` and
stops before building template sources, building system sources, or invoking
`nixome-setup-config`.

`config-staging` owns the editable candidate repository at:

```text
/home/user/nixome-config
```

It can initialize that candidate with:

```bash
nixome-config-staging-init
```

It can run the same local validation model used by the validator implementation
with:

```bash
nixome-config-staging-validate
```

That command exports the clean committed staging repo to a Git bundle, runs the
validator checks locally, and writes validation JSON plus evaluated
`topology-$commit.json`, `system-$commit.json`, and the validated candidate tar
under `/home/user/nixome-config-validation`. This is developer feedback only:
dom0 still asks `config-validator` to validate the candidate again before
archiving or applying it.

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
owned by the normal config-staging -> config-validator -> dom0 apply path. The
seed also exposes optional role-specific config TemplateVM package outputs for
the dedicated mode. It also declares:

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

The schema currently checks domain backend types, domain classes, labels,
Qube/template names, template and kernel source fields, template and
Nix-native source references, JSON-safe prefs/features, boolean services, and
constrained Nixome-owned qrexec policy rules. Template sources can be concrete
directories or references like:

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
`features`/`services`, `backend.type = "qubes-template"`, and an empty
generated qrexec policy. `backend.type = "qubes-template-root"` is used for
TemplateVM Qubes objects, `backend.type = "qubes-standalone"` for traditional
standalone Qubes objects, and `backend.type = "nix-closure"` for the future
closure-backed domain path.

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
artifact, evaluates `.#nixome.system` to the `system.json` host plan artifact,
dry-runs declared template and system build references with `nix build
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
the bootstrap topology at `/etc/nixome/qubes-topology.json`. It prefers the
latest setup result from the log directory, falling back to the durable
`/var/lib/nixome/system/current-setup.json` when transient logs are gone. After
the vault accepts a deployment record, dom0 also keeps
`/var/lib/nixome/system/current-deployment.json`,
`current-config-apply.json`, `last-deployment.json`, and
`last-config-apply.json` so status remains useful after transient logs are
rotated. Profile and `/run/current-system` symlinks are reported as both raw
link targets and resolved store paths. The top-level `pending_reboot` object
summarizes the active plan's reboot requirement, reasons, and structured
actions for automation. The top-level `pending_vm_restarts` object summarizes
Qubes domain changes that were applied while the affected domain was running.
The top-level `host_generation` object is the compact dom0 generation summary:
it reports whether `/nix/var/nix/profiles/system` matches
`/run/current-system`, the currently running system store path, any staged
boot-generation source, the last dom0 system apply result, and the same
pending-reboot decision in one place.

The current seed uses generated literal `/nix/store` path pins for Nixome,
`nixpkgs`, and `nixos-qubes` support sources. Candidate Nix commands therefore
run with `--impure --offline --no-write-lock-file`: impure is needed to read
those immutable store paths from the candidate flake, while offline keeps the
path from fetching during validation or apply. A later production design should
replace this with a pure, content-addressed input mirror or a validator-built
artifact transport.

The staged dom0 NixOS system is built from the Nixome support modules,
`./nixome/dom0-host.nix`, and host fields in `nixome/machine.nix`. The normal
edit surface is:

```nix
dom0.host = {
  apply.mode = "auto";

  overlays = [
    (final: prev: {
      # Example: override or replace dom0 packages here.
    })
  ];
  packages = with pkgs; [ vim ];
  etc."nixome/example".text = "hello\n";
  systemd.services.nixome-example = {
    description = "Nixome example service";
    serviceConfig.Type = "oneshot";
    script = "${pkgs.coreutils}/bin/true";
  };
  configuration = {
    boot.kernelParams = [ "loglevel=4" ];
  };
  modules = [ ];
};
```

These fields compile into normal NixOS modules inside
`nixosConfigurations.nixome-dom0`, so they can affect packages, systemd units,
boot options, kernel options, and Qubes package overlays without changing the
bootstrap module layout. Use `dom0.host.overlays` for ordinary dom0
package/version overrides, including Qubes package overrides. `dom0.extraModules`
remains as a compatibility escape hatch, but new config should prefer
`dom0.host`. `nixome.system` records the apply plan and source references; the
NixOS modules define the actual host closure that dom0 builds and switches.
Unknown top-level `nixome/machine.nix`, `dom0`, and `dom0.host` option names
fail during Nix evaluation.

Host apply classification is automatic by default. Changes limited to
`dom0.host.packages`, `dom0.host.etc`, and `dom0.host.systemd` are classified
as live dom0 userland changes and use `switch-to-configuration switch`.
Changes that touch `dom0.host.overlays`, `dom0.host.configuration`,
`dom0.host.modules`, or `dom0.extraModules` are classified conservatively as a
next-boot generation because those surfaces can affect boot, Xen, package
overrides, kernel settings, or early services. A candidate can override the
default when the operator knows the class:

```nix
dom0.host.apply = {
  mode = "switch"; # force live switch
  reason = "operator classified this host module as live-safe";
};
```

Use `mode = "boot"` to force a next-boot generation. The lower-level
`system.args.dom0SystemSwitchAction` field still exists as an escape hatch and
overrides this classifier.

TemplateVM contents are defined in Nix as well. The seeded
`nixome/templates.nix` turns `templates.definitions` from `nixome/machine.nix`
into both flake packages and topology entries. A local config can add:

```nix
templates.definitions.nixome-nixos-dev = {
  role = "dev-template";
  sourceLabel = "local-config";
  hostName = "nixome-dev-template";
  packages = with pkgs; [
    git
    jq
    ripgrep
  ];
  etc."nixome/template-role".text = "dev\n";
  configuration = {
    programs.bash.shellAliases.ll = "ls -l";
  };
  appVms.nixome-dev = {
    label = "blue";
    prefs.memory = 1536;
    features."nixome-role" = "dev";
  };
};
```

Template definitions can use `configuration = { ... };` for ordinary NixOS
module options, or append raw NixOS modules with
`modules = [ ({ pkgs, ... }: { ... }) ]` when a function module is needed. The
high-level fields, `configuration`, and raw modules compile into the same NixOS
module system before the root image is built.

The generated topology then imports the built template through
`qvm-template-postprocess` if it is missing and creates the declared AppVMs from
that TemplateVM. This keeps Qubes' VM lifecycle and qrexec model intact while
moving guest root contents into the inside Nix config.

The plan separates `live_actions` from `reboot_actions`: a classified live
dom0 generation uses `dom0.nixos.system.switchAction = "switch"`, while a
classified boot generation uses `switchAction = "boot"` and records a pending
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

The config-system dom0 image also enables `nixome-setup-config.service`.
That one-shot unit runs at boot after `libvirtd`, `qubes-db-dom0`, `qubesd`,
and `qubes-qrexec-policy-daemon`, so a fresh image should come up with the
config TemplateVM imported and `config-vault`, `config-staging`, and
`config-validator` present without a manual first setup command. The reusable
module keeps this boot behavior disabled by default; the config-system profile
opts into it. There is no timer. Existing objects are left in place when they
already match, so later boots should usually report no changes.

For a non-mutating preview:

```bash
sudo nixome-setup-config --plan
```

The setup command currently imports missing templates, creates missing domains,
and sets global prefs plus simple domain prefs/features/services. It records
changed objects in `live_actions`, and records affected running domains in
`restart_actions` with `restart_required = true` when a changed pref, feature,
or service may require the Qube to restart before the new setting fully takes
effect. A successful `--apply` writes the active topology and setup result to
`/var/lib/nixome/system/current-topology.json` and
`/var/lib/nixome/system/current-setup.json`, so a freshly set up machine remains
observable even after transient logs are cleaned. It is
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
make vm-config-boot-flow
make vm-config-boot-activate-flow
make vm-config-template-flow
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

`make vm-config-boot-flow` runs the same validated candidate/apply path with
`dom0.nixos.system.switchAction = "boot"`. The apply installs the staged system
as the next dom0 boot generation, records a structured reboot action, and
leaves `/run/current-system` on the old generation.

`make vm-config-boot-activate-flow` adds the second boot. It preserves the same
qcow2 overlay after the staged apply, boots it again, and runs
`nixome-config-boot-verify` in dom0. The verifier checks that the host system
profile and `/run/current-system` resolve to the staged generation and that the
active status JSON no longer reports a pending reboot.

`make vm-config-template-flow` enables
`NIXOME_CONFIG_BUILD_EXTRA_TEMPLATE=1`. In that mode the staged config adds a
second TemplateVM definition with no prebuilt source, so dom0 resolves the
validated flake build reference, imports the built TemplateVM, creates an AppVM
from it, starts the AppVM, and checks qrexec.

## Validated Boundary

The template artifacts are NixOS root images packaged in the shape expected by
`qvm-template-postprocess`. The dom0 setup path has imported the
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
`qrexec-client`. The two-boot path now also proves that a staged inside config
can become the running dom0 NixOS boot generation under nested Xen. The next
boundary is hardening the model: split role-specific guest roots when useful,
classify setup changes that require VM restart or dom0 reboot, and validate the
same path on physical Xen-capable hardware.
