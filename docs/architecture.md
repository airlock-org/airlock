# Architecture

AirlockOS combines the Qubes architecture with NixOS host and guest systems.
Nix describes configuration and build inputs. Qubes manages the virtual
machines, device ownership, inter-VM communication, and policy.

## Qubes foundations

Xen is the virtualization foundation for dom0 and all guest VMs. Dom0 runs
NixOS and administers the machine through the existing Qubes stack:

- **qrexec** provides Qubes' inter-VM remote procedure calls (RPC).
- **QubesDB** publishes runtime settings for guests.
- **qubesd and libvirt** provide VM administration and lifecycle machinery.
- **Qubes policy** defines rules for inter-VM RPC requests.

## Shared global Nix store

A NixOS system closure contains the system and all of its dependencies.
AirlockOS uses a shared global Nix store to reuse package objects across host
and guest closures. Each guest receives a read-only view of its declared system
and dependencies, with separate writable private data.

Global means machine-wide: dom0 manages the shared storage pool and provides
per-VM closure views. Guests do not receive unrestricted access to the entire
store. Writable build stores remain separate from these system views.

## VM topology

Each domain has a specific role. The desktop brings application windows together
while their processes and private data stay in separate VMs.

| Domain | Role |
| --- | --- |
| `dom0` | Administers VMs, device assignment, Qubes policy, storage, and deployment. |
| `sys-gui` | Runs GNOME and drives the display. |
| `sys-net` | Owns the physical network devices. |
| `sys-firewall` | Provides the upstream network path for application and build domains. |
| `sys-ctrl` | Builds committed machine configuration into releases. |
| `usr-home` | Runs everyday applications and holds personal files. |
| `usr-assistant` | Holds the computer assistant's tools and workspace. |

### Network routing

![Network topology: usr-assistant, sys-ctrl and usr-home use sys-firewall as their NetVM; sys-firewall uses sys-net, which owns the physical NICs.](../website/public/diagrams/network-topology.svg)

[Open the full-size network diagram](../website/public/diagrams/network-topology.svg).

Blue arrows follow the upstream network path. Usr-assistant, sys-ctrl, and
usr-home connect through sys-firewall, which uses sys-net. Sys-net owns the
physical network devices. Dom0 has no default network route, and sys-gui has
no NetVM.

### GUI and input

![GUI topology: application windows from usr-assistant and usr-home reach GNOME in sys-gui through Qubes GUI transport. sys-gui owns the display GPU; dom0 forwards keyboard and pointer events through the Qubes input proxy.](../website/public/diagrams/gui-topology.svg)

[Open the full-size GUI diagram](../website/public/diagrams/gui-topology.svg).

Orange arrows show application windows reaching GNOME through Qubes GUI
transport. Sys-gui drives the display using its assigned GPU. Application
processes and data stay in usr-assistant and usr-home.

The dashed path shows keyboard and pointer events forwarded from dom0 through
the Qubes input proxy. Input-device ownership stays separate from the
application VMs. See [Desktop and applications](desktop.md) for everyday use.

## Configuration and releases

Nix describes the host and guest systems together. Sys-ctrl builds committed
configuration into a whole-machine release. Dom0 prepares and activates the
release using the Qubes VM lifecycle and retains rollback information.

System releases and personal data have separate lifecycles. Selecting an earlier
release restores system versions and configuration; it does not rewind the
contents of a VM's private storage.

Read [Configuration](configuration.md) and [Updates and rollback](updates.md)
for the user workflow. [Security](security.md) explains the current
administrative access and domain boundaries.
