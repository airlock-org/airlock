# Compact VM workspace selectors, 2026-10-07

Component `5f50cd8`, pinned by parent `c136664`, removes panel group gaps in favour
of thin slot dividers and a stronger VM divider. Tabs have square corners and
35%/45%/55% VM-colour fills for normal/hover/active states. GNOME's existing moving
thumbnail selector uses the nearest workspace's VM colour and square corners in
both overview modes. The active small thumbnail has one native selection frame;
its added VM outline is suppressed. Disable restores the original selector style.

No tests were run, as requested for this POC. Existing fixture/colour expectations
were kept aligned; normal commit-hook lint/compilation ran. The controller
single-guest route built and activated sys-gui, restarting its sys-agent/usr-home
dependents, with no host reboot. Worker `airlock-machine-deploy-0022ff06bdee`
completed successfully at 11:45:25 UTC, transaction `1791373334930347165`.
Candidate `4b1a5f5f35c707eb73fc7c97bdf5a8d4709af6248b70f506cde08c570f9bdcb4`.

Whole-machine verification reports **complete**: host and all six guests
match their selections. Sys-gui runs `/nix/store/nvijbld8npjy80j475skgx4krzgp15sp-nixos-system-sys-gui-26.11`.
Remote access restored automatically. GNOME was at its login screen at handoff;
extension runtime and visual acceptance await sign-in/manual review. No live
configuration edit or diagnostic overlay was used. Evidence:
`.airlock-logs/vm-workspaces-selectors/` (`guest-plan.log`, `guest-apply.log`,
`live-verify.log`, `pin.log`).
