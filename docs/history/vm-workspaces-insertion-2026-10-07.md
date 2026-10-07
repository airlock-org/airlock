# Native workspace insertion and overview outline, 2026-10-07

Component `e10622a`, pinned by parent `4b28a39`, restores GNOME's native window
drag insertion placeholder at thumbnail edges/gaps adjoining the source VM's
group. Dropping creates an owned workspace and moves that window; existing
workspace objects stay attached to their windows. A single occupied workspace
shows its thumbnail during a drag. No persistent empty slot or separate add
control is introduced. Cross-VM drop restrictions remain in place.

The large overview outline now sits above the wallpaper and below application
previews. Its radius and opacity follow GNOME's animated background; small
thumbnails and their selection frame keep square corners.

No test suite ran, as requested for the POC. JavaScript syntax and normal
commit-hook lint/compilation passed. The controller single-guest build and apply
succeeded without recovery: worker `airlock-machine-deploy-540e605166a5`,
transaction `1791380453588127673`, candidate
`ac3bff7ee660070b04bb753705c105e54e1ec2976093e50f9904b12a3c4721ce`.
Whole-machine verification reports **complete**, host and all six guests matching
selection. Only sys-gui changed, to
`/nix/store/1fybghcvra7qp0qz3nl8c0zvpfvm4rsn-nixos-system-sys-gui-26.11`;
sys-agent and usr-home restarted as dependents. No host reboot was needed.
GNOME reports extension version 8 enabled, state 1, with no error. No runtime
configuration overlay was used. Actual drag creation, cancellation and border
appearance remain for manual review. Evidence:
`.airlock-logs/vm-workspaces-insertion/`.
