# Config System

The config system is Nixome's first self-hosted topology slice. Dom0 still owns
Qubes control authority, but the desired Qubes topology and dom0 system plan
come from a Nix flake edited inside dedicated config VMs.

## Declared Objects

`machines/config-system.nix` declares:

- `config-vault`, an offline closure-backed NixOS `StandaloneVM`.
- `config-staging`, an offline closure-backed NixOS `StandaloneVM`.
- `config-validator`, an offline closure-backed NixOS `StandaloneVM`.
- the Qubes VM kernel used by those NixOS domains.
- bootstrap qrexec policy for staging export, validator checks, vault archival,
  and deployment records.
- a boot-time `nixome-setup-config.service` that ensures the bootstrap topology
  exists after Qubes dom0 services are up.

The bootstrap policy remains dom0/NixOS-owned in `30-nixome-config.policy`.
Staged config cannot remove dom0's ability to fetch, validate, archive, or
apply candidates. Additional Nixome-owned qrexec policy is declared in staged
topology and rendered by `nixome-setup-config` to:

```text
/var/lib/nixome/qubes-policy.d/90-nixome.policy
```

Static Qubes policy is still assembled by Nix under `/etc/qubes/policy.d`.
Nixome prepares `/var/lib/nixome/qubes-policy.d` as the runtime combined policy
directory, symlinks static rules into it, keeps `90-nixome.policy` mutable, and
starts `qubes-qrexec-policy-daemon` with that runtime policy path.

## Domain Construction

Nixome supports two domain construction families.

Traditional Qubes domains keep the mature TemplateVM/AppVM model:

```nix
domains.sys-net = {
  backend.type = "qubes-template";
  template = "fedora-44-xfce";
  label = "red";
  netvm = null;
};
```

These are ordinary Qubes domains. Fedora, Debian, `sys-net`, `sys-firewall`,
`sys-usb`, and other conventional guests stay on this path unless there is a
specific reason to move them.

NixOS domains use the `nix-closure` backend:

```nix
domains.work = {
  backend = {
    type = "nix-closure";
    system.sourceRef.build = "nixosConfigurations.work.config.system.build.toplevel";
    kernel.name = "6.12.64-1.fc41";
    root.sourceRef = {
      build = "packages.x86_64-linux.work-root-image";
      subpath = "root.img";
    };
    store.sourceRef = {
      build = "packages.x86_64-linux.work-store-closure";
      subpath = "";
    };
  };
  label = "green";
  netvm = null;
};
```

A `nix-closure` domain is represented as:

```text
Nix system closure
+ small root image
+ read-only closure store volumes
+ private persistent state
+ volatile state
+ VM resources
+ devices/networking
+ qrexec/security policy
```

Dom0 resolves build references from the validator-approved candidate, imports
the small root image into the Qubes `root` volume, admits the exact closure into
the Nixome closure store, creates separate Qubes volumes for the selected
generation, and sets prefs/features/services/policy. The EROFS metadata volume
is attached at `/dev/xvde`. The filtered blob-data volume is attached at
`/dev/xvdf`. Early guest bootstrap mounts `/dev/xvde` read-only at
`/nix/store` with `/dev/xvdf` as its external EROFS blob device before the real
NixOS init runs. The topology schema currently constrains those devices because
that is the validated Qubes/libvirt ordering for this path.

The default shared-closure backend keeps one canonical physical representation
for each admitted Nix store object. Object data is stored as deterministic
POSIX tar fragments in:

```text
/var/lib/nixome/closure-store/objects/data.blob
/var/lib/nixome/closure-store/objects/blob-metadata/<store-entry>.json
```

Each exact closure gets:

```text
/var/lib/nixome/closure-store/views/by-closure/<closure>/store.erofs
/var/lib/nixome/closure-store/views/by-closure/<closure>/store.blob-map.json
/var/lib/nixome/closure-store/views/by-closure/<closure>/object-refs
```

`store.erofs` is a small EROFS tar-index metadata image containing only the
declared closure namespace. `store.blob-map.json` maps that closure's logical
external-data stream to canonical object extents in `objects/data.blob`. If two
closures include the same Nix store path, their blob maps point at the same
canonical object offset and hash.

At VM start, the Qubes `nixome-closure` storage pool asks the storage engine to
create a read-only device-mapper view from the closure blob map. The guest
receives only that filtered `/dev/mapper/nixome-*` block device as `store-data`;
it never receives the global object blob or an unfiltered global store mount.
Root inside the guest can read only the bytes referenced by the declared
closure view.

Each domain generation records an exact closure manifest under:

```text
/var/lib/nixome/closure-store/domains/<domain>/generations/<n>/manifest.json
```

The Qubes-facing volumes are produced by the `nixome-closure` storage pool.
The object pool, closure views, and generation manifests are the authoritative
state. `shared-closure` with `erofs-objectmap` is the only supported NixOS VM
store backend.

## Warm Integration Loop

Use local tests first:

```bash
make test
```

For repeated real Qubes/Xen checks against an already-created nested dom0 disk,
reuse the warm targets:

```bash
make vm-config-warm-flow
make vm-nix-closure-warm-flow
```

`make vm-nix-closure-warm-flow` imports the current local closures for
`nixome-setup-config`, `nixome-nix-closure-domain-smoke`, and the
`nixome-closure` Qubes storage adapter into the running nested dom0 before it
runs the focused NixOS-domain lifecycle test. It also sets
`NIXOME_VM_SKIP_BASE_BUILD=1`, so it reuses the existing nested base-image
result link instead of resolving a new base image for every warm iteration. The
synced commands are exposed under:

```text
/run/nixome-warm/bin
```

and `qubesd.service` receives a temporary `/run/systemd/system` override that
prepends the synced storage adapter and the required `dmsetup`/`losetup` tools.
This makes warm iterations test the current checkout without recreating the
whole nested dom0 disk.

The cold targets remain the reproducibility checks:

```bash
make vm-config-flow
make vm-nix-closure-flow
```

## Config Roles

The bootstrap config roles have fixed names and separate persistent state:

```text
config-vault      -> config-vault-private
config-staging    -> config-staging-private
config-validator  -> config-validator-private
```

All three default to `netvm = null`, disable `network-manager`, and use normal
qrexec. Their security boundary is Qubes' domain boundary plus the bootstrap
qrexec policy.

`config-staging` owns the editable candidate repository:

```text
/home/user/nixome-config
```

`config-validator` validates exported candidates, evaluates
`.#nixome.topology` and `.#nixome.system`, and dry-runs referenced builds.
Because the VM system store is read-only, config validation runs Nix through
`nixome-config-nix`, which uses a private writable chroot store rooted at:

```text
/home/user/.local/share/nixome/nix-root
```

That keeps `/nix/store` as the immutable system identity of the running guest
while still allowing `config-staging` and `config-validator` to lock, evaluate,
and dry-run candidate flakes.

`config-vault` owns the historical archive:

```text
/home/user/nixome-config-vault
```

It records:

```text
candidates/<candidate-commit>/
deployments/<timestamp>-<candidate-prefix>/
latest-candidate
latest-deployment
latest-deployment-attempt
```

## Staged Edit Surface

Inside `config-staging`, edit:

```text
/home/user/nixome-config/nixome/machine.nix
```

The high-level Nix-native domain surface is:

```nix
nixNative.domains.work = {
  label = "green";
  netvm = null;
  packages = with pkgs; [ git jq ];
  etc."nixome/domain-role".text = "work\n";
};
```

That expands to a guest NixOS system, a small root image, a read-only store
image, and a normalized `backend.type = "nix-closure"` topology entry.

The same file can declare conventional Qubes TemplateVM-backed domains with
the generic `domains` or `topology.extraDomains` surfaces. Those remain
first-class for Fedora, Debian, and other non-NixOS guests.

Dom0 host changes are declared under:

```nix
dom0.host = {
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
};
```

These fields compile into the candidate `nixosConfigurations.nixome-dom0`
closure. Dom0 builds and switches that closure only after validator approval.

## Apply Flow

The normal flow is:

```text
config-staging
    exports committed candidate
        ↓
config-validator
    validates and evaluates topology/system
        ↓
dom0
    archives attempted candidate to config-vault
    resolves build references
    applies topology and dom0 system plan
        ↓
config-vault
    records deployment result
```

Run from dom0:

```bash
sudo nixome-config-apply --apply
```

`nixome-config-apply` resolves every `nix-closure` source reference to a
concrete store path before invoking `nixome-setup-config`. It records
materialized domain artifacts, topology setup output, dom0 system apply output,
and deployment metadata.

`nixome-config-status` reports the active topology, Qubes runtime domains,
current deployment records, current host generation links, pending reboot
state, pending VM restart state, and the Nixome-managed qrexec policy file as
JSON.

## Setup Path

The dom0 module writes the desired bootstrap topology to:

```text
/etc/nixome/qubes-topology.json
```

Apply it explicitly with:

```bash
sudo nixome-setup-config --apply
```

The nested config image also enables `nixome-setup-config.service` at boot.
That one-shot unit runs after `libvirtd`, `qubes-db-dom0`, `qubesd`, and
`qubes-qrexec-policy-daemon`. A fresh image should come up with the three
config VMs present. Existing matching objects are left alone, so later runs are
normally no-ops. There is no timer.

## Remaining Architecture Work

The default `shared-closure` path physically shares canonical Nix store object
fragments across unrelated NixOS domains and generations, while preserving
independent read-only closure views. Remaining storage work is narrower:

- compacting `objects/data.blob` after GC; today stale object metadata is
  removed, but the central object blob file is append-only.
- reducing object admission overhead if tar fragment creation becomes a
  bottleneck for large store paths.
- running repeated warm and final cold Qubes/Xen lifecycle tests for the EROFS
  external-data mount path.
