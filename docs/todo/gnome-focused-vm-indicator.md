# Show the focused window VM in the GNOME panel

Status: implemented and deployed for early testing, 2026-09-30. The operator
tested the indicator and reported that it looks good. Detailed acceptance checks
and host-generation verification after reboot remain pending.

Calculator follow-up (2026-09-30): the operator found that switching from
Firefox to Calculator hid the indicator completely. Calculator's active
Xwayland window had valid `usr-home`, label 6 and `#3465a4` metadata, and the
packaged reader resolved it correctly. Machine revision `3750010` adjusts the
focus handling: application actors in Mutter's window groups are distinguished
from Shell controls, grab/key-focus changes trigger a coalesced idle refresh,
and that refresh rereads Mutter's focused window after the transition settles.
Pending results are invalidated immediately; disable removes the queued refresh.
The added pytest check exercises application actors, Shell interaction, settled
focus refresh and disable cleanup. The quick suite passed **664 tests, 58 skipped**.

Normal whole-machine plan/apply deployed the follow-up and restarted sys-gui,
usr-home and usr-browser. GNOME reports the extension **ACTIVE**, and the
operator retested the Calculator case and reported that it works now. Typed
verification of transaction `1790780281071143141` is **reboot-required**, with
all seven running guests matching their selections. No live code overlay or
diagnostic configuration mutation was used. Follow-up evidence is in
`.airlock-logs/gnome-focused-vm/calculator-fix/`, including `plan.json`,
`apply.json`, `verify-after.json`, and `acceptance.json`.

Machine revision `eb939a1` is committed on main and deployed through normal
whole-machine plan/apply to the discovered development SSH target. The quick
suite passed **663 tests, 58 skipped**. GNOME Shell **50.4** reports the extension
enabled and **ACTIVE**, with no extension errors found in the session journal.
The packaged reader resolves live Files metadata to `usr-home` and `#3465a4`.
Typed machine verification reports transaction `1790778349293663479`
**reboot-required**: all seven running guests match their selected generations,
including the updated sys-gui. The host still runs its previous generation.
No diagnostic configuration mutation or live overlay was used. Evidence is under
`.airlock-logs/gnome-focused-vm/`, including `plan.json`, `apply.json`,
`verify-after.json`, `live-checks.json`, and `acceptance.json`.

Add a GNOME top-panel indicator in `sys-gui` showing the VM that owns the
window with keyboard focus. Display the VM name beside a small dot in its
Qubes label colour, positioned on the left near the workspace indicator.
For example, focusing Files in `usr-home` should show `usr-home`; switching
to Firefox in `usr-browser` should immediately show `usr-browser`.

## Display behavior

- Follow keyboard focus, including mouse selection, Alt+Tab, workspace
  changes, and window closure. The window under the pointer alone does not
  determine the indicator.
- Show `sys-gui` for positively identified local application windows.
- Clear the indicator when no application window is focused. Show
  `Unknown VM` when a focused application's ownership cannot be determined;
  never retain the previous VM name or infer `sys-gui` solely from missing
  Qubes metadata.
- Resolve application dialogs consistently with their owning VM. Define and
  verify behavior for transient windows, GNOME menus, and the overview so the
  indicator does not misrepresent which application will receive input.
- Keep the VM name readable without relying on colour alone. Render names as
  plain text and handle long names without displacing essential panel controls.
- Preserve ordinary GNOME fullscreen behavior. The panel may be hidden in
  fullscreen; this first milestone does not force it to remain visible.

Retain existing window labels and decorations. This indicator supplements
window identity; removing duplicate title bars or completing wider Qubes
window-label integration is separate work.

## Implementation investigation

- [x] Inspect suitable maintained upstream GNOME Shell extensions before
  choosing a custom extension. Verify compatibility with Airlock's pinned
  GNOME version and Qubes GUI path.
- [x] Confirm the effective forwarded-window metadata in the live session.
  Qubes' GUI daemon sets `_QUBES_VMNAME`, `_QUBES_LABEL`, and
  `_QUBES_LABEL_COLOR` on its local windows; use the existing Qubes identity
  source rather than parsing application titles or guessing from app names.
  See the [upstream GUI daemon](https://github.com/QubesOS/qubes-gui-daemon/blob/main/gui-daemon/xside.c).
- [x] Determine how GNOME Shell can reliably access this metadata for
  forwarded Xwayland windows. Trace its provenance and establish how local
  windows are identified; missing metadata must remain distinguishable from
  confirmed local ownership.
- [x] Track focus through Mutter's window API. Use event-driven updates and
  keep metadata reads from blocking the Shell. See
  [Mutter's focused-window property](https://gnome.pages.gitlab.gnome.org/mutter/meta/property.Display.focus-window.html).
- [x] Package and enable the extension declaratively in `airlock-machine`,
  following the existing custom GNOME extension packaging where appropriate.
  Disconnect signals and remove panel elements cleanly on disable.

The installed custom extension follows `notify::focus-window` and Shell keyboard
focus. Overview, Shell menus/modal interaction, and lock/greeter states suppress
the application indicator; application focus actors do not. Keyboard-focus and
grab transitions refresh at idle after Shell finishes changing state. Native
Wayland application windows are local to
sys-gui in this GUI path. For Xwayland windows, an asynchronous Python-Xlib
reader obtains bounded Qubes properties and uses XRes's server-reported client
PID plus `/proc/<pid>/exe` to identify the installed `qubes-guid` binary. It
accepts forwarded identity only from that daemon; absent properties on another
confirmed local client resolve to sys-gui. Missing client provenance, malformed
identity, and destroyed windows resolve to unknown. Transient windows without
Qubes properties can use their Mutter parent; invalid properties do not trigger
that fallback. Read cancellation and per-focus revisions prevent stale results,
including across extension disable/re-enable. Invalid colour suppresses the dot
while preserving a verified VM name.

Verified direct identities and label colours are cached by the live
`Meta.Window` object, rather than an XID, title or application name. A repeated
focus renders the cached identity synchronously and starts no reader. The cache
is removed on `unmanaged` (including unfocused windows) and cleared on extension
disable. Cancelled or late lookups cannot repopulate it. Unknown results are
retried, and transient-parent fallback is not stored on the child, so a changed
parent is resolved again. Native Wayland windows display sys-gui synchronously.
The [GUI daemon sets identity and label properties when creating each local
window](https://github.com/QubesOS/qubes-gui-daemon/blob/main/gui-daemon/xside.c);
this cache follows that window lifetime. Direct property edits on an existing
window are not monitored; recreating the window or extension clears its entry.
A new Xwayland window still needs its first asynchronous provenance lookup.

[Tasks in Panel](https://extensions.gnome.org/extension/8642/tasks-in-panel/)
provides application/task UI, and
[Focused Window D-Bus](https://github.com/flexagoon/focused-window-dbus)
exports focused title/class information. Neither supplies the Qubes ownership
resolution needed here. Airlock therefore packages its small GNOME 50 extension
alongside the existing host-power-off extension. It adds no D-Bus observation
service or VM control actions.

The indicator only observes focus and identity. Preserve Xen, qrexec,
QubesDB, qubesd, libvirt, and Qubes policy; displaying identity must not
introduce VM control actions or require broader control permissions.

## Acceptance checks

- [x] Add focused pytest coverage for ownership resolution and state changes,
  including absent or invalid metadata, destroyed windows, and delayed results
  after focus changes. Check declarative extension installation and enablement.
  Run `make test-quick` and retain results under `.airlock-logs/`.
- [ ] Follow the [whole-machine deployment workflow](../machine-deployment.md)
  and verify the resulting generation live with `usr-home`, `usr-browser`,
  and local `sys-gui` application windows.
- [ ] Verify click focus, rapid Alt+Tab, workspace switches, dialogs, window
  closure, no focused application, overview/menus, and multiple monitors.
  Confirm that unknown ownership never displays a stale or guessed VM name.
- [ ] Verify readable name/colour rendering, normal fullscreen behavior, and
  clean extension disable/re-enable and logout/login without duplicate panel
  elements. Record build results and live verification separately.
