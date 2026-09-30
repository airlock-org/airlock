# Make display and appearance consistent across VMs

Status: planned. Created 2026-09-30. The Tailscale enrollment window is reported
to have a tiny mouse cursor and a white terminal background; its live cursor
behavior has not been diagnosed.

Make application windows look and behave consistently with the GNOME desktop
in `sys-gui`, regardless of which VM owns the application. Cover cursor theme
and apparent size, readable fonts and controls, light/dark appearance, terminal
styling, and usable display layouts. Apply this to all domains that display
applications, including local `sys-gui` utilities; headless service VMs do not
need a graphical session for this task.

## Current gaps

The Tailscale enrollment shortcut launches `xterm` in `sys-gui` and runs the
enrollment command in `sys-dev` through `airlock.DevTailscaleUp`. Its launcher
sets a fixed font size but no explicit colours. The development dom0 terminal
also uses xterm, with its own explicit colours and font size. These utility
windows need to follow the same appearance policy as ordinary applications.

`app-gui-guest.nix` sets GTK rendering scale and `XCURSOR_SIZE` before the
application session starts. Personal VM defaults currently select integer
scale 3 for the tested GNOME 250% fractional-scale setup. This is a lab default,
not automatic synchronization with arbitrary monitors. Light/dark changes in
`sys-gui` also do not currently propagate into application VMs.

## Shared defaults and utility terminals

- [ ] Inventory the launch paths and effective settings for local utilities,
  `usr-home`, `usr-browser`, and any other graphical domains. Diagnose the
  small Tailscale mouse cursor separately from the terminal's text cursor and
  background colours.
- [ ] Define reusable Nix defaults for cursor theme/size, fonts, rendering
  scale, and appearance. Install required theme/font assets in each relevant
  domain and ensure normal RPC launches inherit the intended session settings.
- [ ] Replace the Tailscale and development utility xterm launchers with
  GNOME Console, which is already used by `usr-home`. Verify support in the
  pinned version and preserve command output, failure messages, and a usable
  window after command completion.
- [ ] Preserve the existing RPC services, execution domains, and development
  access restrictions. The terminal renderer's VM and the remote command's
  execution VM remain distinct; keep endpoint descriptions clear.
- [ ] Make utility terminals follow GNOME's appearance preference and use
  consistent font and cursor settings. Keep ordinary user terminal launches
  routed to `usr-home` as before.

## Synchronize user appearance preferences

Use GNOME in `sys-gui` as the source of the user's selected appearance
preferences, with Nix defining defaults and the supported synchronization
mechanism. Each VM retains its own settings and session.

- [ ] Implement the narrow appearance synchronization described in the
  [GNOME notes](../sys-gui-gnome.md#deferred-appearance-synchronization), starting
  with light/dark preference and cursor theme/size. Use validated, allowlisted
  values over a dedicated qrexec service restricted to intended graphical
  domains; do not share a complete dconf database or session bus.
- [ ] Apply current preferences when an application VM starts, including VMs
  that were stopped when a preference changed. Forward later changes to running
  VMs and distinguish settings that apply immediately from those requiring an
  application or session restart.
- [ ] Check GTK, Qt, and legacy X11 applications that Airlock actually ships.
  Add toolkit-specific handling where necessary; a single environment variable
  does not establish consistent results across all applications.

## Scaling and display layout

- [ ] Match apparent text, control, and mouse-cursor sizes across local and
  forwarded windows. Account for GNOME fractional scale and the Qubes Xwayland
  rendering scale separately; do not copy fractional values directly into
  integer `GDK_SCALE` or multiply scale twice.
- [ ] Verify the existing Qubes monitor-layout path after resolution changes
  and monitor attachment/removal. Windows must remain reachable and the pointer
  must reach their full content area.
- [ ] Test movement between monitors, including mixed DPI. Establish supported
  behavior and record any per-application or per-session scaling limits rather
  than claiming identical numeric settings guarantee identical apparent sizes.

## Acceptance checks

- [ ] Add focused pytest coverage for shared defaults, launcher configuration,
  validated preference propagation, and application-VM startup reconciliation.
  Run `make test-quick` and retain results under `.airlock-logs/`.
- [ ] Follow the [whole-machine deployment workflow](../machine-deployment.md).
  Verify local Tailscale/development terminals, Files and Terminal in `usr-home`,
  and Firefox in `usr-browser` on the resulting live generation.
- [ ] Move the mouse between GNOME chrome and each application's content;
  confirm consistent apparent cursor size and readable text/controls. Check
  light/dark changes, command completion/failure output, and restart behavior.
- [ ] Verify VM stop/start, logout/login, reboot, resolution changes, and monitor
  changes. Record build results and live acceptance separately, including
  unresolved toolkit or mixed-DPI limits.

Preserve the Qubes architecture and existing VM ownership boundaries. Keep
trusted VM names and label colours distinct: consistent styling must not erase
window identity. The [focused VM indicator](../completed/gnome-focused-vm-indicator.md)
is complete; [Files launcher visibility](sys-gui-files-launcher.md) remains a
separate TODO.
