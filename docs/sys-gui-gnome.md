# GNOME sys-gui milestone

Status: GNOME and forwarded Firefox/Calculator windows verified live on the
physical lab machine; declarative fixes still need a fresh-image boot check.

`usb-gui-lab with Xen` is the primary physical-GPU sys-gui experiment. It uses
the stock GNOME desktop from the pinned Nixpkgs package set, without additional
GNOME Shell extensions.

Hyprland + DankMaterialShell remains available at
`usb-gui-lab-hyprland with Xen`; the known-good XFCE fallback is
`usb-gui-lab-xfce with Xen`. The three choices share the same GPU assignment,
firmware preparation, input forwarding, guest kernel, and sys-gui role. They
differ only in the selected desktop implementation.

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
