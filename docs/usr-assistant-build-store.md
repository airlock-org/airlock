# usr-assistant's pool-backed build store

The assistant role was previously named `sys-agent`. Dated acceptance below
describes the existing builder mechanism; it does not establish physical
acceptance of the [rename transition](usr-assistant-rename.md).

The former sys-dev and sys-ctrl builders passed physical acceptance on 2026-09-29:
approved host inputs, additive base transitions, saved additions and full reboot
persistence. The final normal deployment boots both builder services successfully;
see [completed 07](completed/07-unified-host-guest-store.md) for scope and evidence.

The USB seed supplies a builder view containing guest closures, selected build
inputs, offline sources and the approved host closure. Its EROFS index and Nix
registration are additional metadata; package extents are shared with other
pool views rather than copied into the builder's writable layer.

Qubes attaches two read-only volumes to usr-assistant: `builder-store` (`xvdg`) and
`builder-data` (`xvdh`). The builder-specific index contains `.airlock-builder/`
with registration and identity metadata. An empty administrative `.links`
directory is supplied by the read-only lower scaffold.
It is created by copying only the canonical index and adding these small files
with EROFS incremental image construction; shared package extents are unchanged.
The image tools include upstream's incremental-data fix for erofs-utils 1.9.
The ordinary guest index is not modified. A persistent physical pin
protects this generation even while the VM is stopped. Host publication can
prepare a successor that retains every registered predecessor object. Qubes
attaches its two volumes atomically during a stopped setup; a running controller
continues using its current base until the next stopped setup. No new
guest-to-dom0 control service is added.

This keeps usr-assistant at eight boot disks, avoiding the invalid SCSI address 8
observed with the previous nine-disk layout, without changing Xen or Qubes.
This change targets a freshly built/flashed USB; there is no migration path.

At boot, `airlock-usr-assistant-build-store.service` (or its sys-ctrl counterpart) mounts
the approved objects before loading Nix registration, so saved build recipes are
readable. Metadata loading disables build-user ownership changes to the sealed
mount, then freezes the lower database read-only. Normal `nix`, `airlock-config-nix`
and machine-build/admission commands use Nix's experimental local overlay store:

- `/run/airlock-builder/`: read-only inputs, lower database and readiness record.
- `/rw/airlock-builder/upper`: only downloaded/built additions.
- `/rw/airlock-builder/merged`: combined store and persistent writable Nix state.
- `/rw/airlock-builder/base.json`: root-owned base identity, checked at each boot.

A missing base or an identity change without a matching dom0-issued predecessor
binding fails closed. An approved additive transition is accepted before mounting
the saved upper; its registration remains valid against the larger lower view.
Shutdown keeps the upper and physical pin. A stopped attachment transfers the
old base's physical retention to the successor, which includes all prior inputs.
This does not shrink a controller's approved input set or erase saved outputs.
Missing build-only dependencies still require downloads/builds.

See [completed 07](completed/07-unified-host-guest-store.md) for host boot,
independent recovery, publication ordering and combined acceptance results.

## Fresh-image acceptance

1. Check `systemctl status airlock-usr-assistant-build-store` and
   `/run/airlock-builder/ready.json`; confirm both builder devices are read-only
   and no `builder-metadata` volume or `xvdi` dependency remains.
2. Query an existing guest system through `airlock-config-nix path-info`, build
   a small change, and confirm unchanged input paths are absent from `upper`.
3. Reboot the machine and repeat the build: base identity and prior output must
   survive, with no input copying. Confirm the persistent pin remains in dom0.
4. Build and admit a real guest change; see the completed trial and evidence in
   [01-sys-dev-machine-build.md](completed/01-sys-dev-machine-build.md).

Pytest reads back the combined EROFS image with its external payload device,
checks registration and package contents, and rejects missing external data.
It also covers metadata/view consistency, shared-object counts, offline pin
retention, Qubes volume serialization, and rejection of a changed/missing base
identity. Physical testing also demonstrated reuse of read-only inputs without
copying them into the private upper layer, retention during a running-VM GC
trial, and full-machine reboot persistence. Offline pin retention is covered
by automated tests, not a physical offline-GC trial.

When a private volume comes from a different builder role, its former base and
upper are archived together beside `/rw/airlock-builder`; they are never mounted
over the new role's independent base. Host publication can seed a missing agent
base from the controller's frozen view, retaining its pinned offline inputs.
