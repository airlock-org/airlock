# Migrate sys-agent to usr-assistant

The committed configuration names the computer-assistant VM `usr-assistant`.
Existing machines with `sys-agent` need an explicit whole-machine transition;
ordinary releases do not silently replace that VM or copy its data.

The transition uses the existing journalled `privateVolumeCopies` and retirement
workflow. Qubes copies the independent private volume while both VMs are
stopped, before the replacement starts. The old VM is retained in recoverable
trash. A completed receipt prevents a retry from overwriting new assistant data.

## Prepare and review

Keep a physical recovery console available. Confirm the old VM's current SSH
or Tailscale address, the committed component pin, and a clean source checkout.
The new name must not already belong to another VM: preflight refuses to
overwrite an existing destination. The source and destination must have
independent persistent private volumes, and the target cannot be smaller.

If the installed machine checkout is clean, submit the new committed source
using the old installed import command before the private-volume copy:

```sh
AIRLOCK_DEPLOY_SOURCE_COMMAND=airlock-sys-agent-machine-source \
  make usr-assistant-machine-source DEPLOY_TARGET='<confirmed-current-SSH-target>'
```

Do not discard local branches or uncommitted work to make this command pass.
Resolve that work first; the importer refuses a dirty existing checkout.

### Update an older controller first

The incremental migration uses a distinct request schema so an older controller
rejects it instead of silently building an ordinary release. Bootstrap the
updated sys-ctrl guest through the existing installed sys-agent client:

```sh
make machine-plan DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_CONTROLLER_UPGRADE=1
make machine-apply DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_CONTROLLER_UPGRADE=1
```

Review the controller guest plan before applying it. This step updates sys-ctrl
through the existing journalled guest deployment workflow; it does not rename
the assistant. Confirm that sys-ctrl's selected and running generations match
before proceeding. The old development transport can inspect the outcome while
the installed sys-agent client still uses the previous name:

```sh
make machine-status DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_LEGACY_DOM0=1
```

The updated controller accepts the old sys-agent caller only for the explicit
rename protocol. Ordinary source submissions require usr-assistant. Existing
host Qubes policy still controls whether either caller can reach the service;
this compatibility path adds no new policy grant.

### Review the incremental migration

Build and review the transition through the controller's shared-pool route:

```sh
make machine-plan DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_ASSISTANT_RENAME=1
```

This selects `airlock-machine-release-usb-lab-assistant-rename`, whose machine
profile remains `usb-lab`. Its only migration intent is:

```nix
retireDomains = [ "sys-agent" ];
privateVolumeCopies.usr-assistant = "sys-agent";
```

The parent submits the migration frontend from the same clean, committed source
checkout being imported. Sys-ctrl builds the named migration release and uses
the existing candidate-bound pool admission and prepared-plan checks. Package
objects already in the pool are reused. Ordinary and migration build/plan
records have separate paths, so apply cannot reuse the other release's plan.

`DEPLOY_LEGACY_DOM0=1` remains an explicit alternative for machines with enough
space for its larger closure staging. It uses the existing password-protected
dom0 development shell. Space preflight does not resize partitions, collect
protected generations, or change VMs to make a release fit.

## Apply, reboot, and verify

After reviewing the concrete plan, apply with the same migration settings:

```sh
make machine-apply DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_ASSISTANT_RENAME=1
```

The EFI partition needs space independently of the root filesystem. If boot
installation fails before guests are stopped, inspect the transaction's command
stderr and EFI usage. On machines whose boot entries are all independently
sealed, `lifecycle/misc/tools/scripts/boot-cache-quarantine.py` can reclaim the
unreferenced upstream `EFI/nixos` cache. Run this committed maintenance command
in dom0 first without `--apply` to review its receipt, then with `--apply`.
It authenticates every retained entry and its embedded Xen payloads, locks out
concurrent deployment, and copies cache files to the root filesystem before
removing them from the EFI partition. It refuses unsealed entries, corrupt
artifacts, symlinks, and insufficient quarantine space. Retained generations,
recovery artifacts, and Nix store paths remain intact. Resume the existing
transaction after cleanup; do not reboot an unfinished deployment.

Stopping sys-agent disconnects its SSH, Console, and assistant processes. The
detached dom0 worker continues. Use the local console to inspect the recorded
outcome if remote access is unavailable. Reboot into the selected host so the
renamed RPC policy and running host generation agree.

Confirm the replacement's current SSH/Tailscale address, then use ordinary
commands without the transition flags:

```sh
make machine-status DEPLOY_TARGET='<confirmed-assistant-SSH-target>'
make machine-verify DEPLOY_TARGET='<confirmed-assistant-SSH-target>'
```

Verify the checkout, local work, credentials, session history, and Tailscale
state in usr-assistant. Exact managed guidance links are retargeted to
`/etc/airlock/usr-assistant/`; user-written guidance and custom links remain.

The former builder base and writable cache are preserved separately under
`/rw/airlock-builder-sys-agent-builder`. They are archived, not automatically
rebased onto the new role's approved builder base. Ordinary workspace and
authentication data stay in the copied private volume.

After whole-machine verification and functional acceptance, deploy an ordinary
release to clear the one-shot retirement/copy declarations. Keep the old VM
and rollback records until their recovery purpose has been reviewed; the
migration does not automatically purge them.

## Validation boundary

Local checks exercise the renamed source and migration contracts. This document
does not record a successful physical migration. Record the actual transaction,
running host/guest generations, persistence checks, and recovery result after a
named machine completes the transition.

Wire schema identifiers containing `sys-agent` remain stable so existing
clients and saved records stay readable. They are protocol identifiers, not
the configured VM name. Historical reports also retain the old observed names.
