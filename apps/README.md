# Airlock applications

This directory owns user-facing Airlock applications. Rust apps are members of
the Cargo workspace here, sharing `Cargo.lock` and build output in `target/`.
Other app technologies can live alongside the Rust crates.

The first app is [airlock-ctrl](airlock-ctrl/README.md), the desktop interface
for source and deployment review. Its current version runs locally and does
not connect to a deployment target.

From the repository root:

```sh
make ctrl-build
make ctrl-preview
make ctrl-check
```

`ctrl-build` explicitly fetches Cargo dependencies and builds the app.
`ctrl-preview` uses the guarded Nix wrapper to build/run just the app, supplying
its native GUI dependencies. On other Linux environments with those libraries
available, use `cargo run --locked --manifest-path apps/Cargo.toml -p airlock-ctrl`.
Neither command installs or deploys machine configuration.

The quick pytest suite never builds apps or installs Rust tools. Tests requiring
the app run when `apps/target/debug/airlock-ctrl` exists, or when
`AIRLOCK_CTRL_BIN` explicitly points to a built binary. Otherwise they skip.

`airlock-machine` remains independently deployable. It will consume pinned app
source or packages when machine integration is added, and continue to own app
installation, VM settings, qrexec policy and deployment authority. See
[repository boundaries](../docs/repositories.md).
