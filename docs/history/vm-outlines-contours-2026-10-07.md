# VM window outlines and overview contours, 2026-10-07

Components `cec9a9f` and `42249a4`, pinned by parent `73dc805`, give every
forwarded application VM window the same two-pixel outline in its trusted
Qubes label colour. This includes usr-home as well as sys-agent; docked tray
icons remain excluded and applications retain their own title bars.

The large overview outline follows the rendered wallpaper's radius, including
its allocation scale. Its native background frame uses the same radius. Active
and inactive outlines have equal thickness, with selection expressed through
opacity. Small thumbnails and their selection frame remain square.

The controller single-guest build succeeded. Initial activation transaction
`1791382619126295093`, worker `airlock-machine-deploy-7ddad7cfa499`, failed in
`stopping`: qvm-shutdown sys-gui returned exit 1. When access returned, sys-gui
still ran the previous generation and GUI daemon. The reported missing usr-home
border therefore preceded activation of the new code. The shutdown failure's
underlying cause has not been established.

Normal `make machine-resume` completed as transaction `1791384108766832186`,
worker `airlock-machine-deploy-9c3b85f198ed`, candidate
`1697bab93588d96941c5634a4695ee4812e63b62a7ac6ff49ab4b15cf5501433`.
`make machine-verify` reports **complete**, with host and all six guests matching
selection. Only sys-gui changed, to
`/nix/store/19rpkx5k8x1iv02aanah8zrpimzk9pxh-nixos-system-sys-gui-26.11`.
Its sys-agent and usr-home dependents restarted; no host reboot was needed.
Live GNOME reports workspace extension version 9 enabled, state 1, with no
extension error. Both VMs' GUI daemons now use the newly built
`8k0a6hblp837fpdkpwr941bg5k2a09yh` package with their respective red/blue labels.

No test suite ran, as requested for the POC. Syntax checks and normal commit-hook
lint/compilation passed. No runtime configuration overlay was used. Window
outline appearance and overview corner alignment remain for manual review.
Evidence: `.airlock-logs/vm-outlines-contours/`.
