# airlock-cli

Provides the `airlock` executable in usr-assistant. `airlock machine` retains the
existing command syntax for source submission, status, verification and maintenance.
The CLI delegates machine argument parsing and maintenance to the packaged
`airlock-machine-cli` Python helper; source submissions use the typed Rust client.
The internal `__nix-submit` adapter is used by the existing compatibility wrapper.
It grants no extra authority: all requests use the existing qrexec interfaces.

Build with `make airlock-nix-build`, or `./apps#airlock-cli`. Standalone builds
need the installed machine helper for `airlock machine`; NixOS integration binds
that helper by absolute package path. Only the client feature of the shared
`airlock-nix` crate is included. No service or GUI executable is installed.
