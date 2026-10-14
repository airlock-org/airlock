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

The sibling `../website` checkout is an independent Git repository for the
landing page and public user documentation. It owns its Markdown, assets,
dependencies, tests, and static build. Development guides and reports remain
here. Shared reference images and copies of served website assets live in
`../assets`; the website builds from its own `public/` directory.

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

Normal builds such as `make usb-dev-image` use the initialized local component
at the exact committed parent pin. The Nix wrapper checks that the checkout
matches that pin and is clean, then supplies a temporary local Git source to
Nix. This works with unpublished commits, feature branches, and detached HEAD;
it does not change branches, Git configuration, or portable lock files. No
environment overrides are needed. Missing or mismatched checkouts stop with
instructions to initialize the component or commit an intentional pin update.

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

When sharing a parent pin, publish the machine commit on the component's `main`
first: Nix fetches the submodule through that branch by default. A commit present
only on another remote branch is insufficient. Verify availability before
publishing the parent:

```sh
# Publish the reviewed machine change on upstream main first, then:
make check-publish
git push --recurse-submodules=check
```

`check-publish` fetches the declared component upstream's current `main` into a
temporary repository and verifies that it contains the committed pin. It never
pushes, changes your checkout, or trusts cached remote-tracking refs. It needs
network access and the same GitHub credentials as cloning. It is separate from
local builds and `test-quick`. Git's additional recursive push check catches
unpublished submodule pins when pushing; the explicit check also verifies the
`main` branch required by Nix. If editing a separate component checkout, fetch its
commit into `airlock-machine/` and check it out before committing the parent pin.

## Stores and generated files

Use each repository's guarded `lifecycle/misc/tools/scripts/airlock-nix.sh` for
Nix commands. Set `AIRLOCK_DATA_ROOT` to your development storage; the example
mount is `/run/media/user/data/airlock`. Keep generated stores, VM disks, logs,
firmware captures, credentials, and deployment receipts out of Git.
Use only the guarded Airlock GC targets. See [contributor setup](../CONTRIBUTING.md)
and [lab configuration](lab-configuration.md).
