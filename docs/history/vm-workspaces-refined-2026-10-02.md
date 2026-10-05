# Refined VM workspace prototype, 2026-10-02

Component `714b6c3`, pinned by parent `d32b3f8`, simplifies the GNOME workspace
prototype. Sys-gui has no dedicated workspace group. Application VMs have
normal-size name buttons and small dots in their Qubes label colours, with a
wider active dot. Clicking a VM name returns to its last used slot; a single
`⋮` menu adds adjacent slots. There are no numbered labels, persistent add buttons,
visible mixed-workspace warnings or panel scrollbars. The focused-VM indicator
continues to identify the actual focused window; movement remains unrestricted.

Initial allocation reuses existing workspace objects and removes only redundant
empty slots. Occupied leftovers remain intact under the personal VM's group.
Native sys-gui windows receive no routing or mixed-VM warning. Existing saved
application-VM counts survive the removal of the old local group. Unchanged
inventory refreshes preserve panel actors, keyboard focus and an open menu.

A read-only live check found the original extension `INACTIVE` while the screen
was locked, despite remaining enabled. Its saved layout contained usr-home,
sys-agent and a local slot. The revised metadata permits `unlock-dialog` so the
controller retains workspace state while locked; the panel and its popup menu
are hidden. This avoids running preference restoration on every screen lock.

Local validation passed **836 quick tests, 59 skipped**, lint and Python
compilation. Tests include reusing empty slots, preserving occupied leftovers,
ignoring native GUI windows, dropping the old local group, menu stability on
unchanged inventory and lock-screen hiding. Guarded Airlock GC skipped below
its threshold. The whole-machine release built successfully:
`/nix/store/4pjjgbwa7g6v3bqhm3xbrl6l7797p299-airlock-machine-release`.
The built package metadata and source hashes were recorded, including the
user/unlock-dialog session modes.

Committed source was pushed to the clean `/home/user/airlock-machine` checkout.
Controller guest plan/apply targets sys-gui; only sys-gui changes, with sys-gui,
sys-agent and usr-home restarted for activation. Detached worker
`airlock-machine-deploy-57a46217aa6b` owns the update.

The worker finished inactive with `Result=success` and exit 0. Transaction
`1790962552320744861` runs and selects
`/nix/store/5qvhnb2fja19hlk49r0527366rv8kmw5-nixos-system-sys-gui-26.11`.
The installed source and stylesheet hashes match the locally built package,
and its metadata includes user/unlock-dialog. `make machine-verify` succeeded
with all six running guests matching their selections. The existing host
`reboot-required` state remains unchanged (`mckynwhd...` running, `9yrfxaq3...`
selected). No live configuration overlay or diagnostic mutation was used.

At the last live check on 2 October, GNOME was at GDM after restart. Signed-in
extension validation and visual feedback on the revised panel remained pending.
The 5 October follow-up requires a fresh Tailscale SSH authentication check;
current session state has not yet been inspected.

Evidence: `.airlock-logs/vm-workspaces-refined/` and the build record beneath
`.airlock-logs/machine-deploy/d32b3f858b23ade426e98d1933428cd7e65d1840/`.
