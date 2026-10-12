# Architecture

Airlock combines the Qubes architecture with declarative NixOS host and guest
systems. Nix describes system configuration and build inputs; Qubes mechanisms
retain VM lifecycle, inter-VM communication, device ownership, and policy.

## Qubes mechanisms

Qubes uses Xen as its virtualization foundation. Airlock runs NixOS in dom0
and retains the existing Qubes mechanisms:

- **qrexec** provides Qubes' inter-VM remote procedure calls (RPC).
- **QubesDB** publishes runtime settings for guests.
- **qubesd and libvirt** provide VM administration and lifecycle machinery.
- **Qubes policy** defines rules for inter-VM RPC requests.

The proof of concept accepts existing Qubes runtime state such as `qubes.xml`.
Nix remains the source of truth for declared host/guest systems wherever
practical. Runtime state and generated release records have distinct ownership.

## Shared global Nix store

Airlock builds guest systems as NixOS closures: the system and all of its
dependencies. A shared global Nix store reuses package objects across VM closures.
Each VM receives a read-only view of its declared system closure, while its
private data remains separate and writable.

Here, global means machine-wide. Dom0 manages the shared storage pool and
provides per-VM closure views; guests do not receive unrestricted access to the
entire store. Writable build stores remain separate from these read-only system
views.

## VM topology

The [computer assistant](usr-assistant.md) lives in `usr-assistant`. It provides
the user's conversation and tool workspace; sys-ctrl and dom0 retain their
existing validation, deployment, and Qubes control responsibilities.

The current configuration starts sys-gui, usr-assistant, sys-ctrl, sys-net, sys-firewall,
and usr-home. All six guests run on the same NixOS dom0 / Xen host. These views
describe the development configuration; optional experiments and
on-demand configuration VMs are omitted.

### Network routing

![Network topology: usr-assistant, sys-ctrl and usr-home use sys-firewall as their NetVM; sys-firewall uses sys-net, which owns the physical NICs.](../website/public/diagrams/network-topology.svg)

[Open the full-size network diagram](../website/public/diagrams/network-topology.svg).

Blue arrows follow the upstream network path. Usr-assistant, sys-ctrl, and usr-home
use sys-firewall as their NetVM; sys-firewall uses sys-net. Sys-net owns the
physical NICs. Dom0 has no default network route, and sys-gui has no NetVM.

### GUI and input

![GUI topology: application windows from usr-assistant and usr-home reach GNOME in sys-gui through Qubes GUI transport. sys-gui owns the display GPU; dom0 forwards keyboard and pointer events through the Qubes input proxy.](../website/public/diagrams/gui-topology.svg)

[Open the full-size GUI diagram](../website/public/diagrams/gui-topology.svg).

Orange arrows show window display through the Qubes GUI transport and the
physical display driven by sys-gui's assigned GPU. Application processes and
data stay in usr-assistant and usr-home. Sys-gui runs GNOME without a NetVM.

The dashed path shows keyboard and pointer events forwarded from dom0 through
the Qubes input proxy. It does not move input-device ownership to application
VMs. See [GNOME integration](sys-gui-gnome.md) and
[physical input](sys-gui-input.md) for implementation and acceptance scope.

### Development access

The development configuration explicitly permits a `usr-assistant` → dom0 shell over qrexec.
This privileged RPC is separate from network routing and GUI transport;
other guests do not inherit it. Read the [development access guide](usb-lab.md) for remote
access and the development boundary.

## Declaration and activation

```text
Nix host and guest declarations
             |
             v
     whole-machine release
             |
             v
     NixOS dom0 / Xen
             |
     Qubes VM lifecycle
             |
     declared NixOS guests
```

This flow shows how Nix declarations become a running machine. The topology
views above describe the resulting domains and their separate responsibilities.

## Whole-machine releases

The committed development source is submitted through usr-assistant to sys-ctrl.
The controller validates and builds candidates, then requests release
preparation and admission. Dom0 owns the shared guest pool, generation
validation, Qubes lifecycle, activation, and recovery records.

Typed status and recovery operations remain available through the supported
deployment client. Selected generations, running generations, pending reboots,
and verified completion are tracked separately. See
[machine deployment](machine-deployment.md),
[pool-backed build storage](usr-assistant-build-store.md), and
[generation retention](machine-generation-retention.md).

## Source ownership

`airlock-machine/` is a pinned Git submodule with its own flake and tests. It
owns the deployable machine and runtime implementation. The parent repository
owns image assembly, prepared-store seeding, provisioning, development-host
orchestration, integration tests, documentation, and the website.

Keep implementation changes with their owner and update the parent component
pin after a coherent machine change. See [repository workflow](repositories.md).

## Verification boundaries

Quick tests, Nix builds, nested Xen trials, and physical trials answer different
questions. A build proves an artifact can be produced; physical acceptance
checks the resulting running machine within a named configuration and scope.
See [testing](testing.md) and [validation status](status.md).

The project remains experimental. Preserving Qubes components does not itself
establish the full Qubes OS security model or broad hardware compatibility.
