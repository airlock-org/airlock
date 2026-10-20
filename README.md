<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/airlock-wordmark-thin-red-dark.svg">
    <img alt="AirlockOS" src="docs/assets/airlock-wordmark-thin-red-light.svg" height="64">
  </picture>
</p>

AirlockOS runs [NixOS](https://nixos.org/) in every virtual machine (VM) using the [Qubes](https://www.qubes-os.org/) architecture.
Nix defines the machine configuration. Xen runs all VMs, including dom0, the
privileged control VM.

- NixOS configuration for every VM, including dom0.
- Whole-machine updates and system generation rollback.
- Shared global Nix store with read-only per-VM closure views.
- Persistent private VM data across updates and rollback.
- Qubes VM isolation, device assignment, and inter-VM RPC policy.
- GNOME desktop displaying applications from multiple VMs.

[![VM layout: Xen runs dom0 and the system and user VMs. Dom0 manages the shared Nix store and provides read-only closure views to the other VMs.](docs/assets/system-overview.svg)](docs/assets/system-overview.svg)

[Contributor setup](CONTRIBUTING.md) covers source layout, builds, and checks.
The landing page and product documentation are maintained in the sibling
`website` project.

AirlockOS is under active development. Qubes OS security equivalence has not
been established. See [Security](SECURITY.md) for limitations and reporting.

Original code is [MIT licensed](LICENSE). Dependencies retain their own terms;
see [third-party notices](THIRD_PARTY.md).
