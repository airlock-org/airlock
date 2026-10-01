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

GNOME's forwarded-window policy now leaves all window chrome to applications:
the GUI daemon requests no outer title bar or frame and draws no colour border,
including on override-redirect and docked windows. This policy is uniform and
does not select applications by class or inspect their decoration requests.
Windows remain managed normally; the focused VM panel indicator supplies the
trusted identity display. It identifies only the focused window. Applications
that depend on external title-bar controls must supply those inside their VM.
Recheck launches, scaling, cursor, move/resize/maximize, dialogs and login/lock
after deployment; manual acceptance of the new policy is pending.

The [Files launcher TODO](todo/sys-gui-files-launcher.md) tracks hiding local
Files while keeping Nautilus installed for desktop integration.
The [completed focused VM indicator plan](completed/gnome-focused-vm-indicator.md)
records the deployed panel indicator, Calculator focus fix, window identity cache
and successful operator retests.

## Personal VM GUI assignment fix (2026-09-30)

The Files launcher reported that `usr-home` could not start its graphical
session. Both personal VMs had an explicit `guivm=None`, overriding the global
`default_guivm=sys-gui`. They lacked the `guivm-sys-gui` policy tag, so sys-gui
could not discover them; the `usr-home` GUI agent also waited for its missing
GUI-domain QubesDB value.

Machine revision `caff35e` assigns `guivm=sys-gui` explicitly for `usr-home` and
`usr-browser`, in both bootstrap defaults and the initial runtime configuration.
Regression tests cover the physical and nested profiles and runtime defaults.
The quick suite passed **722 tests, 25 skipped**. Normal whole-machine apply
through sys-dev/sys-ctrl converged the two preferences and restored the GUI tags.
Before reboot, `usr-home`'s GUI agent was active and its session had `DISPLAY=:0`,
`GDK_SCALE=3`, and `XCURSOR_SIZE=72`. No diagnostic configuration changes were
needed on the target.

After reboot, typed machine verification reports `complete`, with matching host
and guest generations. Both GUI assignments and the `usr-home` graphical session
survived startup. GNOME's Xwayland client list showed Firefox with
`_QUBES_VMNAME=usr-browser` and its expected window class, and focused Files with
`_QUBES_VMNAME=usr-home` and the Nautilus window class. Both application processes
were running in their respective VMs. The operator confirmed GNOME reached login
and subsequently reported that the desktop looked good. Evidence is under
`.airlock-logs/personal-guivm-fix/`, including `live-final-checks.json`; see
[machine deployment](machine-deployment.md).

## Deferred: appearance synchronization

The [display consistency TODO](todo/cross-vm-display-consistency.md) tracks
this work alongside cursor/font defaults, scaling, and GNOME utility terminals.

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
