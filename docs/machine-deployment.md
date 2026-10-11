# Repository-first whole-machine deployment

This is the operating guide for committed whole-machine updates. Start with
[repository setup](repositories.md), [lab configuration](lab-configuration.md),
and [current validation status](status.md). Dated lab results and migration-only
instructions are in [historical deployment notes](history/machine-deployment-lab-notes-2026-10-09.md).
A build, a selected release, and a verified running generation are distinct results.

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

For `usb-lab`, host-side plan/apply commands push committed source to sys-agent
and use the typed controller route. The explicitly selected development-host
transport (`DEPLOY_LEGACY_DOM0=1`) uses sys-agent's existing **development-only
full dom0 administrative access**, with a password-protected sudo session. It adds no
dom0 SSH listener or new production qrexec capability. This is not yet the
restricted config-authority/approval workflow described in `config-system.md`.

## Prerequisites and current validation boundary

- A bootstrapped machine started from the `Airlock` boot-menu entry, with
  sys-agent reachable over SSH/Tailscale and access to the dom0 development
  password. The internal deployment profile remains named `usb-lab`; it is not
  a boot-menu label. Discover the development VM's current SSH/Tailscale
  hostname or address with `tailscale status` and confirm it with `ssh`.
  Enrollment can change that hostname; do not infer it from the Qubes name,
  which remains `sys-agent`. Set `DEPLOY_TARGET` to the confirmed SSH target.
- Keep a physical dom0 console available for the first trials. Restarting
  sys-agent removes the remote connection; failures may require local recovery.
- Clean, committed `airlock-machine` and parent `airlock` repositories, an initialized
  `airlock-machine/` submodule at the committed pointer, and the guarded Airlock Nix store
  under `/run/media/user/data/airlock`.
- Free space in sys-ctrl's writable build store and dom0's shared pool and
  ordinary store. Normal updates transfer missing guest objects to the pool
  and missing host dependencies to the ordinary store. Space checks never
  resize partitions, run GC or delete retained generations to make an update fit.

Physical lab apply, reboot, rollback, and persistence trials are recorded in
[completed acceptance](completed/) and [historical deployment notes](history/machine-deployment-lab-notes-2026-10-09.md).
Their success is hardware- and scope-specific. Check [current status](status.md)
for open GPU/GUI failures and pending host verification.

## Normal loop

Commit component changes and update the parent pin as described in
[repository workflow](repositories.md). Remote builds need access to the pinned
component commit. Before hosting is available, use a local-source checkout and
an explicit local URL override; the public URL alone cannot fetch unpublished
commits. Do not upload source as part of a local-only preparation task.

```bash
# Make the change in this repository, then:
make test-quick
git add <the relevant files>
git commit -m 'Describe the change'

make machine-build
# Set this once to the current SSH target discovered above:
export DEPLOY_TARGET='<current-development-VM-SSH-target>'
ssh "$DEPLOY_TARGET" hostname
make machine-plan
make machine-apply
make machine-status
make machine-verify
```

The same whole-machine release can be built and submitted directly from the
clean committed checkout included in sys-agent, without a parent `airlock`
checkout or development-host build artifacts:

```bash
airlock-sys-agent-machine-deploy build
airlock-sys-agent-machine-deploy plan
airlock-sys-agent-machine-deploy apply
airlock-sys-agent-machine-deploy status
airlock-sys-agent-machine-deploy verify
```

The sys-agent client sends a versioned source bundle to `sys-ctrl`. The
controller verifies the Git revision and lock hash and builds from the locked
source, fetching missing inputs or substitutes through sys-firewall. The live-VM
addition branch first imports missing host/transport dependencies through the
installed `airlock.Ctrl.MachinePrepare` interface into dom0's private host-store
upper. Dom0 validates the candidate and issues an inactive admission token.
The controller sends guest manifests through candidate-bound pool admission,
which requests only missing package objects and immutable root artifacts.
The TODO 07 implementation then publishes
verified host objects into the same pool and prepares an approved sys-ctrl base
for its next stopped setup. Dom0 binds independently checked admission receipts
to the release and invokes its journalled coordinator. `plan` never activates;
`apply` starts a detached worker so restarting sys-ctrl or sys-agent cannot cancel
activation. Normal status/verify/resume/rollback and retention commands use a
typed dom0 interface, including when sys-ctrl is unavailable. The explicitly
selected `--legacy-dom0` development route remains separate.

The controller records source archives and requests in
`~/.local/share/airlock/controller/candidates/<revision>/`; its observation timer
records actual dom0 outcomes. Submission is not success. The final report
includes package and sparse-root payload counts, root logical/allocated bytes,
host NAR/compressed bytes, build time and transfer/admission/validation timings.
The unique root inventory avoids counting a shared immutable base once per VM.
Unchanged roots seeded by image creation are verified and reused without another
Nix-store import. Root artifacts live in dom0’s pool and are pinned by guest
generations; Qubes imports them into separate per-VM writable root disks.
Private disks remain per VM. Host kernels stay available through the host closure.
Stopped sys-ctrl does not own the serving guest pool, and dom0 retains the host
and controller boot artifacts independently.

For a committed single-guest update, the controller route is:

```bash
# Run inside sys-agent from its clean airlock-machine checkout.
airlock-sys-agent-machine-deploy guest-plan --target usr-home
airlock-sys-agent-machine-deploy guest-apply --target usr-home
```

The client sends a versioned source archive over qrexec. `sys-ctrl` verifies
the revision and lock hash, imports the checkout, builds from locked inputs
in its pool-backed private store, and sends the resulting guest closure
to dom0's shared pool. Dom0 accepts admission only from `sys-ctrl`, validates
the inactive generation receipt, and runs its existing journalled guest
plan/apply coordinator. The controller cannot submit an arbitrary release path
or shell command to this interface. Guest activation also uses a detached dom0 worker, so updating sys-agent or
sys-ctrl can disconnect the submitting client without cancelling activation.
Inspect dom0's machine deployment status to establish the outcome. The newest whole-machine integration has not been tested or built in this
pass; the sys-ctrl TODO remains open for automated and physical validation.

For a guest closure already built and admitted by sys-agent, the development
coordinator can derive a one-guest candidate without building or transferring a
second whole-machine release:

```bash
make machine-guest-plan ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
make machine-guest-apply ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
```

The receipt path is inside sys-agent. Dom0 validates it against the retained pool
generation and keeps the resulting candidate and exact generation binding in
the machine journal. Apply, verify, reboot handling, resume and rollback then
use the same coordinator flow as an ordinary whole-machine release.

For a coordinator fix on an already flashed image, add `UPDATE_COORDINATOR=1`
to `machine-guest-plan` and `machine-guest-apply`. This builds the standalone
coordinator from the committed local submodule and transfers only missing Nix
paths through the existing development transport. The candidate records that
coordinator, and apply roots it against GC. This avoids rebuilding/flashing the
USB for a coordinator-only fix; it does not update other host components.

`machine-build` needs no remote machine. All commands default to the internal
`usb-lab` deployment profile; this is not a boot-menu entry.
`DEPLOY_PROFILE=usb-gui-lab` builds that complete profile instead, but its usual
topology has no sys-agent bridge; use the dom0 coordinator directly if artifacts
are provisioned by another authorized transport. A deployment cannot silently
change the active boot profile.

`plan` builds/imports missing immutable artifacts and displays the complete
change/restart plan; it does **not** activate a generation or restart VMs.
`apply` submits the requested release and launches a detached systemd worker in dom0.
Closing the terminal or losing SSH does not terminate that worker. Reconnect
and inspect status; do not interpret a disconnected SSH session as failure.

Guest changes are applied live by restarting affected VMs. Dom0 uses
`switch-to-configuration boot`, not `switch`: physical GPU, Xen, initrd, host
services and host policy changes take effect on reboot. `reboot-required` means
the guests were verified but the selected host generation is not running yet.
After reboot, run `machine-verify` again. Only `complete` means both host and
guest generation checks passed. Window behavior still needs its own functional
test; generation equality alone is not a GUI acceptance test.

The boot menu lists normal Airlock generations from newest installed to oldest,
then recovery entries. New images and releases use titles such as
`Airlock (2026-09-30_14:30-a1b2c3d)` and
`Airlock (recovery, 2026-09-30_14:30-a1b2c3d)`, using the source commit's UTC
timestamp and short SHA. Entries created before source metadata was embedded
are explicitly labelled `legacy generation N` during verified migration.

The newest normal generation is always the persistent default. Manually choosing
an older generation boots it once; the following boot defaults to the newest
again. Whole-machine rollback and recovery selection do not establish an older
persistent default: use the physical menu to boot that older host. Boot artifacts
and writable host state remain specific to the manually chosen generation.

Recovery does not start VMs, hand off the GPU or bring up networking automatically.
The repository-owned post-install normalizer applies presentation after image
builds and deployments. Recovery is pinned as a separate Nix system profile, so
it remains available when the original image generation falls outside history.
Retained entry migrations verify existing checksums before changing presentation,
with a journalled checksum transition; Xen, kernel and initrd checksums stay fixed.

Normal USB boots are guarded by a persistent boot-attempt supervisor. A VM is
not ready merely because Xen created it: network providers, sys-gui, and every
declared autostart guest must answer qrexec and report the exact
`/run/current-system` selected by the topology. Startup stages have hard
timeouts and failures capture the journal, Xen state, Qubes state, memory, and
guest console logs under `/var/log/airlock/boot-attempts/<boot-id>/`. The
machine-readable attempt state is under
`/var/lib/airlock/boot-attempts/<boot-id>/`. A detected failure replaces the
bootstrap display with an actionable tty1 diagnostic login instead of leaving
a stale completion message onscreen.

If a hard reset interrupts a supervised boot, the next boot preserves the old
logs before Xen or Qubes can reopen them. It then deliberately enters a safe
local recovery boot: PCI-backed VMs, GPU handoff, networking, and guest
autostart remain disabled, and tty1 explains where the evidence lives. Inspect
that evidence and reboot once more for a normal retry. This recovery boot is
distinct from the boot-menu `Airlock (recovery)` generation.

The 2026-09-22 physical trial exposed three integration defects before any guest
started: the readiness package omitted its supervisor dependency, concurrent
failure handlers collided on the state file, and logind's `autovt@tty1` login
competed with the diagnostic console. The local fixes declare the dependency,
serialize state/event/marker updates with unique temporary files, and mask both
standard tty1 login names during supervised boots. Diagnostic login is also
requested if capture fails. Regression checks run the built readiness package
with an empty search path, launch concurrent failure handlers, and evaluate both
tty1 masks. A fresh physical boot and failure-console trial remain required.

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
3. Pins host artifacts and checks the controller-admitted guest generations without selecting them.
4. Records the desired release before stopping affected VMs, dependants first.
5. Runs the release's exact setup-config binary over the complete topology,
   reusing its staged admissions; private/home volumes are not replaced.
6. Selects the host boot generation, starts VMs in dependency order, and checks
   selected systems and running guests' `/run/current-system` over qrexec.

Unchanged VMs remain running. A first deployment can affect every VM because
the image's prepared descriptor sources are replaced by closure-manifest sources.
Already running VMs and declared autostart VMs are brought back, with their
dependencies. Shutdown timeouts fail the transaction; the tool does not kill VMs.

### Live VM addition implementation

[TODO 12](completed/12-live-vm-additions.md) implements a released baseline that can
prepare and activate a new ordinary VM without a preparatory host deployment
or reboot. Its host generation is selected for the next boot while Qubes creates
the VM and its persistent private disk on the running host. Physical acceptance
is pending; the existing live machine has not received this branch.

Candidate admission is restricted to sys-ctrl and binds exact guest identities
to the imported candidate. It does not activate anything. The normal whole-machine
apply still binds independently checked pool receipts, journals activation and
uses the existing selected-release boot and recoverable rollback paths. An
admission token becomes stale when boot, selection or deployment state changes;
prepare the release again rather than bypassing the check.

Graphical readiness uses the Qubes GUI-VM tag. The new sys-gui interface
`airlock-sync-qube-appmenus VM` explicitly starts a guest, waits for its session
and uses upstream Qubes tools to install its declared menus live. `menu-items`
contains the selected desktop filenames; generated launchers use fixed StartApp
RPCs and the readiness barrier. Routine menu additions need no sys-gui restart.
User review/approval and the smaller declaration interface remain separate work.

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
inactive generation. Reused objects are neither packaged nor uploaded; sys-agent
still reads their local contents to compute the inventory hashes. The response
records `objects_transferred`, `payload_bytes_transferred`, and reuse counts.
Admission, GC, compaction, and recovery share a lock to protect negotiated reuse.
Both sys-agent and dom0 need the updated image; the original full-artifact service
remains available, but the new sender does not silently fall back to a full upload.
Fresh-image physical validation passed on 2026-09-18: a committed usr-home
change built in sys-agent admitted 4 new objects (434,176 bytes) and reused 978.
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

## Independent recovery acceptance

Select the labelled Airlock recovery entry manually from the boot menu and log in as `user`
with the provisioned local password. This entry deliberately has no networking,
Qubes guests or main pool-store assembly, so remote Tailscale access is unavailable.
For the initial healthy-pool recovery smoke test, inspect:

```bash
cat /etc/airlock/pool-recovery
findmnt /nix/store
```

The store should be a read-only bind of `/var/lib/airlock/recovery-store`, not
the normal host overlay. Then reselect the currently retained normal host:

```bash
sudo airlock-host-store recover "$(readlink -f /nix/var/nix/profiles/system)"
```

This validates the retained pool binding and installed EFI artifacts before
selecting the entry; it does not reboot. Capture the command result at the
console, then manually boot the newest normal Airlock entry and verify the machine again.
This smoke test alone does not establish recovery from a missing/corrupt view;
that fault-injection trial remains separate. Do not use the recovery environment
to run normal whole-machine apply or garbage-collect its shared Nix database.

## Failures, recovery, and limits

This is a journalled convergent deployment, **not an atomic multi-VM database
transaction**. A failure can leave a mixed running state. The journal records
the failed phase and selected/previous releases; it never reports that as success.
Boot reconciliation refuses an interrupted transaction instead of silently
adopting the image's original topology. Don't run legacy apply tools concurrently.

```bash
make machine-status
make machine-resume
# Or explicitly select the previous whole-machine release:
make machine-rollback
```

If sys-agent is down, use the physical dom0 console:

```bash
sudo /var/lib/airlock/machine/controller status
sudo /var/lib/airlock/machine/controller resume
# Or:
sudo /var/lib/airlock/machine/controller rollback
```

Do this from the matching lab profile. Text recovery can inspect files and boot
entries, but is deliberately not an alternative configuration apply target.
Rollback reselects systems, not user-data snapshots; application data migrations
are not undone. A VM introduced by the rolled-back release is stopped,
quarantined in recoverable trash with its private data, and reported as
`retained_undeclared`.

Automatic deployment currently supports Nix-closure/shared-closure VMs and
recoverable domain retirement. It still rejects class/template/storage-layout
changes, arbitrary physical device reassignment, and deletion of
preference/feature/service keys without explicit reset values. A rollback that
removes newly introduced settings can likewise require explicit reset values.
Unsupported changes fail preflight rather than being silently ignored.

The USB lab `sys-agent`/`sys-net` automatic network PCI handoff is a scoped
exception: the release coordinator reports `pci_handoff`, checks current
network-controller assignments before stopping VMs, and releases the former
owner after the dependent VMs stop. Setup assigns the new owner. The same
sequence supports rollback; confirm it on hardware before relying on remote
access through sys-agent.

To retire a domain, remove it from the topology and list its name in
`airlock.machineDeployment.retireDomains` in the same committed release. The
list must exactly match domains removed relative to the selected release; clear
it again in later releases. Preflight rejects removal while surviving domains,
machine defaults, or topology-managed qrexec rules still refer to the domain.
Later releases are held to the same rule while the name remains in trash;
reintroducing the VM through a validated restore is the exception.
Apply stops the domain, disables
autostart, applies the `airlock-retired` tag, pins its selected pool generation,
and records its UUID, private storage identity, source release, and expiry under
`/var/lib/airlock/machine/trash/`. Early qrexec policy denies ordinary calls
from or to tagged domains. The domain remains registered under its original name so Qubes
and the shared pool retain their existing identity bindings.

Whole-machine rollback, or a later committed release that reintroduces the same
domain and private storage identity, restores it and removes the retirement tag.
Boot enforces the stopped, tagged state and refuses to continue if a trashed VM
was found running. Inspect trash from a dom0 recovery shell with:

```bash
sudo /var/lib/airlock/machine/controller trash-list
```

Trash is retained for at least
`airlock.machineDeployment.trashRetentionDays` (30 days by default). Expiry only
makes it eligible for permanent deletion. Purge also requires every retained
release and bootable host profile referencing that VM to have been safely
pruned. Use the deployed coordinator to inspect all recorded releases, their
roots, removable Nix boot generations, and protection reasons:

```bash
airlock-sys-agent-machine-deploy release-list
airlock-sys-agent-machine-deploy release-prune-plan --release /nix/store/RELEASE
airlock-sys-agent-machine-deploy release-retain --release /nix/store/RELEASE --retain true
airlock-sys-agent-machine-deploy release-prune --release /nix/store/RELEASE \
  --confirm /nix/store/RELEASE
```

Replace `RELEASE` with the exact release path shown by `release-list`.
The selected release, its immediate rollback choice, the running/booted host,
recovery profiles, unfinished transactions, and unexpired trash requirements
are protected. Additional rollback choices can be protected with
`release-retain`; `--retain false` removes only that explicit protection.
To move the immediate rollback choice beyond a retirement, deploy another
committed release that still omits the VM and clears `retireDomains`. A
confirmation cannot override any protection. Legacy failed transaction records
without a recorded recovery remain protected and require investigation.
Boot into the selected retirement generation before testing purge: a running
host whose embedded topology still references the VM also blocks deletion.
Recovery images remain protected; use a disposable VM added after image boot
for the first purge trial so the image's recovery topology does not declare it.

To empty a specific trash record before its deadline, inspect the effective
prune plan and prune each release that references the VM:

```bash
airlock-sys-agent-machine-deploy release-prune-plan --release /nix/store/RELEASE \
  --allow-unexpired-trash TRANSACTION-VM
airlock-sys-agent-machine-deploy release-prune --release /nix/store/RELEASE \
  --confirm /nix/store/RELEASE --allow-unexpired-trash TRANSACTION-VM
```

The override waives only `unexpired trash TRANSACTION-VM` for that release.
It cannot bypass a selected or running host, rollback/recovery reference,
unfinished transaction or purge, or explicit retain. Each additional unexpired
record needs its own `--allow-unexpired-trash` argument. Pruning the source
release gives up restoration from that release; its exact roots and boot
generations are still journalled before removal. Repeat an interrupted prune
with the same override.

Pruning journals the exact roots and profile generation numbers before deleting
them, refreshes the boot menu, and preserves deployment audit files. It does
not run Nix or pool GC. Repeat the same command after an interruption; deployment
and purge refuse to proceed while pruning is unfinished. Inspect
`release-prune-plan` for the durable status and the reported systemd unit journal
for errors. Pruned releases cannot be applied again through the coordinator;
reintroducing a VM requires a new committed release. Once references have been
pruned, old software may be collected and that rollback choice is lost.

After expiry and pruning, purge one exact trash record:

```bash
sudo /var/lib/airlock/machine/controller trash-purge TRANSACTION-VM \
  --confirm TRANSACTION-VM
```

The sys-agent equivalent is `airlock-sys-agent-machine-deploy trash-purge
--identity TRANSACTION-VM --confirm TRANSACTION-VM`. Prune and purge use detached
dom0 workers and survive SSH disconnection; `trash-list` reports purge status.
Before expiry, add `--before-expiry` to that exact purge command. This waives
only the deadline; the retained-release, profile, VM identity, ownership, and
stopped-state checks still apply. The trash audit record notes early purging.

Purge checks the UUID, actual volume identities, sharing/source relationships,
retirement tag, stopped state, and recorded pool generation before removing
the Qubes domain. Trash created before volume snapshots were introduced must
be restored and retired again before purge. Do not edit its metadata to bypass
the check. The private volumes are permanently removed through Qubes, while the
pool generation becomes eligible for ordinary GC. Shared objects remain subject
to other generations' and builder views' retention. Durable pool retirement
receipts allow retries even if GC runs before the coordinator records completion.
The audit record remains with status `purged`; repeating a finished purge is a
no-op. No expiration or purge automatically deletes unrelated pool objects.

There is no automatic old-release pruning. Retained releases cost disk space.
Only use the guarded Airlock GC targets on the development host; never run GC
on its host `/nix/store`. Do not manually delete runtime closure-store segments.
