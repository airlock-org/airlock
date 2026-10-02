# Console action icons, 2026-10-02

Console 50's new-tab button requests `tab-new-symbolic`. Before this change,
sys-agent exposed only Hicolor in its active icon profile. A read-only GTK 4
probe confirmed that the selected theme was Adwaita, `has_icon` returned false
for this name, and lookup returned the built-in `image-missing` placeholder.
Other tested basic action icons were available through GTK's limited fallback.
The packaged Console UI resource confirmed the requested icon name, and the
Adwaita 50 package contains that icon.

Component `c08fc7f`, pinned by parent `f89e405`, adds `adwaita-icon-theme` to
the shared `app-gui-guest.nix` module alongside Hicolor. Its existing
`/share/icons` profile projection exposes standard action icons inside the VM
where applications run. No Console-specific override, desktop session,
theme-selection change or sys-gui update is needed. Both declared application
VM configurations and future consumers of the shared module receive the assets.

Local validation passed **824 quick tests, 59 skipped**, lint and Python
compilation. The whole-machine release built successfully:
`/nix/store/dxb9pvi5gr7q60q1z2dxpzi0jdadvh5r-airlock-machine-release`.
Its sys-agent profile links Adwaita into `share/icons`. Guarded Airlock GC
skipped collection below its threshold.

Committed source was pushed to the clean `/home/user/airlock-machine` checkout.
The controller `guest-plan` / `guest-apply --target sys-agent` route activated
only sys-agent, leaving GNOME and usr-home running. Detached worker
`airlock-machine-deploy-68335434fe13` exited 0 with `Result=success`.
Transaction `1790958896136916790` selected and started
`/nix/store/yclxm263g6mwv2kfnq6qmmn3zk3v905g-nixos-system-sys-agent-26.11`.

After restart, the same GTK probe finds `tab-new-symbolic` and resolves it to
`/run/current-system/sw/share/icons/Adwaita/symbolic/actions/tab-new-symbolic.svg`.
Using Console's existing SVG loader cache, GdkPixbuf loads a nonempty 16x16 image
with alpha. This validates lookup and asset loading; manual inspection of the
reopened Console button remains available to the operator.

`make machine-verify` succeeded with all six running guest generations matching
their selections. Whole-machine state remains `reboot-required` for the earlier
host update: running host `mckynwhd...` and selected host `9yrfxaq3...` are
unchanged. This deployment applies the shared module fix to sys-agent only;
usr-home will receive its new profile selection in a subsequent update.
No live configuration overlays or diagnostic configuration mutations were used.

Evidence: `.airlock-logs/console-icons/`.
