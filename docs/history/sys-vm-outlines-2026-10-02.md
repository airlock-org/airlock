# System VM window outlines, 2026-10-02

Component `0660b1b`, pinned by parent `04ab749`, changes only GNOME's sys-gui
GUI daemon decoration policy. Forwarded windows from VMs named `sys-*` receive
the existing Qubes two-pixel forced outline in their trusted VM label colour.
Other forwarded windows and docked tray icons receive no added outline.
Applications continue to own their title bars; Motif hints suppress additional
window-manager decorations. The rule uses the daemon's Qubes VM name, never
application class or guest-supplied window metadata. Native sys-gui applications
are unaffected. Maintenance guidance is in `airlock-machine/docs/sys-gui.md`.

Validation passed **824 quick tests, 59 skipped**, lint and Python compilation.
The local release build succeeded, including application of the decoration
patch and compilation of Qubes GUI daemon 4.3.15. Its output was
`/nix/store/g7a2i8sq0k33ba2r2jmhbh9svd5q1wly-airlock-machine-release`.
Guarded Airlock GC skipped collection below its threshold.

The committed source was pushed to the clean `/home/user/airlock-machine`
checkout in sys-agent. The controller `guest-plan` / `guest-apply --target
sys-gui` route changed only sys-gui's selection and restarted its dependent
sys-agent and usr-home guests. Detached worker
`airlock-machine-deploy-f47c26ec42e3` exited 0 with `Result=success`.
Transaction `1790957619169849131` selected and started
`/nix/store/7vssv92q9ndjm894c2581yk6fyjd5wxf-nixos-system-sys-gui-26.11`.
The running sys-gui processes use the locally built daemon package
`/nix/store/0hkyacaqa2miaygwary025pmc6iyxqdd-qubes-gui-daemon-4.3.15`.

After startup settled, `make machine-verify` succeeded: all six running guest
generations match their selections. Whole-machine state remains
`reboot-required`, with the same previously running host `mckynwhd...` and
selected host `9yrfxaq3...`; this guest update did not change that pending host
selection or reboot the laptop.

Read-only X11 pixel samples before deployment showed neutral edges on
sys-agent Console, usr-home Console and usr-home Firefox. After deployment,
sys-agent Console's first two left/top edge pixels and its right/bottom edges
are `0xcc0000`, matching its Qubes label; the third pixel retains the native
neutral application colour. Its Motif no-decoration hints remain set, and its
X server border width remains zero: the outline is drawn in the forwarded
window, rather than supplied by an additional window-manager frame. The
operator signed in, reported both Console and Firefox open, and confirmed
that the result looks OK. Only Console remained visible to the later automated
pixel probe, so post-deployment Firefox appearance has manual acceptance rather
than an automated pixel comparison.

No live configuration overlays or diagnostic configuration mutations were used.
Evidence: `.airlock-logs/sys-vm-outline/` and
`.airlock-logs/sys-vm-outline-quick.log`.
