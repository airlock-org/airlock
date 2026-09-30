# GNOME sys-gui milestone

Status: GNOME and forwarded Firefox/Calculator windows verified live on the
physical lab machine. The report below originally called for fresh-image
verification of its declarative fixes. Later whole-machine boot/deployment
evidence is in [machine deployment](machine-deployment.md); do not infer that
every appearance, login/lock, and window-integration check below is complete.

The normal **Airlock** USB entry now uses the physical-GPU GNOME desktop
(internal profile: `usb-lab`). See [USB development](usb-dev.md) for build,
boot, and recovery instructions. Hyprland and XFCE builders remain in source,
but the older experimental menu entries are no longer shipped as desktop
choices. The evidence below records the GNOME integration milestone; it does
not establish general hardware compatibility or close every lifecycle check.

GNOME's ordinary host-facing convenience services are explicitly disabled in
sys-gui: NetworkManager, Bluetooth, Avahi, GNOME Remote Desktop, user sharing,
and Rygel. GNOME Initial Setup is also disabled: the image already has its
local user and login policy. Sys-gui still has no NetVM. This does not change
the existing Qubes input, GUI, or PCI assignment boundaries.

Build and provision:

```console
make usb-dev-image
make usb-dev-provision USB_SERIAL=25110814640207
```

## Cross-VM app display fixes (2026-09-15)

Firefox in `usr-browser` and Calculator in `usr-home` were verified through
their normal desktop launchers, with visible cursors and working resize/maximize.
The image configuration now captures the live fixes:

- Mutter uses the upstream Qubes Xwayland wrapper, which preloads `shmoverride`.
  Mutter embeds the executable path, so an Xorg-only wrapper is insufficient.
- `/var/log/qubes` is group-writable by `qubes` for the user's GUI daemons.
- The guest GUI service sets a short `ENV_PATH`: the upstream session launcher
  cannot accommodate Nix's long generated service PATH.
- The guest qrexec-fork-server autostart entry uses its Nix store executable,
  so normal app RPCs inherit the graphical session environment.
- `airlock.appGui.scale` sets GTK rendering and cursor scale before upstream
  session startup imports the environment into D-Bus/systemd and qrexec.

The personal-VM defaults currently select scale **3**, matching the tested
panel's Xwayland rendering scale at GNOME's **250%** fractional monitor scale.
This is a lab default, not automatic monitor/DPI synchronization. Override
`airlock.appGui.scale` in the guest configuration for other displays (the base
module defaults to 1). Firefox and Calculator text size was confirmed live.

The outer GNOME title bars remain. Removing duplicate decorations while keeping
trusted VM identity visible is unfinished; do not treat these changes as
complete Qubes window-label integration. Recheck launches, scaling, cursor,
resize/maximize and login/lock after building and booting the updated image.

## TODO: Hide local Files from GNOME

The user reports that GNOME exposes a Files launcher that may start Nautilus
in `sys-gui`. Confirm its origin on the live desktop. Where `usr-home` is
available, the visible Files launcher should open the file browser in
`usr-home` through the existing Qubes RPC path.

Keep Nautilus installed in `sys-gui`: the GNOME configuration explicitly
retains it for DING's desktop file operations. The existing
`airlock-files.desktop` launcher targets `usr-home` and is the declared
`inode/directory` default, but its distinct desktop ID does not hide the
local Nautilus entry.

- [ ] Identify the visible Files entries and confirm which VM each starts.
- [ ] Hide the local Nautilus application entry declaratively in
  `airlock-machine`, scoped to GNOME profiles with personal VM launchers
  enabled. Evaluate `NoDisplay=true` first; it suppresses menu visibility
  while retaining the entry, as described in the
  [Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).
- [ ] Check GNOME's application grid, app search, Files search provider,
  desktop shortcuts, and existing dock favourites for exposure of local Files.
  Ensure the ordinary visible Files launch routes to `usr-home`.
- [ ] Preserve the local Nautilus executable and DING's required operations.
  Inspect desktop-folder actions separately: hiding a launcher does not
  prevent direct invocation, and a `sys-gui` path cannot simply be treated as
  the same path in `usr-home`.
- [ ] Preserve local Files visibility in minimal profiles without `usr-home`.
- [ ] Extend pytest coverage in
  `airlock-machine/tests/test_usr_home_gnome.py` to verify the effective desktop
  entry, retained Nautilus dependency, existing RPC launcher and MIME default,
  and minimal-profile behavior. Run `make test-quick`.
- [ ] Follow the [whole-machine deployment workflow](machine-deployment.md)
  and verify the resulting live generation: only the `usr-home` Files launcher
  is exposed, its process runs there, and DING's desktop operations still work.
  Record build and live verification separately.

This task changes desktop visibility while preserving the existing Qubes
architecture and policy. It does not remove local Nautilus or prevent its
intentional use by desktop integration.

## Deferred: appearance synchronization

Changing light/dark appearance in sys-gui does not currently update applications
running in `usr-home` or `usr-browser`, because each VM has its own GNOME/dconf
settings and session bus. Add a narrow appearance-sync mechanism later:

- watch sys-gui's `org.gnome.desktop.interface color-scheme` value;
- send only allowlisted appearance values over a dedicated qrexec service;
- restrict policy to sys-gui targeting `usr-home` and `usr-browser`;
- apply the value to the desktop user's GNOME settings in each target VM; and
- apply the current value when an application VM starts, as well as forwarding
  later changes to already-running applications.

Do not share the complete dconf database or a session bus between VMs. The same
mechanism may later gain separately allowlisted accent colour, high-contrast and
text-scaling values. Window-manager chrome already belongs to sys-gui and does
not need this synchronization.
