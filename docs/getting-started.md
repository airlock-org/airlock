# Getting started

Start by inspecting the source and running the local checks. This path requires
no Xen boot, image build, USB provisioning, or Tailscale enrollment.

## Prepare your checkout

On Linux, install Git, Bash, Make, Python 3.11 or newer, and uv. The parent and
machine repositories must both be accessible. Hosted SSH clones need a GitHub
SSH key with access to both sources while they remain private.

```bash
git clone --recurse-submodules git@github.com:airlock-org/airlock.git
cd airlock
uv sync --locked
make test-quick
```

Dependency installation is explicit. The quick suite uses prepared dependencies
and does not evaluate or build Nix. JUnit XML and JSON reports go to the Airlock
log directory, normally `.airlock-logs/` when the example data mount is absent.

See [contributing](../CONTRIBUTING.md) for all prerequisites,
[repository workflow](repositories.md) for component changes and local-source
clones, and [testing](testing.md) for the verification layers.

## Understand the machine

Read [architecture](architecture.md), [validation status](status.md), and
[lab configuration](lab-configuration.md) before choosing a hardware profile.
The existing physical configuration is a development example for named hardware.

Nix builds require flakes and nix-command. The repository wrapper selects an
isolated Airlock store. Set an explicit storage location when the example lab
mount is absent:

```bash
export AIRLOCK_DATA_ROOT=/path/to/airlock-data
make build
```

Nix checks are a separate, opt-in step:

```bash
make test-nix
```

## Work with physical hardware

Read [USB development](usb-dev.md) before creating or provisioning an image.
Writing an image erases the explicitly selected USB device. Read the
[development access](usb-lab.md) for the configuration's privileged access.

Physical updates use the committed [whole-machine deployment
workflow](machine-deployment.md): plan, apply, verify the running generation,
and retain recovery paths. Build success and live verification are separate
results. Consult the [hardware usability policy](hardware-usability-policy.md)
before changing device ownership or connectivity.

## Work on the website

The landing page and Fumadocs application live in `website/`. Documentation is
edited in repository Markdown and selected by `website/docs.config.json`.
See the [website development guide](../website/README.md) for installation,
preview, static builds, and hosting configuration.
