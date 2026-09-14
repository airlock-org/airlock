# Hyprland and Quickshell milestone

Status: image configuration added; no physical Hyprland boot verified yet.
The preceding XFCE milestone verified AMD hardware-accelerated OpenGL in
sys-gui. That is a prerequisite, not evidence that Wayland or cross-VM windows
work. See [the GPU investigation](amd-gpu-investigation.md).

## Repository layout

`lifecycle/misc/modules/sys-gui/base.nix` contains the shared Qubes sys-gui
role. `desktop/selector.nix` chooses the desktop implementation; it supports
`xfce`, `gnome`, and `hyprland`. The Hyprland/DMS implementation, its AMD-lab
HiDPI profile, and its DMS package customization live together under
`sys-gui/desktop/hyprland/`. GNOME lives in the sibling
`sys-gui/desktop/gnome/` implementation.

## Build and boot

```console
make usb-dev-image
make usb-dev-provision USB_SERIAL=25110814640207
```

Choose **usb-amd-lab-hyprland with Xen**, then log in as `user` with `password`.
LightDM selects **Airlock Hyprland (experimental)** by default. The shell is
DankMaterialShell v1.6.1, pinned in flake.lock, running on the separate upstream
Quickshell package from our existing nixpkgs pin. Hyprland and Mesa also use that
pin. The Hyprland 0.56 configuration is Lua, not older hyprlang syntax.

For the known-working physical-GPU desktop, reboot and choose
**usb-amd-lab-xfce with Xen**. That guest does not enable Hyprland or DMS.
All physical boot entries use identical GPU assignment, firmware preparation
and input forwarding. The ordinary hybrid XFCE and text recovery entries remain
unchanged. Boot specialisations are not live-switchable GPU modes.

The three guest closures share the prepared backing store; identical store
paths are admitted once, rather than copied separately for each boot choice.
New desktop packages still increase the image payload.

## First experiment

A native Wayland terminal opens automatically, independently of the shell.

- Super+Enter: sys-gui terminal (Foot).
- Super+D: DMS application launcher.
- Super+Shift+D: existing development dom0 terminal through Qubes RPC.
- Super+Shift+E: log out to LightDM, even if DMS has failed.
- Super+Q: close the focused window.
- Super+1 through Super+9: change workspace.

In the sys-gui terminal, inspect:

```console
hyprctl systeminfo
hyprctl monitors
hyprctl configerrors
wayland-info
systemctl --user status airlock-desktop-shell
journalctl --user -u airlock-desktop-shell -b --no-pager
tail -n 100 ~/.local/state/airlock/hyprland.log
```

Check physical display, acceleration (not software rendering), keyboard and
mouse, panel/launcher, terminal, dom0 terminal, logout and return to the desktop.
The session wrapper appends compositor output to the persistent sys-gui home
and stops the shell unit on exit. The shell has no automatic systemd target:
XFCE does not start it.

## Customization and boundaries

GPU/input configuration is separate from `sys-gui/desktop/hyprland/default.nix`. That module
has a replaceable `shellCommand`, so another Quickshell layout can replace DMS
without changing GPU passthrough. DMS-specific launcher bindings and defaults
would then also need adapting; arbitrary layouts do not inherit DMS services.
The packaged DMS QML is immutable. For development, a writable QML tree and a
replacement shell command can be used; preserve the required Qt import paths.

First-login DMS settings are copied to
`~/.config/DankMaterialShell/settings.json` only if absent, preserving later
customization. Initially only logout and host poweroff appear in the power
menu. Poweroff uses the existing `airlock.Dom0Poweroff` RPC; no new dom0
authority is granted. Unintegrated network/audio/brightness controls are omitted
from the control-center defaults. These UI defaults are not a security policy;
other settings can expose local guest controls. Host suspend/reboot, audio,
network integration and lock-screen behavior are not acceptance claims here.

The reusable `sys-gui/desktop` selector chooses a desktop implementation. Its
Hyprland implementation is hardware-neutral. The AMD lab imports its separate
`sys-gui/desktop/hyprland/hidpi-amd-lab.nix` appearance profile, which supplies the
larger terminal, DMS fonts/icons and a small, version-pinned Quickshell patch to
DMS's Spotlight launcher. The patch enlarges its search field, result rows,
icons and modal width; it is deliberately not a change to Hyprland's display
scale and is checked when the pinned DMS package builds. This is the first
example of the intended layout-customization seam; keep future changes similarly
narrow or replace `shellCommand` with a separate Quickshell layout when DMS no
longer fits.

Cross-VM windows are a separate follow-up: the existing Qubes Xorg
`shmoverride` integration is not applied to Hyprland or Xwayland. Xwayland is
enabled for ordinary local X11 applications, not claimed to provide working
Qubes window transport. Verify that transport, trusted VM labels, focus and
clipboard isolation before treating this as a replacement production desktop.

Upstream references: [Hyprland NixOS integration](https://wiki.hypr.land/Nix/Hyprland-on-NixOS/)
and [DankMaterialShell](https://github.com/AvengeMedia/DankMaterialShell/tree/v1.6.1).
