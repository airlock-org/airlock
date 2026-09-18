# Repository-first whole-machine deployment

## The development contract

The committed `airlock-machine` repository and the parent repository's pinned `airlock-machine/`
submodule pointer are the source of truth. A running machine is a
deployment target, not a second configuration repository. Implement fixes here,
test and commit them, then deploy those artifacts. Do not copy hand-edited GUI
scripts into `/run`, edit guest store paths, or leave systemd overrides as the
only implementation of a fix. Temporary diagnostic changes must be documented
and removed before claiming that a Nix generation reproduces the result.

The coordinator covers the whole declared machine, not one application/VM.
It uses the existing Xen/Qubes lifecycle, setup-config converger, policy, and
immutable closure-store engine. It does not replace those mechanisms.

The current transport uses sys-dev's existing **development-only full dom0
administrative access**, with a password-protected sudo session. It adds no
dom0 SSH listener or new production qrexec capability. This is not yet the
restricted config-authority/approval workflow described in `config-system.md`.

## Prerequisites and current validation boundary

- A bootstrapped machine started from the `Airlock` boot-menu entry, with
  sys-dev reachable over SSH/Tailscale and access to the dom0 development
  password. The internal deployment profile remains named `usb-lab`; it is not
  a boot-menu label. Use the Tailscale name (which can be `sys-dev-2`); its
  Qubes name remains `sys-dev`.
- Keep a physical dom0 console available for the first trials. Restarting
  sys-dev removes the remote connection; failures may require local recovery.
- Clean, committed `airlock-machine` and parent `airlock` repositories, an initialized
  `airlock-machine/` submodule at the committed pointer, and the guarded Airlock Nix store
  under `/run/media/user/data/airlock`.
- Free space in dom0 **and** sys-dev. The initial deployment imports ordinary
  Nix closures in addition to the image's packed guest store. Budget substantial
  development space, not just space for the compressed USB image. The client
  checks conservative space estimates before uploading. It never resizes
  partitions, runs GC, or deletes retained generations to make an update fit.

At implementation time the live `sys-dev-2` machine's 31-GiB root had only
about 1.4 GiB free. SSH and the password-protected dom0 control transport were
tested, but a full live apply/reboot/rollback cannot be validated on that disk
without provisioning more space. `make usb-dev-provision USB_SERIAL=...` now
expands a freshly flashed root filesystem to the USB capacity before
personalization; see `usb-dev.md`. This erases the old installation as before,
so back up wanted runtime data first. No larger seed image is necessary.
The release build and pytest fault tests are
separate evidence, not a claim of a completed physical deployment. Do not erase
this distinction when handing off to another agent.

Physical follow-up, 2026-09-16: the expanded USB successfully applied release
revision `f8e15558ccc625c84035851727420e7d9b00f98c` and rebooted into its selected
host generation. `airlock-machine-autostart` started `usr-home` automatically;
`machine-verify` returned `complete`, with matching host and running guest
generations and no failed dom0 units. The browser stayed stopped as configured.
The earlier image-to-release apply also survived restarting sys-dev/sys-gui.

Rollback follow-up, 2026-09-16: revision `814b3a4de5d744a5f6d8f83c317166728a097bf3`
removed the temporary `usr-home` generation marker and switched `usr-home` from
`hbnf7k25...` to `n911xmhv...`. Rolling back restored both the prior release and
`hbnf7k25...`, including its marker, while exact sentinels in the persistent
homes of `usr-home` and `sys-dev` survived. Reapplying `814b3a4...` removed the
marker again; after reboot, `machine-verify` returned `complete`, both sentinels
still matched, and dom0 had no failed units. The temporary sentinels were then
removed. This validates one-generation whole-machine rollback and persistent
home preservation; it does not claim application-level data migration rollback.

GNOME's shutdown action was observed stopping only sys-gui, and input forwarding
did not recover after that VM stopped; these are separate unresolved desktop
issues.

## Normal loop

```bash
# Make the change in this repository, then:
make test-quick
git add <the relevant files>
git commit -m 'Describe the change'

make machine-build
make machine-plan DEPLOY_TARGET=sys-dev-2
make machine-apply DEPLOY_TARGET=sys-dev-2
make machine-status DEPLOY_TARGET=sys-dev-2
make machine-verify DEPLOY_TARGET=sys-dev-2
```

`machine-build` needs no remote machine. All commands default to the internal
`usb-lab` deployment profile; this is not a boot-menu entry.
`DEPLOY_PROFILE=usb-gui-lab` builds that complete profile instead, but its usual
topology has no sys-dev bridge; use the dom0 coordinator directly if artifacts
are provisioned by another authorized transport. A deployment cannot silently
change the active boot profile.

`plan` builds/imports missing immutable artifacts and displays the complete
change/restart plan; it does **not** activate a generation or restart VMs.
`apply` asks for confirmation and launches a detached systemd worker in dom0.
Closing the terminal or losing SSH does not terminate that worker. Reconnect
and inspect status; do not interpret a disconnected SSH session as failure.

Guest changes are applied live by restarting affected VMs. Dom0 uses
`switch-to-configuration boot`, not `switch`: physical GPU, Xen, initrd, host
services and host policy changes take effect on reboot. `reboot-required` means
the guests were verified but the selected host generation is not running yet.
After reboot, run `machine-verify` again. Only `complete` means both host and
guest generation checks passed. Window behavior still needs its own functional
test; generation equality alone is not a GUI acceptance test.

At the boot menu, use `Airlock`, which points to the newest selected normal Xen
generation. Retained normal generations are titled `Airlock (generation N)`.
`Airlock (recovery)` is the newest Xen text-recovery entry and does not start
VMs, hand off the GPU or bring up networking automatically. The repository-owned
post-install normalizer applies these names after image builds and deployments.
Recovery is pinned as a separate Nix system profile, so it remains available
when the original image generation falls outside the retained history.

The `airlock-machine` revision and lock-file hash are recorded in the release. The
development-side receipt also records the parent revision. Local build receipts
and GC roots are under `.airlock-logs/machine-deploy/<airlock-dev-commit>/`.
Runtime journals are in `/var/lib/airlock/machine/transactions/<id>/`, including
`status.json`, `staged.json`, and setup's JSON/command logs. Use
`journalctl -u 'airlock-machine-deploy-*'` in dom0 for worker output.

## What is built and applied

The flake output is:

```text
./airlock-machine#airlock-machine-release-usb-lab
```

It contains `release.json` (host system, exact setup/coordinator binaries,
profile, commit and lock hash) and `topology.json` (all declared VMs, policies,
preferences, devices and exact VM systems/root/store sources).

The release is built directly from the pinned `airlock-machine` checkout. The USB
image calls the same OS construction function, then substitutes prepared-store
descriptors only for the topology embedded in the image. Release construction
canonicalizes those entries to ordinary closure manifests for transport and
admission. Both paths name the same VM system paths and cannot drift into
independently maintained host or guest configurations. A standalone release
does not evaluate or build the prepared store or disk image.

The coordinator:

1. Captures the booted image/current topology as the first rollback baseline.
2. Validates the whole topology and calculates dependency-ordered restarts.
3. Pins artifacts and admits changed guest closures without selecting them.
4. Records the desired release before stopping affected VMs, dependants first.
5. Runs the release's exact setup-config binary over the complete topology,
   reusing its staged admissions; private/home volumes are not replaced.
6. Selects the host boot generation, starts VMs in dependency order, and checks
   selected systems and running guests' `/run/current-system` over qrexec.

Unchanged VMs remain running. A first deployment can affect every VM because
the image's prepared descriptor sources are replaced by closure-manifest sources.
Already running VMs and declared autostart VMs are brought back, with their
dependencies. Shutdown timeouts fail the transaction; the tool does not kill VMs.

Deployment admission uses `--verify-mode incremental`: complete canonical object
records in dom0's managed append-only pool are reused without rehashing their
source trees or stored payloads. Both metadata records must agree on identity,
tree hash and extent; backing extents must exist and fit their segment. Missing
records fall back to admission. New paths undergo nonrecursive Nix content
verification, tree hashing and archive admission before generation publication.
The generation manifest records new/reused object counts. Prepared image objects
are trusted under the same build-validated image assumption used at bootstrap.

This assumes the pool and its admission metadata remain dom0-owned and payloads
are not modified after admission. Incremental deployment does not detect silent
bit rot in previously admitted bytes. Explicit `admit --verify-mode nix-store`
retains the full source/closure verification path and payload checks; it can be
used for an integrity investigation rather than on every deployment. Transport
checks, generation verification and fail-closed publication remain in place.

Sys-dev's `airlock-pool-send` now uses `airlock.Pool.AdmitIncremental+TARGET`.
It sends a closure inventory with source tree hashes first. Dom0 checks its
existing pool records (including the prepared USB seed), requests missing
objects, then verifies only the transferred payloads before publishing a retained,
inactive generation. Reused objects are neither packaged nor uploaded; sys-dev
still reads their local contents to compute the inventory hashes. The response
records `objects_transferred`, `payload_bytes_transferred`, and reuse counts.
Admission, GC, compaction, and recovery share a lock to protect negotiated reuse.
Both sys-dev and dom0 need the updated image; the original full-artifact service
remains available, but the new sender does not silently fall back to a full upload.
Fresh-image physical validation passed on 2026-09-18: a committed usr-home
change built in sys-dev admitted 4 new objects (434,176 bytes) and reused 978.
Generation 2 was retained but inactive; generation 1 and the running guest
stayed unchanged. See the [completed build/admission trial](completed/01-sys-dev-machine-build.md).

Measured on the physical USB laptop on 2026-09-16: incremental re-admission of
the already deployed sys-gui closure took 2.13 seconds, reusing all 1,578 objects
(9,488,805,888 logical store bytes) with zero admissions. This was an inactive,
unretained benchmark generation (sys-gui generation 8), not a VM switch. It tests
the fully reused case; a changed closure may still need a new EROFS index and VM
restart, so this is not a two-second end-to-end deployment claim.

The selection survives a normal reboot. Boot setup consumes the selected
release, not the original image topology. Old images receive a repository-owned
compatibility drop-in for setup-config; new images include the coordinator and
profile marker. `/var/lib/airlock/machine/controller` points to a pinned Nix
binary, not a manually maintained live script.

`airlock-machine-autostart.service` starts declared autostart VMs and their
dependencies after topology setup, network-provider startup, and the guarded
physical-GPU startup. This replaces the boot responsibility of the deliberately
masked USB `qubes-vm@` service, without bypassing GPU checks. It consumes the
selected release (or image topology before the first deployment), skips running
VMs, and fails closed on interrupted deployments or a profile mismatch. It does
not resurrect every VM that happened to be open before shutdown.

Reflashing can erase deployment journals and machine data. It does **not** erase
the committed implementation or configuration in this local repo. Building the
next image from that commit reproduces its declared configuration. Runtime user
data, Tailscale enrollment and credentials need separate backups.

## Failures, recovery, and limits

This is a journalled convergent deployment, **not an atomic multi-VM database
transaction**. A failure can leave a mixed running state. The journal records
the failed phase and selected/previous releases; it never reports that as success.
Boot reconciliation refuses an interrupted transaction instead of silently
adopting the image's original topology. Don't run legacy apply tools concurrently.

```bash
make machine-status DEPLOY_TARGET=sys-dev-2
make machine-resume DEPLOY_TARGET=sys-dev-2
# Or explicitly select the previous whole-machine release:
make machine-rollback DEPLOY_TARGET=sys-dev-2
```

If sys-dev is down, use the physical dom0 console:

```bash
sudo /var/lib/airlock/machine/controller status
sudo /var/lib/airlock/machine/controller resume
# Or:
sudo /var/lib/airlock/machine/controller rollback
```

Do this from the matching lab profile. Text recovery can inspect files and boot
entries, but is deliberately not an alternative configuration apply target.
Rollback reselects systems, not user-data snapshots; application data migrations
are not undone. A VM introduced by the rolled-back release is stopped and left
on disk with its private data, and reported as `retained_undeclared`.

Automatic deployment currently supports Nix-closure/shared-closure VMs. It
rejects domain deletion, class/template/storage-layout changes, physical device
reassignment, and deletion of preference/feature/service keys without explicit
reset values. Those need an explicit migration design. A rollback that removes
newly introduced settings can likewise require explicit reset values. Unsupported
changes fail preflight rather than being silently ignored.

There is no automatic old-release pruning. Retained releases cost disk space.
Only use the guarded Airlock GC targets on the development host; never run GC
on its host `/nix/store`. Do not manually delete runtime closure-store segments.

## For the next agent

Start with `git status`, this guide, and the last deployment journal. Keep four
claims separate: source committed, release built, guests verified, host verified.
Add pytest regression coverage before declaring an incident fixed. The primary
feedback loop is `make test-quick`; focused tests are
`uv run pytest tests/test_machine_deploy.py`.

Implementation entry points:

- `airlock-machine/lifecycle/misc/modules/machine-release.nix`: standalone release construction.
- `lifecycle/image/profiles/usb-prepared-store.nix`: prepared USB topology adapter.
- `lifecycle/misc/tools/scripts/machine-deploy.py`: build/provenance/SSH transport.
- `airlock-machine/lifecycle/runtime/tools/machine-deploy.py`: dom0 planning and journalled apply.
- `airlock-machine/lifecycle/misc/modules/machine-deployment.nix`: image/boot integration.
- `airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py`: staged generation use.

The physical acceptance run has demonstrated an ordinary VM config change,
unchanged providers staying up, a GUI/provider restart surviving SSH loss,
reboot preserving selection, and rollback restoring the previous guest system
while preserving home data. Treat this as a physically exercised development
updater, not a production-safe updater: failure injection, power-loss recovery,
release pruning, schema migrations, and restricted production authority remain
outside the demonstrated boundary.
