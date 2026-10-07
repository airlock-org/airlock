# VM workspace preparation, 2026-10-07

Component `a5fbd2b`, pinned by parent `ae8cbc6`, adds workspace ownership and
prelaunch preparation to sys-gui. The common GNOME launcher resolves the target
VM UUID, reserves and focuses its workspace before sending the application RPC.
Reservations end on a matching window, failure or a 90-second deadline. New
windows use trusted Qubes identity during creation, with a mapping guard while
identity is unresolved. Manual cross-VM moves remain unrestricted in this POC.

No test suite ran, as requested. JavaScript syntax and commit-hook Python
lint/compilation passed. The controller single-guest build and apply succeeded:
worker `airlock-machine-deploy-d8e806f5c586`, transaction `1791376027111091559`,
candidate `9846a2a146c83c025d7d7f4bde894def46c86055d600c6ec8cca525b327d9d16`.
Only sys-gui changed; sys-agent and usr-home restarted as dependents. No host
reboot was required. Whole-machine verification reports **complete**, with the
host and all six guests matching selection. Sys-gui runs
`/nix/store/z8x6d871kk7slqp53fmsbpqk420camg8-nixos-system-sys-gui-26.11`.

After login, GNOME reported extension version 6 enabled, state 1, with no error.
The installed preparation client reserved sys-agent's UUID workspace; the State
method showed one pending request. Cancellation removed the empty slot and State
returned an empty list. No runtime configuration overlay was used. Actual app
mapping and focus transitions remain for manual review. Evidence:
`.airlock-logs/vm-workspaces-prelaunch/` (source, plan, apply, status, verification
and runtime reservation logs).
