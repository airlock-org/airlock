# VM colour in workspace previews and panel tabs, 2026-10-07

Component `ed7a61a`, pinned by parent `6dc979f`, adds VM-coloured outlines to
GNOME's existing overview workspace previews and small thumbnails. Hover labels
show the full VM name and workspace number, with a short delay and suppression
during window/application dragging. Nonreactive border overlays preserve native
click/drag gestures. Small GNOME 50 constructor hooks cover newly created previews;
existing previews are decorated too. Disabling removes overlays/hooks and restores
the preview actors' original input properties.

Panel tabs use their VM colour at 20% opacity normally, 30% on hover/focus and 40%
when active. Numbers stay opaque, a light outline marks selection, and the solid
colour underline still connects each VM's group. Mixed/unidentified workspaces
remain neutral. Workspace lifetime and grouping are unchanged.

Checks were intentionally focused for this POC: six existing/extended pytest cases,
JavaScript syntax, whitespace checks, commit-hook lint and Python compilation.
The new case checks existing/future overview decoration, tint levels, hover label
content, drag suppression and cleanup. No full quick suite or Nix test gate ran.
Visual acceptance remains manual.

The controller single-guest plan/apply route built and deployed only the changed
sys-gui generation, restarting its sys-agent/usr-home dependents. Worker
`airlock-machine-deploy-f0f364dda148` completed successfully at 10:06:34 UTC.
Transaction `1791367386189972236`, candidate
`cf9106edfcadb21ea1c5a6daa462de64dfd623f5204628158f44676bf84d56dd`.

`make machine-verify DEPLOY_TARGET=user@100.104.92.101` reports **complete**:
the retained host and all six guests match their selected generations. Sys-gui
runs `/nix/store/apyfnqplca5mv1k4j2wwsw5981fraiwi-nixos-system-sys-gui-26.11`.
Remote access restored automatically. A read-only observation found GNOME at its
login screen; the user-session extension could not yet be queried, so its runtime
state and visual behaviour are pending sign-in/manual acceptance. No live
configuration edit or diagnostic overlay was made. The dom0 observation session
was closed before handing this POC over for review.

Evidence: `.airlock-logs/vm-workspaces-colours/` (`focused.xml`, `guest-plan.log`,
`guest-apply.log`, `live-verify.log`).
