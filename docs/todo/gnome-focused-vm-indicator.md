# Show the focused window VM in the GNOME panel

Status: planned. Created 2026-09-30. Metadata access from GNOME Shell and live
focus behavior remain unverified.

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

- [ ] Inspect suitable maintained upstream GNOME Shell extensions before
  choosing a custom extension. Verify compatibility with Airlock's pinned
  GNOME version and Qubes GUI path.
- [ ] Confirm the effective forwarded-window metadata in the live session.
  Qubes' GUI daemon sets `_QUBES_VMNAME`, `_QUBES_LABEL`, and
  `_QUBES_LABEL_COLOR` on its local windows; use the existing Qubes identity
  source rather than parsing application titles or guessing from app names.
  See the [upstream GUI daemon](https://github.com/QubesOS/qubes-gui-daemon/blob/main/gui-daemon/xside.c).
- [ ] Determine how GNOME Shell can reliably access this metadata for
  forwarded Xwayland windows. Trace its provenance and establish how local
  windows are identified; missing metadata must remain distinguishable from
  confirmed local ownership.
- [ ] Track focus through Mutter's window API. Use event-driven updates and
  keep metadata reads from blocking the Shell. See
  [Mutter's focused-window property](https://gnome.pages.gitlab.gnome.org/mutter/meta/property.Display.focus-window.html).
- [ ] Package and enable the extension declaratively in `airlock-machine`,
  following the existing custom GNOME extension packaging where appropriate.
  Disconnect signals and remove panel elements cleanly on disable.

The indicator only observes focus and identity. Preserve Xen, qrexec,
QubesDB, qubesd, libvirt, and Qubes policy; displaying identity must not
introduce VM control actions or require broader control permissions.

## Acceptance checks

- [ ] Add focused pytest coverage for ownership resolution and state changes,
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
