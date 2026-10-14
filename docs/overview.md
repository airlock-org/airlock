# Welcome to AirlockOS

AirlockOS is a Nix-native Qubes system. It brings together a familiar desktop,
applications in separate domains, and an assistant to help operate your computer.
Nix describes the host and guest systems; the Qubes architecture provides VM
isolation, device ownership, communication, and policy.

## A desktop across domains

GNOME provides one desktop for applications running in different virtual machines.
Your everyday applications and files live in `usr-home`. The computer assistant
has its own workspace in `usr-assistant`. Separate system domains handle the
display, networking, and system builds.

Read [Desktop and applications](desktop.md) to see how these pieces feel in use,
or [Architecture](architecture.md) to understand how they fit together.

## Ask your computer

The [computer assistant](usr-assistant.md) helps you inspect problems, understand
configuration, and carry out changes. You can ask what changed in an update,
why an application cannot connect, or how to set up a workspace.

The current interface is Codex in Console, running in its own VM.

## A declared system

Host and guest configuration live together in Nix. A shared global Nix store
reuses package objects across their system closures, while each guest receives
a read-only view of its own system and dependencies. Personal files remain on
separate writable storage.

Changes become whole-machine releases with a reviewable plan and rollback
options. See [Configuration](configuration.md) and [Updates and rollback](updates.md).

## Start exploring

Begin with [Getting started](getting-started.md). AirlockOS is under active
development; the current images and setup workflow are intended for development
use. Read [Security](security.md) before deciding what work to entrust to it.
