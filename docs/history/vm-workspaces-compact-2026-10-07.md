# Compact occupied VM workspace bar, 2026-10-07

Component `073c68a`, pinned by parent `bc681ec`, keeps GNOME's native Activities
control on the left and adds a compact numbered tab bar at the beginning of the
right panel section. VM groups use a continuous Qubes-colour underline, hover
labels and a separate active-workspace highlight. Mixed workspaces have neutral
colour and show their actual VM names. Manual window movement is unrestricted.

Workspace lifetime follows open application windows, including minimized windows.
No VM receives a reservation; sys-gui has no group. Empty workspaces disappear,
and each VM's occupied slots remain contiguous. One underlying blank workspace
remains when no applications are open, with no VM bar entries. Shift-click moves
the group's focused window to a new occupied slot for experimenting with multiple
workspaces. No saved layout or add-workspace menu remains.

Validation was deliberately limited for this POC: five focused pytest checks,
JavaScript syntax, diff whitespace, commit-hook lint and Python compilation.
The focused checks cover occupied-only grouping, stable group order/removal,
neutral mixed identity, preserving native Activities, tab activation/tooltips,
empty-slot removal and unrestricted manual moves. No full quick suite or Nix test
gate was run. Visual acceptance is left to the operator.

The clean source was pushed to sys-agent. The documented controller single-guest
plan/apply route built and activated only the changed sys-gui generation,
restarting sys-gui and its running sys-agent/usr-home dependents. No host reboot
was required. Worker `airlock-machine-deploy-d4584689c6cd` completed successfully
at 09:42:10 UTC, transaction `1791365917355493408`, candidate
`adfa3a3f50cc912af6e4ecaa78d79d808cb270835ca58e4100dc503981ebcafb`.

`make machine-verify DEPLOY_TARGET=user@SYS_AGENT_ADDRESS` reports **complete**:
the retained host `aaq4px7jylgzpa21v4mg6phd2zlq7cr6` and all six guest generations
match selection. Sys-gui runs
`/nix/store/0jl32xbsnl3wm5nkradp2rnml8rzb3q3-nixos-system-sys-gui-26.11`.
Remote access restored automatically after restart. GNOME reports the workspace
extension version 2 enabled and **ACTIVE**. A focused journal inspection found
no workspace extension JavaScript/CSS errors. Existing unrelated desktop startup
warnings were present. No runtime configuration edit or diagnostic overlay was
used. Deployment is ready for manual hover/click, window launch/close, overview
and multiple-workspace acceptance.

Evidence: `.airlock-logs/vm-workspaces-compact/` (`focused.xml`, `guest-plan.log`,
`guest-apply.log`, `live-verify.log`). Live extension state and the retained worker
journal were also inspected through a read-only dom0 session, then disconnected.
