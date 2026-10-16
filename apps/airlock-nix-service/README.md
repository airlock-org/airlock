# airlock-nix-service

The sys-ctrl backend for source review, detached Nix build jobs and deployment
orchestration. Commands are `serve`, `work`, `observe`, and `review-status-serve`.
It is invoked through qrexec and systemd workers rather than a persistent daemon.
Dom0 retains protected approval, admission and activation authority.

Build with `make airlock-nix-build`, or `./apps#airlock-nix-service`. Only the
service feature of the shared [airlock-nix](../airlock-nix/README.md) crate is
included. The CLI and [review GUI](../airlock-ctrl-gui/README.md) are separate
packages. Existing Python implementations and wire schemas are preserved.
