# Getting started

Start with the [desktop](desktop.md), [computer assistant](usr-assistant.md), and
[architecture](architecture.md) to understand how AirlockOS works.

## On a running AirlockOS system

Sign in to the GNOME desktop and open the application overview. Applications run
in their assigned domains and appear together on the desktop. Everyday apps and
personal files belong in `usr-home`; the assistant workspace is `usr-assistant`.

Open **usr-assistant: Console** to use the computer assistant. Start Codex in the
terminal and complete its authentication if needed. Try asking it to explain the
running domains or inspect the current configuration before making changes.

The [desktop guide](desktop.md) explains application and VM identity. The
[assistant guide](usr-assistant.md) covers the workspace and examples of useful tasks.

## Setting up a machine

AirlockOS currently uses a development disk image built from source. Setting up
a machine requires building and provisioning that image; a guided installation
flow is still in development. Writing an image erases the selected target disk.

Source and build instructions are available in the
[Airlock repository](https://github.com/airlock-org/airlock). The deployable system
configuration lives in the [machine repository](https://github.com/airlock-org/machine).
Repository access is required while those sources remain private.

## Your first system change

Read [Configuration](configuration.md) to understand how Nix describes the
machine. For an existing installation, [Updates and rollback](updates.md) explains
how to review a plan, apply a release, and return to an earlier system.
