# GNOME sys-gui milestone

Status: configured, pending its first physical AMD-lab boot.

`usb-amd-lab with Xen` is the primary physical-GPU sys-gui experiment. It uses
GNOME and installs Dash to Dock from the pinned Nixpkgs package set. Airlock
enables Dash to Dock declaratively through the system dconf database, so the
dock is present on first login. GNOME users may change the extension's own
preferences; only GNOME Shell's enabled-extension list is locked to keep it
active.

Hyprland + DankMaterialShell remains available at
`usb-amd-lab-hyprland with Xen`; the known-good XFCE fallback is
`usb-amd-lab-xfce with Xen`. The three choices share the same GPU assignment,
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

On the first GNOME boot, verify the login, dock, local terminal, display
rendering, mouse/keyboard forwarding, and clean logout. Hardware-accelerated
OpenGL is already verified for the shared XFCE AMD-lab path, but that is not
yet evidence of a working GNOME session or Qubes cross-VM windows.
