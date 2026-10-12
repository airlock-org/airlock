# Airlock

Airlock is an experimental Nix-native Qubes system. It runs NixOS in dom0 and
declares host and guest systems in Nix while preserving the Qubes architecture:
Xen-based virtualization, VM lifecycle, device ownership, inter-VM RPC through
qrexec, and Qubes policy.

The project has demonstrated Qubes VM lifecycle and inter-VM RPC, closure-backed
NixOS guests, configuration validation, and whole-machine deployment and rollback. Nested
verification and physical lab acceptance are recorded separately. The current
physical example uses GNOME in sys-gui and a ThinkPad T14 Gen 3 AMD hardware
profile. GPU restart/recovery and GUI startup issues remain open. Airlock is
experimental; it has not established Qubes OS security equivalence or general
hardware support.

## Start with the local checks

You can inspect the source and run the quick suite without booting Xen, building
an image, attaching a USB disk, or enrolling in Tailscale. You need Git, Bash,
Make, Python 3.11 or newer, and uv. Linux is the development target; Nix builds
support x86_64-linux. See [contributor setup](CONTRIBUTING.md) for prerequisites
and explicit dependency installation.

Once both repositories are available at the intended hosting locations:

```sh
git clone --recurse-submodules git@github.com:airlock-org/airlock.git
cd airlock
uv sync --locked
make test-quick
```

The component URL is prepared locally; this preparation does not publish it.
While the component remote is unavailable, use the
[local-source clone procedure](docs/repositories.md#local-source-clone).
`make test-quick` runs pytest without Nix evaluations, builds, or dependency
installation. Results include JUnit XML and JSON under the Airlock log directory.

## Source ownership

- [`airlock-machine/`](airlock-machine/README.md) is a pinned Git submodule. It
  owns deployable host/guest configurations, bootstrap, runtime code, and component tests.
- [`lifecycle/image/`](lifecycle/image/README.md) assembles USB images and prepared stores.
- `lifecycle/misc/tools/scripts/` holds development-host and lab orchestration.
- `tests/` covers parent integration; it runs component tests from their canonical source.
- [`docs/`](docs/README.md) indexes current guides, experiments, plans, and dated evidence.
- [`website/`](website/README.md) builds the landing page and selected documentation
  as a static Fumadocs site, using repository Markdown as its source.

Keep machine changes in the component repository, then commit its new pointer
here. See [repository workflow](docs/repositories.md).

## Builds and hardware development

Nix is needed for builds, with flakes and nix-command enabled. Use the repository
Nix wrapper and an explicit `AIRLOCK_DATA_ROOT` when the example lab data mount
is absent. It uses an isolated store and refuses to fall back to the host store.
See [lab configuration](docs/lab-configuration.md) before using the example machine.

```sh
export AIRLOCK_DATA_ROOT=/path/to/airlock-data
make build
# Optional Nix evaluations and package checks:
make test-nix
```

Read [USB provisioning](docs/usb-dev.md) before building or writing an image.
Writing/provisioning erases the explicitly selected USB. Codex is available as
an application in sys-agent; Airlock does not copy or manage its credentials.
Tailscale enrollment is interactive after boot, with state retained in the VM's
private volume.

Physical updates follow the committed
[whole-machine deployment workflow](docs/machine-deployment.md). Build success
and live deployment verification are separate results. This repository cleanup
has not deployed or verified a new live generation.

## Contributing and licensing

See [CONTRIBUTING.md](CONTRIBUTING.md), [security reporting](SECURITY.md), and
[validation status](docs/status.md). Original Airlock code is MIT licensed.
Third-party source and derived patches retain their own terms; see
[THIRD_PARTY.md](THIRD_PARTY.md), including the outstanding vendored-source
licensing question. [Local publication preparation](docs/public-readiness.md)
records checks and remaining hosting prerequisites.
