# AMD ownership experiment

An experimental physical GPU profile, with one successful laptop desktop
login; **not general hardware or reset certification**. The default USB
configuration still runs the hybrid XFCE desktop with AMD driven by dom0.
The lab is a separate boot specialisation, not a runtime toggle.
Its current milestone is reproducible boot-time GPU assignment and automatic
firmware delivery, checked over SSH. The lab now also enables automatic
XFCE/LightDM login and selected input forwarding; fresh-boot validation of
this combined workflow is pending.

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
| `usb-amd-lab` | sys-gui physical XFCE login after startup | Validated assignment, one sys-gui start, then input forwarding |

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

After a fresh flash, configure dom0 networking and Tailscale from
`usb-text-recovery` with Xen, then reboot into the lab **without reflashing**.
The physical desktop is in sys-gui, not dom0: its terminal cannot configure
dom0's network or enroll dom0 in Tailscale. Guest login is `user` / `password`.
The screen can remain blank during bootstrap; a continuing blank screen needs
SSH/log inspection, not an assumption that the computer is frozen.

Implementation checks (2026-09-14): 32 focused pytest checks passed, one optional
SSH check skipped. The generated guest display-manager and dom0 startup units
built successfully. Evidence is under `.airlock-logs/amd-lab-desktop/`.
This does not yet establish that the combined workflow works on a fresh boot.

## Historical reservation experiment

The following sequence documents the earlier manual investigation. Current
images assign and start the reviewed GPU on lab boot; use the
[fresh-flash acceptance checklist](gui-gpu-assignment.md) for the next test.

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

- Fresh-flash verification of the reviewed selection's declarative assignment
  and startup. Reset effectiveness remains a separate open question.
- Confirm the shared Qubes kernel's attached modules contain `amdgpu`, firmware
  loads, PCI BARs map, and the guest exposes AMD DRM and the built-in eDP panel.
  Do not use a NixOS kernel's unrelated modules with the shared Qubes kernel.
- The lab guest is HVM, has no emulated video or outer GUI agent, and uses
  physical Xorg with Qubes' shared-memory wrapper. The physical lab now boots
  to `graphical.target` via `airlock.sysGuiGuest.physicalDesktopOnBoot = true`.
  Setting that option false retains text-only guest diagnostics.
  Both USB guest
  variants share declarative `user` / `password` credentials; no runtime
  password seeding is required. LightDM starts automatically inside the guest.
  Dom0 credentials still come from personalization.
- **Upstream Qubes input proxy** is now packaged and wired, including dom0's
  selected PS/2 and I2C inputs. The shared module is opt-in; the machine
  hardware file supplies stable paths. The lab starts its target from the
  successful VM startup service's `ExecStartPost`, never before `qvm-start`
  returns. Individual senders remain non-restarting and release their grabs
  when the connection ends; see [the input runbook](sys-gui-input.md).
- Validate display initialization, input, logout and cold-boot recovery.
  Guest stop/start/reset cycles are a separate risk review, not an automatic loop.

## Automatic VBIOS workflow

The lab enables `airlock.amdVbios.domains.sys-gui` in Nix. Bootstrap now applies
the machine's declared required GPU assignment, and the ordered start service
runs ordinary `qvm-start sys-gui` once. The extension selects the matching
host VFCT entry, validates it, and generates a table for the explicit guest
PCI slot. No `airlock-amd-vfct` feature or provisioned firmware file is used.
The GPU must be the sole required PCI assignment and already reserved to
pciback. Failures are reported through Qubes and the dom0 journal.

GPU selection and isolation/reset review remain separate from firmware delivery.
See [declarative assignment](gui-gpu-assignment.md) for configuration, safety
boundaries and validation. Keep the persistent assignment for the reboot test;
the earlier unassign-after-start procedure was for manual experiments only.

This follows upstream's separation of [hybrid and GPU GUI domains](https://doc.qubes-os.org/en/latest/user/advanced-topics/gui-domain.html)
and its [HVM/input-proxy configuration](https://github.com/QubesOS/qubes-mgmt-salt-dom0-virtual-machines/blob/main/qvm/sys-gui-gpu.sls).
Upstream's permissive PCI suggestion is deliberately not enabled here.

## Recovery

If the GPU or the host wedges, a cold power cycle may be necessary. Select
`usb-text-recovery` **with Xen**, not the AMD entry. The profile does not hide
the GPU and masks guest autostart even if Qubes' mutable state has stale links.
To return to the hybrid profile after a physical-GPU boot, inspect
`qvm-pci list --assignments` and unassign only the verified experimental
`dom0:00_08.1-00_00.0` assignment from sys-gui using Qubes' device tools. Do not delete
VM disks or `qubes.xml`. Then reboot the default USB entry.

An empty declarative PCI list does not currently remove runtime assignments.
The GPU-selection guard now stops hybrid bootstrap if sys-gui still has PCI
state from the lab. Unassign the verified device while halted in text recovery
before returning to the default profile, whose desktop relay starts sys-gui.
