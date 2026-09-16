# Repository-first whole-machine deployment

## The development contract

The committed local repository is the source of truth. A running machine is a
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

- A bootstrapped `usb-lab` machine, sys-dev reachable over SSH/Tailscale, and
  access to the dom0 development password. Use the Tailscale name (which can be
  `sys-dev-2`); its Qubes name remains `sys-dev`.
- Keep a physical dom0 console available for the first trials. Restarting
  sys-dev removes the remote connection; failures may require local recovery.
- A clean, committed repository and its existing guarded Airlock Nix store.
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

`machine-build` needs no remote machine. All commands default to `usb-lab`.
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

At the boot menu, use the default Xen entry for the newest generation. A deployed
profile is a standalone generation and may be titled simply `NixOS (with Xen
Hypervisor)`; the `usb-lab` specialisation entry can belong to the original image
generation instead. Check the generation number, not just the profile name.
Entries marked `Recovery: without Xen` are not normal Qubes boot targets. The
post-install label hook strips that marker from Xen entries, including retained
older generations whose titles upstream copied from recovery entries.

The source revision and lock-file hash are recorded in the release. Local build
receipts and GC roots are under `.airlock-logs/machine-deploy/<commit>/`.
Runtime journals are in `/var/lib/airlock/machine/transactions/<id>/`, including
`status.json`, `staged.json`, and setup's JSON/command logs. Use
`journalctl -u 'airlock-machine-deploy-*'` in dom0 for worker output.

## What is built and applied

The flake output is:

```text
nixosConfigurations.usb-dev.config.specialisation.usb-lab.configuration.system.build.airlockMachineRelease
```

It contains `release.json` (host system, exact setup/coordinator binaries,
profile, commit and lock hash) and `topology.json` (all declared VMs, policies,
preferences, devices and exact VM systems/root/store sources).

The release reuses the USB profile's evaluated host and guest systems. The
image uses prepared-store descriptors to avoid first-boot package import; the
deployment topology uses ordinary closure manifests for transport/admission.
These name the **same VM system paths**, not independently maintained VM configs.
The release is a separate build output and does not build/write a disk image.
The host still references seed descriptors: guest changes can consequently
rebuild prepared-store artifacts. Eliminating that build-time cost is a future
optimization, not a reason to fork the configuration.

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

The selection survives a normal reboot. Boot setup consumes the selected
release, not the original image topology. Old images receive a repository-owned
compatibility drop-in for setup-config; new images include the coordinator and
profile marker. `/var/lib/airlock/machine/controller` points to a pinned Nix
binary, not a manually maintained live script.

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

- `lifecycle/image/profiles/usb-prepared-store.nix`: shared release construction.
- `lifecycle/misc/tools/scripts/machine-deploy.py`: build/provenance/SSH transport.
- `lifecycle/runtime/tools/machine-deploy.py`: dom0 planning and journalled apply.
- `lifecycle/misc/modules/machine-deployment.nix`: image/boot integration.
- `lifecycle/runtime/tools/scripts/nix-domain-storage.py`: staged generation use.

The next physical acceptance run must demonstrate an ordinary VM config change,
unchanged providers staying up, a GUI/provider restart surviving SSH loss,
reboot preserving selection, and rollback restoring the previous systems while
preserving home data. Until those pass, describe this as an implemented and
locally tested development workflow, not a production-proven updater.
