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

Build and review the transition through the supported development transport:

```sh
make machine-plan DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_ASSISTANT_RENAME=1 DEPLOY_LEGACY_DOM0=1
```

This selects `airlock-machine-release-usb-lab-assistant-rename`, whose machine
profile remains `usb-lab`. Its only migration intent is:

```nix
retireDomains = [ "sys-agent" ];
privateVolumeCopies.usr-assistant = "sys-agent";
```

The transport still names the existing bridge VM `sys-agent`. Its dom0 shell
requires the existing password-protected development access. No new RPC
capability or policy bypass is added by the migration option.

## Apply, reboot, and verify

After reviewing the concrete plan, apply with the same migration settings:

```sh
make machine-apply DEPLOY_TARGET='<confirmed-current-SSH-target>' \
  DEPLOY_ASSISTANT_RENAME=1 DEPLOY_LEGACY_DOM0=1
```

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
