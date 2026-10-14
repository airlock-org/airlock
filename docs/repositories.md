# Repository boundary and workflow

`airlock` is the main monorepo for applications, shared development work,
USB image assembly, prepared-store seeding, provisioning, lab transport,
documentation, and integration tests. User-facing applications live under
`apps/`; its Cargo workspace groups the Rust applications.

`airlock-machine` remains a separate pinned Git repository for deployable
whole-machine Nix configuration, dom0 and Qubes guests, bootstrap, machine
runtime helpers, its lock file, and component tests. Its independent clean
checkout, committed revision, and lock hash support deployment provenance and
consistency checks. Keep one editable source per application, runtime helper,
and component test.

Machine configuration owns which applications are installed and what authority
they receive. When an app is integrated, the component must consume pinned app
source or packages; it must not depend on an adjacent `../apps` checkout.
Local app development and previews do not require a machine build or deployment.

## Clone

The component URL in `.gitmodules` is
`git@github.com:airlock-org/machine.git`. Both repositories and the
pinned component commit must be accessible before a remote recursive clone can
work. The existing GitHub repository is named `machine`; its local submodule
directory remains `airlock-machine`. Updating this file does not change visibility.

```sh
git clone --recurse-submodules git@github.com:airlock-org/airlock.git
```

For an existing checkout after changing the URL:

```sh
git submodule sync -- airlock-machine
git submodule update --init airlock-machine
```

## Local-source clone

Before hosting is available, clone from local committed repositories. Override
the URL in the clone's Git configuration, leaving tracked metadata portable:

```sh
git clone --no-local /path/to/airlock /path/to/new-airlock
cd /path/to/new-airlock
git config submodule.airlock-machine.url /path/to/airlock-machine
git -c protocol.file.allow=always submodule update --init airlock-machine
uv sync --locked
make test-quick
```

`/path/to/airlock-machine` must contain the exact pinned commit. It can be the
initialized component checkout from the original parent. File transport is
allowed only for this explicit local initialization command.

## Change and pin the component

Work inside `airlock-machine/` on a named branch rather than detached HEAD:

```sh
git -C airlock-machine switch -c my-change
# edit and run the component or parent quick suite
git -C airlock-machine add <changed-files>
git -C airlock-machine commit -m 'Describe the machine change'
git add airlock-machine
git commit -m 'Pin machine change'
```

When remote hosting is in use, make the component commit accessible there before
sharing the parent pointer. Local builds may need a local submodule URL override
if the committed revision has not been uploaded; never claim that a local clone
proves remote availability. If editing a separate component checkout, fetch its
commit into `airlock-machine/` and check it out before committing the parent pin.

## Stores and generated files

Use each repository's guarded `lifecycle/misc/tools/scripts/airlock-nix.sh` for
Nix commands. Set `AIRLOCK_DATA_ROOT` to your development storage; the example
mount is `/run/media/user/data/airlock`. Keep generated stores, VM disks, logs,
firmware captures, credentials, and deployment receipts out of Git.
Use only the guarded Airlock GC targets. See [contributor setup](../CONTRIBUTING.md)
and [lab configuration](lab-configuration.md).
