# Contributing to AirlockOS

Preserve Xen, qrexec, QubesDB, qubesd, libvirt, and Qubes policy. Prefer upstream
integrations. Repository rules and Rust documentation standards are in
[AGENTS.md](AGENTS.md).

## Source and checkout

- `crates/`: applications, services, and shared Rust code.
- `airlock-machine/`: pinned machine configuration and runtime integration.
- `lifecycle/`: image assembly and external development tools.
- `../website`: landing page and product documentation, with its own build.

Initialize the component before building or running the parent tests:

```sh
git submodule update --init airlock-machine
uv sync --locked
make native-integration-build
make test-quick
```

The quick suite uses locked, explicitly prepared dependencies. It does not
provision tools, evaluate Nix, or require Xen or application enrollment.
Reports use `AIRLOCK_LOG_DIR`, falling back to `.airlock-logs/` without a data
mount. New tests use Cargo and cover behavior and failure handling. Python fixtures
only adapt actual upstream Qubes interfaces.

## Builds and checks

Nix builds use the repository wrapper and an isolated store. On an external
x86_64-linux development computer, set its location:

```sh
export AIRLOCK_DATA_ROOT=/path/to/airlock-data
make test-nix
make machine-build
```

Nix checks are opt-in. Use only the guarded Airlock GC targets; never collect
the computer's `/nix/store`. [Image operations](lifecycle/image/tools/README.md)
cover provisioning and boot recovery. [Development operations](lifecycle/misc/tools/README.md)
cover deployment and nested-VM checks.

Rust checks and source documentation follow
[the repository standards](AGENTS.md#rust-documentation-and-formatting).
[Application commands](crates/README.md) cover explicit app builds and previews.
For website development, run `pnpm dev` or `pnpm build` in `../website`.

## Commits and publication

Commit machine changes in `airlock-machine`, then update and commit the parent
pin. Keep unrelated work separate. Before publishing a parent pin, publish its
machine commit on upstream `main`, then run:

```sh
make check-publish
git push --recurse-submodules=check
```

Document implementation in source and tests. Keep only essential contributor
and operating instructions as repository prose. Product explanations belong
in the website. Git history retains past investigations and validation records.

Original contributions use the [MIT license](LICENSE). Preserve upstream terms
and [third-party notices](THIRD_PARTY.md). Follow [SECURITY.md](SECURITY.md) for
sensitive findings; keep credentials, private keys, firmware dumps, and raw
machine diagnostics out of commits.
