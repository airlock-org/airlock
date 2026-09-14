# AMD ownership experiment

An experimental physical GPU profile, with one successful laptop desktop
login; **not general hardware or reset certification**. The default USB
configuration still runs the hybrid XFCE desktop with AMD driven by dom0.
The lab is a separate boot specialisation, not a runtime toggle.
Its immediate purpose is to inspect Xen ownership of the laptop GPU over SSH
before attempting assignment. A usable desktop still needs safe assignment,
display initialization and input forwarding.

The current evidence, investigation gates and completion criteria are in
[the AMD GPU investigation plan](amd-gpu-investigation.md). Generation 2 has
now booted successfully over SSH with the GPU reserved and bootstrap passed.
The [first assignment experiment](amd-gpu-first-assignment.md) found a missing
video BIOS. The [VBIOS follow-up](amd-gpu-vbios.md) resolved that: amdgpu now
initializes inside sys-gui, exposes DRM devices and sees the connected eDP
panel. A subsequent [input experiment](sys-gui-input.md) connected the physical
keyboard and pointers, and the user logged into the visible desktop. The lab now
enables [automatic firmware preparation](amd-vbios-automatic.md) at guest startup.
The reusable implementation discovers the assigned GPU; only the lab's boot-time
GPU reservation and input-device selection remain machine-specific.
The [assignment/reset review](amd-gpu-assignment-reset-findings.md) finds no
shared Xen DMA-requestor group for the GPU, but reset effectiveness remains
unproven. The conservative preflight failure is not a proof that assignment
is impossible, and successful guest startup would not prove safe reset.

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
Airlock dom0 configures libvirt to shut guests down instead of saving their
memory at host shutdown, and to ignore previously running guests at boot.
Qubes controls their startup. Older USB builds used libvirt's suspend/resume
defaults, which could restore the hybrid guest while lab setup replaced its
configuration. Existing saved memory must be inspected and discarded for the
affected, stopped guest before its next cold start; this does not remove its
virtual disks.
Do not activate a specialisation with `switch-to-configuration switch`: GPU
ownership is established during boot. The upstream systemd-boot/Xen builder
keeps the normal generation as the default; do not manually make the lab entry
the saved default.

## Ordered experiment

1. Boot the default USB. Confirm the laptop's internal filesystems are not
   mounted, Xen is running, XFCE/input works and Tailscale SSH reconnects.
2. An optional recovery rehearsal is to boot **`usb-text-recovery` with Xen**,
   verify console login and remote access, then return to the default entry.
   This rehearsal was skipped for the current experiment; see the plan's
   recovery notes before any persistent device assignment.
3. Select **`usb-amd-lab` with Xen** once from the boot menu.
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
   current probe is expected to fail on this machine; a reviewed reset
   decision does not automatically change its Linux-sysfs/FLR-only checks.
   See the assignment/reset findings before any guest start. There is no
   `no-strict-reset`, permissive mode, ACS override, forced unbind, reset write,
   automatic PCI assignment or reset-check bypass in these profiles/tools.
   The underlying pciback/libvirt/Xen lifecycle can still attempt resets
   automatically; absence of explicit reset commands is not a no-reset mode.

## What remains before a usable physical sys-gui

- A reviewed, hardware-specific decision on safe reset and actual PCI
  assignment through upstream Qubes tooling. This commit does not assign it.
- Confirm the shared Qubes kernel's attached modules contain `amdgpu`, firmware
  loads, PCI BARs map, and the guest exposes AMD DRM and the built-in eDP panel.
  Do not use a NixOS kernel's unrelated modules with the shared Qubes kernel.
- The lab guest is HVM, has no emulated video or outer GUI agent, and uses
  physical Xorg with Qubes' shared-memory wrapper. The physical lab now boots
  to `multi-user.target` so LightDM remains manual; the already-built guest
  used the equivalent kernel argument for the successful VBIOS experiment.
  Both USB guest
  variants share declarative `user` / `password` credentials; no runtime
  password seeding is required. After safe assignment/start, explicitly start
  the guest's display-manager. Dom0 credentials still come from personalization.
- **Upstream Qubes input proxy** is now packaged and wired, including dom0's
  selected PS/2 and I2C inputs. The shared module is opt-in; the machine
  hardware file supplies stable paths. Activate it manually after the guest
  is ready, following [the input runbook](sys-gui-input.md). It was exercised
  live on the existing guest; the updated full image has not been rebuilt.
- Validate display initialization, input, logout and cold-boot recovery.
  Guest stop/start/reset cycles are a separate risk review, not an automatic loop.

## Automatic VBIOS workflow

The lab enables `airlock.amdVbios.domains.sys-gui` in Nix. After bootstrap has
finished, assign the reviewed GPU using its currently enumerated Qubes port
and run the ordinary `qvm-start sys-gui`. The extension selects the matching
host VFCT entry, validates it, and generates a table for the explicit guest
PCI slot. No `airlock-amd-vfct` feature or provisioned firmware file is used.
The GPU must be the sole required PCI assignment and already reserved to
pciback. Failures are reported through Qubes and the dom0 journal.

GPU selection, isolation/reset review, and assigning devices remain separate
from firmware delivery. The lab does not automatically assign or start the GPU.
For a single controlled start, remove the persistent assignment after startup
using `qvm-device pci unassign sys-gui <port>`; this leaves the running device
attached. Inspect assignments with `qvm-device pci list sys-gui --assignments`.

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
