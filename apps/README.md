# Airlock applications

This directory owns Airlock applications and their backends. Rust apps are
members of the Cargo workspace here, sharing `Cargo.lock` and build output in `target/`.
Other app technologies can live alongside the Rust crates.

- [airlock-ctrl-gui](airlock-ctrl-gui/README.md) is the desktop interface for source and
  deployment review. It supports local previews and the installed controller's
  approval protocol; the GUI itself does not execute deployments.
- [airlock-cli](airlock-cli/README.md) provides the `airlock` command. It submits
  requests and reports results without owning deployment authority.
- [airlock-nix-service](airlock-nix-service/README.md) handles Nix source review,
  detached jobs, builds and artifact transfer in `sys-ctrl`.
- [airlock-nix](airlock-nix/README.md) contains their shared typed foundations
  and a read-only local inspector. Client and service features compile separately;
  existing dom0 services retain approval and activation authority.

From the repository root:

```sh
make ctrl-build
make ctrl-preview
make ctrl-check
```

`ctrl-build` explicitly fetches Cargo dependencies and builds the app.
`ctrl-preview` uses the guarded Nix wrapper and the standalone `apps/flake.nix`
to build/run just the app, supplying its native GUI dependencies. The app flake
explicitly disables submodule fetching and does not load machine inputs.
Its Git source includes tracked files, excluding Cargo build artifacts.
`apps/flake.lock` pins its own nixpkgs input, initially matching the machine's
existing pin. On other Linux environments with those libraries
available, use `cargo run --locked --manifest-path apps/Cargo.toml -p airlock-ctrl-gui`.
Neither command installs or deploys machine configuration.

Build and check the Nix backend separately, without GUI dependencies:

```sh
make airlock-nix-build
make airlock-nix-check
apps/target/release/airlock-nix inspect-job apps/airlock-nix/examples/job.json
```

The standalone flake also exports `./apps#airlock-nix` as a release package.
Cargo/Nix builds are explicit development actions, separate from `test-quick`.

Supply `CTRL_PREVIEW_ARGS` to review real source or capture a preview, for example
`make ctrl-preview CTRL_PREVIEW_ARGS='--demo --screenshot /tmp/airlock-ctrl-gui.png'`.

The quick pytest suite never builds apps or installs Rust tools. Tests requiring
the app run when `apps/target/debug/airlock-ctrl-gui` exists, or when
`AIRLOCK_CTRL_BIN` explicitly points to a built binary. Otherwise they skip.

`airlock-machine` remains independently deployable. It consumes pinned app
source or packages for integrated applications, and continues to own app
installation, VM settings, qrexec policy and deployment authority. See
[repository boundaries](../docs/repositories.md).
