# Overview

Airlock is an experimental Nix-native Qubes system. It runs NixOS in dom0 and
declares host and guest systems in Nix while preserving the Qubes architecture:
Xen-based virtualization, VM lifecycle, device ownership, inter-VM RPC through
qrexec, and Qubes policy.

A shared global Nix store reuses package objects across VM closures. Each guest
receives a read-only view of its system and dependencies, with separate writable
private data. The store is global to the machine and managed by dom0.

The [computer assistant](usr-assistant.md) runs in its own user VM,
`usr-assistant`. It is a workspace for inspecting, configuring, and maintaining
the computer; the current interface is Codex in Console.

The project explores how declarative systems and whole-machine releases can
work with the existing Qubes architecture. It has demonstrated Qubes VM lifecycle
and inter-VM RPC, closure-backed NixOS guests, configuration validation, and machine
deployment and rollback.

## Start here

Begin with [getting started](getting-started.md) to inspect the source and run
local checks. Read [architecture](architecture.md) for the system boundaries
and [validation status](status.md) for the demonstrated scope and open limits.

Hardware development uses the [lab configuration](lab-configuration.md),
[USB development](usb-dev.md), and [machine deployment](machine-deployment.md)
guides. The [full documentation map](README.md) also links plans, experiments,
and dated evidence. Website navigation publishes a selected set of those pages;
other links lead to repository source, which requires access while the
repositories remain private.

## Current scope

The physical example uses GNOME in sys-gui on a ThinkPad T14 Gen 3 AMD.
GPU reset/restart, GUI recovery, first-run setup acceptance, and broader hardware
support remain active work. Nested VM checks and physical acceptance are
recorded separately.

Airlock has not established Qubes OS security equivalence. The development
configuration includes privileged access through usr-assistant; read the
[development access guide](usb-lab.md) before treating that configuration as a security boundary.

## Project source

The [parent repository](https://github.com/airlock-org/airlock) owns image
assembly, development tooling, integration, documentation, and this website.
The pinned [machine component](https://github.com/airlock-org/machine) owns
deployable host/guest configuration, bootstrap, runtime code, and component tests.
See [repository workflow](repositories.md).

Original Airlock code uses the MIT license. Third-party sources keep their own
terms, including an outstanding vendored-source licensing question. See
[third-party attribution](../THIRD_PARTY.md) and
[contributor guidance](../CONTRIBUTING.md).
