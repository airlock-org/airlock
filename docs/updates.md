# Updates and rollback

AirlockOS updates the machine from committed Nix configuration. A release brings
together the host system, guest systems, and domain configuration. You can
inspect its effect before applying it and retain earlier releases for rollback.

## Prepare and review

From the clean, committed machine checkout in `usr-assistant`, run:

```sh
airlock-usr-assistant-machine-deploy plan
```

This builds and prepares the candidate release and shows the changes, affected
VMs, and required restarts. Planning does not activate the release.

## Apply a release

Save work in affected VMs, then run:

```sh
airlock-usr-assistant-machine-deploy apply
```

Guest updates restart affected VMs. Updating the assistant or networking can
interrupt the connection used to submit the update; the deployment continues
in the background. Reconnect and check its status:

```sh
airlock-usr-assistant-machine-deploy status
```

Host changes take effect after a reboot. If the status is `reboot-required`,
reboot when ready, then check the running system:

```sh
airlock-usr-assistant-machine-deploy verify
```

A `complete` result means the host and guests match the selected release.

## Return to an earlier release

Inspect the available rollback choice before requesting it:

```sh
airlock-usr-assistant-machine-deploy status
airlock-usr-assistant-machine-deploy rollback
```

Rollback restores system versions and configuration. It does not rewind your
personal files or application data, so backups still matter.

The boot menu also offers retained host generations and a recovery entry. An
older host selected manually from the menu boots once; the newest normal
generation remains the default for the following boot.

## If an update is interrupted

Check `status` before starting another update. The deployment records its
progress and supports resuming interrupted work:

```sh
airlock-usr-assistant-machine-deploy resume
```

Ask the assistant to inspect the reported state if the next step is unclear.
If the desktop or assistant is unavailable, use the machine's local console
and recovery entry.
