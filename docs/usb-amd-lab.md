# AMD ownership experiment

Implemented scaffolding, **not validated GPU passthrough**. The default USB
configuration still runs the hybrid XFCE desktop with AMD driven by dom0.
The lab is a separate boot specialisation, not a runtime toggle.
Its immediate purpose is to inspect Xen ownership of the laptop GPU over SSH
before attempting assignment. A usable desktop still needs safe assignment,
display initialization and input forwarding.

## Boot choices

The USB should contain three Xen boot choices, plus upstream non-Xen recovery
entries. Verify the generated entries before the first hardware experiment:

| Xen entry | Display | VM startup |
| --- | --- | --- |
| Default | Existing hybrid XFCE | sys-gui started by the desktop relay after setup |
| `usb-text-recovery` | dom0 text console, AMD not hidden | No setup or guest autostart |
| `usb-amd-lab` | May be blank; AMD reserved for Xen pciback | Lab definition applied, no guest autostart |

Both manual entries retain direct dom0 networking, SSH and Tailscale. The lab
reserves **only** `0000:04:00.0`, not audio, USB, PSP, ACP or storage functions.
It blacklists dom0 AMD drivers and disables the firmware framebuffer. The
lab is specific to the inspected ThinkPad; do not select it on other hardware.
Do not activate a specialisation with `switch-to-configuration switch`: GPU
ownership is established during boot. The upstream systemd-boot/Xen builder
keeps the normal generation as the default; do not manually make the lab entry
the saved default.

## Ordered experiment

1. Boot the default USB. Confirm the laptop's internal filesystems are not
   mounted, Xen is running, XFCE/input works and Tailscale SSH reconnects.
2. Boot **`usb-text-recovery` with Xen** from the boot menu. Verify console
   login and remote access. Return to the default entry and verify XFCE.
3. Only then select **`usb-amd-lab` with Xen** once from the boot menu.
   Expect a blank panel. Reconnect over SSH and collect evidence:

   ```sh
   mkdir -p /home/airlock/dev/nixome/.airlock-logs/usb-dev
   sudo airlock-usb-amd-preflight > /home/airlock/dev/nixome/.airlock-logs/usb-dev/amd-preflight.json
   sudo lspci -nnk -s 04:00.0
   sudo journalctl -b -k --no-pager
   ```

   The preflight is read-only and exits 2 if a prerequisite is missing. It
   checks the exact device, Xen dom0, pciback ownership, isolation membership,
   dom0 DRM bindings and a selected function-level reset method. Missing
   sysfs evidence is a failure, not an assumed success.

4. **Stop for reset review.** Ubuntu reported only `bus` for this GPU, and
   other bus 04 functions include important USB/security/audio devices. The
   gate is expected to fail until reset safety is established. There is no
   `no-strict-reset`, permissive mode, ACS override, forced unbind, reset write,
   automatic PCI assignment or reset-check bypass in these profiles/tools.

## What remains before a usable physical sys-gui

- A reviewed, hardware-specific decision on safe reset and actual PCI
  assignment through upstream Qubes tooling. This commit does not assign it.
- Confirm the shared Qubes kernel's attached modules contain `amdgpu`, firmware
  loads, PCI BARs map, and the guest exposes AMD DRM and the built-in eDP panel.
  Do not use a NixOS kernel's unrelated modules with the shared Qubes kernel.
- The lab guest is HVM, has no emulated video or outer GUI agent, and uses
  physical Xorg with Qubes' shared-memory wrapper. Its LightDM remains manual
  so remote diagnostics can run before taking over the screen. Both USB guest
  variants share declarative `user` / `password` credentials; no runtime
  password seeding is required. After safe assignment/start, explicitly start
  the guest's display-manager. Dom0 credentials still come from personalization.
- Package and wire **upstream Qubes input proxy**, including dom0's PS/2 and
  I2C inputs, with narrowly scoped receiver policy. No custom Rust/RPC input
  protocol and no broad USB controller passthrough. This is not implemented
  by the lab profile yet; remote diagnostics remain available.
- Validate display initialization, input, logout and cold-boot recovery.
  Guest stop/start/reset cycles are a separate risk review, not an automatic loop.

This follows upstream's separation of [hybrid and GPU GUI domains](https://doc.qubes-os.org/en/latest/user/advanced-topics/gui-domain.html)
and its [HVM/input-proxy configuration](https://github.com/QubesOS/qubes-mgmt-salt-dom0-virtual-machines/blob/main/qvm/sys-gui-gpu.sls).
Upstream's permissive PCI suggestion is deliberately not enabled here.

## Recovery

If the GPU or the host wedges, a cold power cycle may be necessary. Select
`usb-text-recovery` **with Xen**, not the AMD entry. The profile does not hide
the GPU and masks guest autostart even if Qubes' mutable state has stale links.
If a later manual experiment assigned devices, inspect
`qvm-pci list --assignments` and unassign only the verified experimental
`dom0:04_00.0` assignment from sys-gui using Qubes' device tools. Do not delete
VM disks or `qubes.xml`. Then reboot the default USB entry.

An empty declarative PCI list does not currently remove runtime assignments.
The former password service also checked for leftover PCI assignments; that
check was removed with the service. Until a separate guard is implemented,
unassign the experimental GPU in text recovery before returning to the default
profile, whose desktop relay starts sys-gui.
