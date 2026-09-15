# sys-gui desktop and login

`sys-gui` owns LightDM's login screen, desktop session, Qubes Manager,
application menus, tray widgets, notifications, and graphical settings tools.
The offline `usr-home` desktop-work VM owns the Files and Terminal processes
and the user's persistent home directory; their windows render in `sys-gui`.
Its GNOME application catalog also includes local document, image, media,
calendar, contact, editor, utility and help applications. `sys-gui` exposes
each through a fixed launcher; it does not install GNOME's broad core app set.
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
remains independent. `sys-config-*` VMs remain on demand in the full profile.

For a fresh full development image, build and boot with
`AIRLOCK_VM_FRESH=1 make vm-gui-config-run`. Verify guest login, visible cursor,
VM Manager, logout/relogin, and that config VMs remain halted. Guest login
diagnostics are in `journalctl -b -u display-manager` inside `sys-gui`.

## Default ownership direction

`sys-gui` is the intended standard desktop domain. The GPU needed for the
desktop belongs to it, including the only GPU on a single-GPU machine.
`sys-gpu` is an optional, hardware-specific addition for spare internal or
external GPUs, never a dependency of the desktop. For this machine the target
is AMD integrated graphics in `sys-gui`, with the RTX 3090 reserved for a later
custom `sys-gpu` deployment.

## Minimal nested image

`nixosConfigurations.vm-gui-minimal` declares only `sys-gui` alongside dom0.
It reuses Xen, qubesd, QubesDB, qrexec, libvirt, the Qubes GUI transport and the
existing Nix-native guest builder. The full image profiles remain unchanged.

- XFCE and the current hybrid display/login path are retained. Dom0 still
  hosts the outer display; this image does **not** pass through the AMD GPU.
- The sole guest has no NetVM and no PCI assignments.
- Network, configuration, application and `sys-gpu` VM images are not selected
  as dependencies of this image. Their flake outputs remain available.
- Files/Firefox launchers, their panel bootstrap and their MIME defaults are
  omitted because `usr-home` and `usr-browser` do not exist here. The normal
  GUI guest keeps those launchers.
- A single boot setup unit applies and records the entire small topology;
  no deferred provisioning, network-provider startup or config workflow runs.
- The full development toolset and the proxy targeting `sys-firewall` are
  disabled. Existing Qubes administration tools/policy are retained.

The topology is defined in `lifecycle/bootstrap/gui-minimal.nix`; the nested
harness settings are in `lifecycle/image/profiles/vm-gui-minimal.nix`.

## Build and launch when ready

```sh
make vm-gui-minimal-image
AIRLOCK_VM_DISPLAY=gtk make vm-gui-minimal-run
```

The launcher uses its own `vm-gui-minimal` directory under the Airlock data
root, loopback SSH port 2332 and 8192 MiB RAM by default. Override
`VM_GUI_MINIMAL_DIR`, `VM_GUI_MINIMAL_SSH_PORT` or `AIRLOCK_VM_MEMORY` as needed.
It does not reuse the normal development VM disk. The minimal launcher sets
`AIRLOCK_VM_SYSNET_NIC=0`, omitting the extra NIC used by the full networked
profile. Other launch targets keep that NIC enabled by default.

In another terminal:

```sh
make vm-gui-minimal-ssh
sudo systemctl status airlock-setup-config
sudo qvm-ls
```

This is a disposable nested test image, not a release for physical machines:
dom0 retains DHCP/SSH for the test harness, the `airlock` account uses password
`airlock` and passwordless sudo, and the `sys-gui` login is `user` with password
`airlock`. Do not deploy these credentials or network settings on hardware.

Pytest configuration coverage is in `tests/test_gui_minimal.py` and included
in `make test-quick`. No tests, builds or VM boots were run for this change,
as requested. Physical display ownership and boot behavior remain unverified.

## Next hardware milestone

Keep the hybrid image as a development/recovery path. A separate, explicit
AMD direct-display configuration still needs:

1. GPU/IOMMU and reset checks, companion-function selection and boot-time
   reservation so dom0 does not load the GPU's normal display driver.
2. A compatible guest kernel, `amdgpu`, firmware and Mesa. Guests currently
   boot a shared Qubes kernel; a NixOS graphics option alone does not prove
   driver compatibility.
3. A direct display-manager path in `sys-gui`, independent of the existing
   dom0 GUI session relay, plus keyboard/mouse delivery at login.
4. A tested recovery boot entry before handing over the only display GPU.
5. XFCE login, locking, monitor modes and reboot validation on hardware.

Hyprland follows that milestone, with separate Qubes window/input integration
testing. No GPU discovery or automatic device reassignment is introduced here.

The opt-in physical laptop experiment now has a confirmed GUI login with
keyboard and pointer input. Its [reusable input module and findings](sys-gui-input.md)
are separate from machine-specific GPU reservation and firmware delivery.
This does not change the default hybrid profile or establish reset safety.
