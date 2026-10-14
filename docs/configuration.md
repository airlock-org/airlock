# Configuration

AirlockOS describes the host and guest systems together in Nix. Configuration
defines the domains, their software, networking, device assignments, and Qubes
permissions.

## System configuration and personal data

A system closure contains a VM's operating system and its dependencies. Each
guest uses a read-only view of that closure from the shared global Nix store.
Installing system software means building a new closure from configuration.

Personal files and application settings live on separate writable storage.
Changing a system release does not replace that private data.

## Make a change

Work from the machine configuration checkout in `usr-assistant`. You can edit
Nix directly or ask the [assistant](usr-assistant.md) to help prepare a change.
For example, you might add an application to `usr-home` or adjust a domain's
settings.

Review and commit the configuration before submitting it. The build uses the
committed source and pinned dependencies, so the resulting release corresponds
to a specific version of your configuration.

## Review the effect on the machine

A configuration change can affect several domains. A deployment plan shows which
VMs will restart and whether the host needs a reboot. Changing networking or
permissions also changes how domains can communicate.

Follow [Updates and rollback](updates.md) to build, inspect, and apply a release.
Keep changes in configuration so later updates reproduce them.

## Where to learn more

[Architecture](architecture.md) explains the responsibilities of dom0 and the
system domains. The [machine source](https://github.com/airlock-org/machine)
contains the configuration modules and options.
