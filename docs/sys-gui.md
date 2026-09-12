# sys-gui desktop and login

`sys-gui` owns LightDM's login screen, the XFCE desktop, Qubes Manager,
application menus, tray widgets, notifications, and graphical settings tools.
The bootstrap and initial runtime configurations use **user / airlock** for
the guest login. The dom0 console account remains **airlock / airlock**.
These are development credentials. Deployments should configure the GUI
user through NixOS `users.users.user.hashedPasswordFile`; credentials are
independent of dom0 and are not copied out of its shadow file.

Dom0 automatically opens a greeterless relay session. It retains Xorg,
LightDM's session supervisor, and `qubes-guid` to forward the guest display.
The Qubes GUI agent starts the outer Xorg server on `:1` inside `sys-gui`;
the `qubes.GuiVMSession` RPC then starts guest LightDM on a software-cursor
Xephyr display. Guest LightDM authenticates the user and launches XFCE.
Logging out returns to the guest login screen. Disconnecting the relay
stops guest LightDM, so reconnecting starts at the greeter.

Dom0 no longer installs XFCE, its fallback xterm desktop, Qubes Manager,
the Qubes desktop-manager frontend package, or Qubes artwork in its system
profile. Xen, libvirt, qubesd, QubesDB, qrexec, and policy enforcement remain
in dom0. The desktop-linux-common package stays there because it also
contains RPC and command-line helpers used by the backend. Dependencies and
the guest closures shipped in the image can still contain GUI libraries;
this is not a claim that every GUI-related store path has been removed.

The desktop tools use the Qubes Admin API. Policy explicitly permits VM
inspection, creation of AppVM/StandaloneVM/TemplateVM domains, per-VM
settings, firewall rules, volume resizing, lifecycle actions, and removal.
Global settings writes, dom0 shell access, policy editing, host device
assignment, and backup/restore are not granted. Some installed tools or
controls will therefore report permission denied; installing a frontend
does not give it unrestricted host administration. Runtime VM edits can be
reconciled back to the declared Nix configuration by subsequent setup.

This remains a hybrid GUI domain. Removing dom0's Xorg server requires a
separate display-device and input-ownership change. The AI `sys-gpu` role
remains independent. `sys-config-*` VMs remain on demand.

For a fresh development image, build and boot with
`AIRLOCK_VM_FRESH=1 make vm-gui-config-run`. Verify guest login, visible cursor,
VM Manager, logout/relogin, and that config VMs remain halted. Guest login
diagnostics are in `journalctl -b -u display-manager` inside `sys-gui`.
