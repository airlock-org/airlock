# Managed Codex update, 2026-10-12

Sys-agent's Nix-managed Codex was updated from `0.151.0` to the upstream stable
`0.162.1` release. Component commit `422663a388de4f5e5c5e088305c9509efbd07b68`
pins the complete official x86_64 Linux archive, including its code-mode host,
package metadata, ripgrep, sandbox, voice runtime and Zsh. The archive SHA-256
is `a676f5722aae0d86cbe3764332f08eab1fdd89dc0a300b3ad4ffbf48611fa3e3`.
The overlay leaves other architectures on the existing upstream Nix package.

The first package build stopped before deployment because bundled Zsh needed
`libtinfo.so.6`. The committed fix adds ncurses. Strict autoPatchelf adapts the
dynamic helpers to Nix; the static CLI remains unwrapped with its upstream
package-relative layout. The successful build checked the CLI version/help and
the bundled Zsh, ripgrep and sandbox helper. `make test-quick` passed with
899 passed and 59 skipped after the fix; no routine `test-nix` was run.

Normal whole-machine preparation and activation succeeded. Transaction
`1791817002012299715` selected candidate
`3bbd2bad9833f6a52f1b01435a9c32a12a36b4fe91fdf6b423abc402c2697928`.
Only sys-agent restarted; sys-gui and the other VMs stayed running. The update
closes existing sys-agent Console/Codex processes; persistent conversations
remain available through `codex resume`.

Live sys-agent verification:

- `codex --version`: `codex-cli 0.162.1`.
- Executable: `/nix/store/i1qfmi3r5m707qsvljhcggnvza54y5pl-codex-0.162.1/bin/codex`.
- `codex --help` succeeded; `codex login status` reported ChatGPT login present.
  No authentication file contents were read or copied.
- Running and selected sys-agent system:
  `/nix/store/rnsr0l4ifh1c9zf0div2j2jbmqdrz1i8-nixos-system-sys-agent-26.11`.
- All six guest systems matched selection; `make machine-verify` passed at the
  pre-reboot boundary, with coordinator phase `reboot-required` and no error.

Codex is usable in the live session. The selected host
`/nix/store/h8r00gipvjilhy2f77mpgpnfa99w96dh-nixos-system-dom0-26.11.20260904.801bef6`
still needs a host reboot and post-reboot verification for complete whole-machine
deployment. It reuses the existing initrd. No host reboot was initiated for this
update. No model request or full interactive Codex task was submitted as a test.
