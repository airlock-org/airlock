# airlock-nix-service

The sys-ctrl backend for source review, detached Nix build jobs and deployment
orchestration. Commands are `serve`, `work`, `retry`, `observe`, and `review-status-serve`.
It is invoked through qrexec and systemd workers rather than a persistent daemon.
Dom0 retains protected approval, admission and activation authority.

Build with `make airlock-nix-build`, or `./apps#airlock-nix-service`. Only the
service feature of the shared [airlock-nix](../airlock-nix/README.md) crate is
included. The CLI and [review GUI](../airlock-ctrl-gui/README.md) are separate
packages. Existing Python implementations and wire schemas are preserved.

`retry REQUEST_ID` is an operator recovery command for failed approved jobs.
It requires the unchanged clean source and a still-valid protected approval,
checks that the previous worker is stopped, and creates a new request linked by
`retry_of`. It preserves the failed record and uses the normal checked worker;
it cannot revive rejected, cancelled or stale approvals.
