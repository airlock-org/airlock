# Build routine guest configuration changes offline

Status: completed for the two documented guest changes on the 2026-09-24 USB
image, including physical offline builds, pool cleanup, and reboot. Extends the
implemented [pool-backed build store](../usr-assistant-build-store.md).

## Goal and boundary

On a freshly provisioned image, sys-dev must be able to evaluate and build a
documented set of routine guest configuration changes without internet access
or help from the development host. Use the image's pinned software versions
and existing guest software. This makes local configuration work possible
when networking or external caches are unavailable, including changes needed
to repair networking. Saving download time is a secondary benefit.

Start with a new `/etc` file and a representative service-setting change that
uses existing software. Record the exact supported examples and their targets;
do not infer support for every configuration option from one successful build.
The resulting guest closure must be newly built, not a preloaded final result.

Offline support is bounded by the inputs shipped with that image. Adding absent
packages, changing pinned versions, or rebuilding packages from source may
require additional binaries, sources and build tools. Those can be obtained
beforehand through a download or explicit import. This task does not promise
arbitrary new software or complete source rebuilds offline. Its acceptance
boundary is guest evaluation and build; include no sys-ctrl implementation or
changes to activation authority.

## Determine what to seed and pin

1. Define the supported configuration changes against the image's committed
   machine source and lock file. Use these examples to drive input selection.
2. Start with the guest-build trial's download list in
   `.airlock-logs/pool-trial-20260918T223925Z/first-build.log`, linked from the
   [completed guest-build trial](01-sys-dev-machine-build.md).
   It fetched 12 store paths, reported as 734.5 KiB downloaded and 11.1 MiB
   unpacked. These are candidate inputs for the baseline, not proof that the
   current image or every supported change needs exactly that set.
3. Account for both evaluation and build inputs. Evaluation needs the machine
   checkout and all required transitive flake sources, resolvable locally from
   the pinned lock file. Build inputs include the tools, hooks and package
   outputs needed to construct the changed guest system. Prefer upstream Nix
   mechanisms such as `nix flake archive` for collecting flake sources; archiving
   those sources alone does not supply all build dependencies.
4. Evaluate and build each example from a clean builder upper and fresh user
   caches with external networking unavailable. Use missing-input failures and
   a separate connected diagnostic run to identify gaps. Map needed paths to
   explicit Nix package/output or source references at the image's pins; do not
   hard-code the old trial's store hashes or seed an entire warmed private store.
5. Compute the complete dependency closure of the selected inputs and compare
   it with the existing builder view. Measure additional unique pool data,
   accounting for sharing with guest views, and verify the built image fits the
   provisioned USB. An isolated comparison against an otherwise identical
   image is optional for this milestone. Repeat the clean offline trials until
   every supported example succeeds. Record why each selected root is needed
   and repeat this acceptance when the pinned inputs or supported examples change.

The existing `offline_inputs_ready` preflight field checks only
`flake metadata --offline`; it is not evidence that full evaluation and build
will succeed offline. Likewise, an "unpacking into the Git cache" log line
does not by itself prove a network download. Actual disconnected builds are
the acceptance criterion.

## Local implementation

The image seed declares a small set of Nix package roots covering the trial's
common hooks and tools, including composefs outputs, `file`, `gnu-config`,
`patchelf`, the no-compiler standard environment, and relevant wrapper and
verification hooks. The `usb-dev-image` preparation archives the committed
machine flake's pinned sources with Nix and records their store paths and lock
hash. The builder view includes those paths, and sys-dev's guest closure also
exposes the source paths at `/nix/store` so Nix can resolve path inputs without
copying them into the private builder upper. The sys-dev command accepts
`--offline` immediately after `preflight`, `build`, or `admit`; it checks the
checkout lock against the image manifest, resolves archived sources locally,
and disables substitutes. For example:

```bash
airlock-sys-dev-machine-build preflight --offline
airlock-sys-dev-machine-build build --offline
```

The selected roots were qualified by the isolated check and the fresh-USB
trial below. This supports the two documented changes at the image's pins;
changes outside that set may need more inputs.

## Input preparation before image assembly

`make usb-dev-image` runs `make usb-dev-prepare-inputs`, then checks the
realized builder closure before building the image. The automatic cached
acceptance was added in [task 07](07-cached-offline-builder-check.md).
Preparation requires a clean committed machine checkout, archives its pinned
flake sources without updating the lock, validates their manifest entries and
records the lock hash. It stops before image assembly if preparation fails.
The normal image build realizes and includes the declared builder input closure
as part of its dependency graph.

This preparation may fetch pinned sources that are not available locally. The
following cached acceptance checks the two documented examples; it does not
prove that arbitrary guest changes work offline. Image creation can still take
time to build dependencies and assemble the disk.

## Separate offline acceptance check

Run `make usb-builder-offline-check` to force fresh trials after changes to the
machine configuration, flake pins, declared builder inputs, or the acceptance
examples and their runner. Ordinary image builds reuse a matching passing
receipt or run both trials before image assembly. The former command
`make usb-builder-offline-gate` remains an alias for the forced check.

The check prepares the pinned flake sources and builder input closure, then
tries two committed-source variants of usr-home: one adds an `/etc` marker and
one changes an existing service setting. These build changed guest
configuration outputs using the seeded software; they do not assemble USB
images. They establish support for the documented examples, not every possible
configuration change.

Each trial starts with a separate empty writable store and cache. Only paths in
the declared builder closure are registered as baseline inputs. Bubblewrap
maps the entire temporary Nix store and database under `/nix`, mounts each
selected store object read-only once, and removes network access. Nix runs
against that temporary store with substitutes disabled and its build sandbox
enabled, with sandbox fallback disabled. This lets builders read
their inputs without following staging links outside the sandbox. All mounts
exist only inside the trial's mount namespace; the host's main `/nix/store`
and Nix configuration are not changed. New outputs and registration data go to
the temporary store. The check rejects a preseeded final system result and
returns failure if either trial fails. Results and stderr are saved as
JSON and text under `.airlock-logs/offline-builder-gate/`, or the configured
`AIRLOCK_LOG_DIR`.

The check builds any guest and builder input closures it needs, so it may take
time and temporary disk space. It exercises the Nix inputs and example builds; the
fresh-USB trial below remains necessary to check the actual mounted pool,
sys-dev startup, pin retention and reboot behavior.

## Storage and lifecycle

Declare the selected baseline in Nix and seed its complete closure into the
USB's shared pool, extending the frozen `sys-dev-builder` view. Include the
evaluation sources and any metadata needed to resolve them locally. Expose
those sources in sys-dev's guest store at the same logical paths; the Nix path
fetcher needs to read `/nix/store` even when building into an overlay store.
Preserve the existing image-provided machine checkout. Reuse shared pool
objects rather than copying baseline inputs into sys-dev's private writable
layer.

Protect the baseline with the builder's persistent pool pin, including while
sys-dev is stopped. Build tools need not be installed in a guest's operating
system closure. Preserve registration and base identity checks; never silently
replace the base underneath an existing writable build layer. Use fresh images
for this milestone; migration or rebasing of existing builder layers remains
separate work.

## Fresh-image acceptance

1. Boot a freshly built/provisioned image with no previous builder additions or
   user download caches. Make external networking unavailable before the first
   evaluation/build; do not rely on a successful `--offline` metadata check.
2. Commit and build each supported configuration change inside sys-dev using
   the seeded checkout. Isolate the trials so earlier builds cannot hide missing
   baseline inputs. Confirm the changed outputs were not preseeded, no external
   fetches were needed, and baseline store paths were reused without copying
   them into the upper layer. New derivations and build outputs may use it.
3. Stop sys-dev, run the supported pool cleanup procedure, and confirm the
   persistent pin retains the complete baseline. Verify offline build capability
   still works afterward, including after a whole-machine reboot.
4. Record source revision, lock hash, selected roots, closure manifest, supported
   examples, network isolation, build results, and storage measurements under
   `.airlock-logs/`. Add relevant pytest coverage and use `make test-quick` for
   implementation feedback. Report image build and live acceptance separately.

Any input that still requires an external fetch for a supported example is an
acceptance failure. Document downloads needed by changes outside the supported
set as limits of that image's offline capability.

## Acceptance record: 2026-09-24

The provisioned USB ran machine revision
`7905339d474a5cc0a1accf93d09237c5ce4d9121` with lock hash
`8437acbbe9c01d499002691ede6647a9b5ec9bddf053e53ef210ac95ec8fa71e`.
Both supported changes were committed in separate disposable checkouts and
built in separate empty writable stores over the image's pinned read-only
builder view. Each build ran inside a network namespace with no external
network route and with Nix substitutes disabled. Neither resulting system was
present in the baseline; both appeared in their respective writable stores.
None of those stores' top-level objects duplicated a baseline object.

| Change | Built guest system | Writable store after build |
| --- | --- | ---: |
| `/etc` marker | `racn34by9qywri0ppfg8w73v3nl16xwi` | 28,218,574 bytes |
| Existing service setting | `1aas16n9zjsrscg0zgs3pnj2harr5zr6` | 28,246,125 bytes |

The pinned builder view registered 1,728 distinct store paths. Compared with
the four ordinary guest views on that USB, it added 26 distinct store objects,
whose recorded pool fragment sizes total 11,832,320 bytes. Its separate
builder metadata image was 52,672,000 bytes. These are measured pool costs.
The resulting image was provisioned to and booted from the USB. An isolated
whole-image size delta was not measured because the available older images
also differ in other configuration; fitting the USB is sufficient for this
milestone.

With sys-dev stopped, `airlock-closure-store gc --keep-generations 0` returned
`status: passed` and retained `sys-dev-builder` generation 1. A fresh offline
`/etc` build then passed. After a whole-machine reboot, the builder mounted the
same base identity and another fresh offline `/etc` build passed. The current
local source revision `42ce4161617b81a135f638a04e38765d1d64ef3d` also
passed `make usb-builder-offline-check` for both examples; that check is
separate from the physical trial on the installed revision.

The live trial's report, complete baseline path manifest, input manifest and
build logs are under `.airlock-logs/offline-builder-live-20260924/`. The
independent check writes `.airlock-logs/offline-builder-gate/` or the configured
`AIRLOCK_LOG_DIR`; the dom0 cleanup result is
`/var/log/airlock/offline-builder-gc.json` on the USB.
