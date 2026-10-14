# AirlockOS

AirlockOS runs NixOS in every virtual machine (VM) using the Qubes architecture.
Nix defines the machine configuration. Xen runs all VMs, including dom0, the
privileged control VM.

[![VM layout: Xen runs dom0 and the system and user VMs. Dom0 manages the shared Nix store and provides read-only closure views to the other VMs.](docs/assets/system-overview.svg)](docs/assets/system-overview.svg)

GNOME displays application windows from separate VMs on one desktop. Personal
applications and files stay in `usr-home`; `usr-assistant` provides the system
workspace for configuration and administration.

Dom0 manages a shared global Nix store. The other VMs receive read-only views of
their system closures, with separate writable private storage. Configuration
changes are built into machine releases with deployment planning and rollback.

## Development

See [contributor setup](CONTRIBUTING.md) for prerequisites and repository access.
To clone the source and run the local checks:

```sh
git clone --recurse-submodules git@github.com:airlock-org/airlock.git
cd airlock
uv sync --locked
make test-quick
```

The quick checks do not require Xen or Nix builds.

## Documentation

- [Repository workflow](docs/repositories.md): source ownership and the pinned machine configuration repository.
- [Image building and provisioning](docs/usb-dev.md).
- [Deployment and rollback](docs/machine-deployment.md).
- [Development documentation](docs/README.md).

AirlockOS is under active development. Qubes OS security equivalence has not
been established. See [Security](SECURITY.md) for limitations and reporting.

Original code is [MIT licensed](LICENSE). Dependencies retain their own terms;
see [third-party notices](THIRD_PARTY.md).
