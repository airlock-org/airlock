# sys-dev's pool-backed build store

Implemented locally; fresh-image boot/build/restart validation is still pending.
No sys-ctrl or activation changes are included.

The USB seed supplies a frozen `sys-dev-builder` pool generation containing the
union of the image's guest closures. This is a **view**, not another VM or a
second copy of the packages. Its small EROFS index and Nix registration metadata
are additional; its package extents are shared with the ordinary guest views.

Qubes attaches two read-only volumes to sys-dev: `builder-store` (`xvdg`) and
`builder-data` (`xvdh`). The builder-specific index contains `.airlock-builder/`
with registration and identity metadata, plus Nix's empty `.links` directory.
It is created by copying only the canonical index and adding these small files
with EROFS incremental image construction; shared package extents are unchanged.
The image tools include upstream's incremental-data fix for erofs-utils 1.9.
The ordinary guest index is not modified. A persistent physical pin
protects this generation even while sys-dev is stopped. Guest OS upgrades do not
replace the builder base. No new guest-to-dom0 control service is added.

This keeps sys-dev at eight boot disks, avoiding the invalid SCSI address 8
observed with the previous nine-disk layout, without changing Xen or Qubes.
Existing three-disk installations are refused rather than silently migrated;
this change targets a freshly built/flashed USB.

At boot, `airlock-sys-dev-build-store.service` mounts the base and registers its
metadata in a read-only lower Nix database. Normal `nix`, `airlock-config-nix`
and machine-build/admission commands use Nix's experimental local overlay store:

- `/run/airlock-builder/`: read-only inputs, lower database and readiness record.
- `/rw/airlock-builder/upper`: only downloaded/built additions.
- `/rw/airlock-builder/merged`: combined store and persistent writable Nix state.
- `/rw/airlock-builder/base.json`: root-owned base identity, checked at each boot.

A missing base or changed identity fails closed; no silent empty-store fallback
or automatic rebase. Shutdown unmounts the view but keeps the upper and pin.
Do not release `builder-persistent` while any saved upper still depends on it.
Base replacement/cache disposal is intentionally an explicit future operation.
An older private store at `/rw/home/user/airlock-config-nix-root` is left untouched;
there is no bulk-copy migration. Remote Nix build hooks are disabled for this
local-only store. Missing build-only dependencies still require downloads/builds.

## Fresh-image acceptance

1. Check `systemctl status airlock-sys-dev-build-store` and
   `/run/airlock-builder/ready.json`; confirm both builder devices are read-only
   and no `builder-metadata` volume or `xvdi` dependency remains.
2. Query an existing guest system through `airlock-config-nix path-info`, build
   a small change, and confirm unchanged input paths are absent from `upper`.
3. Restart sys-dev and repeat the build: base identity and prior output must
   survive, with no input copying. Confirm the persistent pin remains in dom0.
4. Resume the real guest build/inactive-admission trial in
   [01-sys-dev-machine-build.md](todo/01-sys-dev-machine-build.md).

Pytest reads back the combined EROFS image with its external payload device,
checks registration and package contents, and rejects missing external data.
It also covers metadata/view consistency, shared-object counts, offline pin
retention, Qubes volume serialization, and rejection of a changed/missing base
identity. These checks are not a substitute for the pending physical boot test.
