# Validate the physical Hyprland desktop experiment

Status: deferred. Image configuration exists; no physical Hyprland boot or
Qubes cross-VM window transport is verified. Extracted on 2026-09-30.

## Prerequisites and trial

Read the [implementation and historical procedure](../experiments/sys-gui-hyprland.md).
Normal USB images ship GNOME and text recovery; the historical Hyprland/XFCE
menu choices are absent. Declare an explicit experimental image configuration
before building a trial. Preserve a verified desktop and recovery route.

Use [repository-first deployment](../machine-deployment.md); commit source
before building or deploying. Review GPU ownership and reset boundaries through
the [AMD lifecycle investigation](amd-gpu-lifecycle.md). Earlier working AMD
Xorg/OpenGL results do not verify Wayland or Qubes window transport.

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
## Acceptance

- Verify the physical display and hardware acceleration, keyboard and pointer,
  panel/launcher, native terminal and development dom0 terminal.
- Verify logout and return to the desktop; record cold-boot and recovery results.
- Verify Qubes cross-VM windows, trusted VM labels, focus and clipboard isolation
  before considering Hyprland as a replacement desktop. The existing Xorg
  `shmoverride` integration is not applied to Hyprland or Xwayland; enabled local
  Xwayland applications do not prove Qubes transport works.
- Record shell failure behavior and diagnostics. The native terminal and logout
  binding should permit inspection even if DMS fails.

Host suspend/reboot, audio, network integration and lock-screen behavior need
separate acceptance before being advertised as supported. This experiment does
not depend on the additional GPU or inference work in
[sys-gpu](sys-gpu-hardware.md).

Keep dated evidence and machine-readable results under `.airlock-logs/`.
Distinguish a successful build, booted generation, working desktop and accepted
cross-VM integration.
