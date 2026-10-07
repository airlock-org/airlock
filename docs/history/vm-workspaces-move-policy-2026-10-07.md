# VM workspace move policy, 2026-10-07

Component `fae4faf` (following `0fac988`), pinned by parent `dbe7710`, rejects
foreign-VM window moves in the panel, overview previews and thumbnails, keyboard
move actions and workspace menu methods. Same-VM moves remain allowed and new
unowned slots can join that VM. Sticky application windows are blocked. A native
workspace/property-change guard restores valid placement when Mutter bypasses
Shell hooks. This remains extension policy rather than a hardened compositor rule.

No test suite ran, as requested for this POC. JavaScript syntax checks and normal
commit-hook lint/compilation passed. The controller single-guest build succeeded.
Initial worker `airlock-machine-deploy-3c8545a818f2`, transaction
`1791377243402766857`, failed during sys-gui shutdown (qvm-shutdown exit 1).
Read-only inspection found all six guests running; the precise shutdown failure
was not established. Normal `make machine-resume` worker
`airlock-machine-deploy-cd50f24f63d7` succeeded, transaction `1791377355852355687`.
Candidate `9d68d54a8663bf7bb1f9fd33cc9ea2549dc8651ee1235e82a12e2f1426560160`.

Whole-machine verification reports **complete**, host and all six guests matching
selection. Only sys-gui changed, to
`/nix/store/rib00299hj1h9wnmkdg6m9im05610smh-nixos-system-sys-gui-26.11`;
sys-agent and usr-home restarted as dependents. No host reboot was needed.
GNOME reported extension version 7 enabled, state 1, with no error; the workspace
service responded. No runtime configuration overlay was used. Drag, keyboard and
same-VM interaction acceptance remain manual. Evidence:
`.airlock-logs/vm-workspaces-move-policy/`.
