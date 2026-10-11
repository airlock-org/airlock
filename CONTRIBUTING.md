# Contributing to Airlock

Airlock is experimental. Preserve Xen, qrexec, QubesDB, qubesd, libvirt, and
Qubes policy. Prefer upstream integrations and upstreamable fixes. Start with
[the docs index](docs/README.md) and the repository's `AGENTS.md`.

## Prepare a checkout

Initialize the pinned component using [repository workflow](docs/repositories.md).
Both sources must be present for the parent quick suite. On Linux, install Git,
Bash, Make, Python >=3.11, and uv. The checks also use ordinary coreutils and
OpenSSH tools. Some host capability checks skip when optional tools are absent.
The CI environment includes jq, util-linux, e2fsprogs, and ripgrep.

Install the locked Python dependencies explicitly:

```sh
uv sync --locked
make test-quick
```

The quick target uses `uv run --no-sync`: prepare dependencies again after a
lock-file change. It must not evaluate/build Nix or automatically install tools.
No Xen host, Tailscale enrollment, or Codex login is needed for the quick suite.
JUnit and JSON reports default to `.airlock-logs/` without the lab data mount.
Set `AIRLOCK_LOG_DIR` to choose another destination.

## Optional build environment

Nix builds target x86_64-linux and require flakes and nix-command. The repo's
Nix wrapper selects an isolated Airlock store. Choose a storage location with
room for large closures, images, and VM disks:

```sh
export AIRLOCK_DATA_ROOT=/path/to/airlock-data
make test-nix
make build
```

Nix checks are opt-in. Do not make them a routine pre-deployment gate. Use the
repository wrapper for evaluations and builds, and only guarded `make gc-auto`,
`make gc-dry-run`, or `make gc` for cleanup. Never garbage-collect the host store.
See [testing](docs/testing.md) for warm and cold nested VM workflows.

## Changes and review

Machine runtime changes belong in `airlock-machine`; image/lab integration
belongs here. Commit each coherent change, then update the parent submodule pin.
New tests use pytest; extend existing coverage rather than adding Bash test scripts.
Test externally observable behavior and failure handling. State which checks
ran and their limits; quick tests do not prove physical behavior or isolation.

For hardware work, read [machine deployment](docs/machine-deployment.md) first.
Change, test, and commit source before the repository-owned plan/apply workflow.
Verify the whole-machine generation after activation. Record and remove any
unavoidable diagnostic mutation; a fix left only in a live VM or `/run` is incomplete.
Full GUI support does not block the core qrexec/VM lifecycle milestone.

Keep personal machine identities, SSH keys, credentials, firmware dumps, and
raw diagnostics out of commits. Use placeholders in shared instructions and
keep useful hardware/evidence boundaries when editing historical reports.
For sensitive findings, follow [SECURITY.md](SECURITY.md).

## License

Contributions of original Airlock code are under the MIT license in `LICENSE`.
Retain existing third-party notices and identify the source and terms of copied
or derived code. See [THIRD_PARTY.md](THIRD_PARTY.md).
