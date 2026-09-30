# AMD ownership experiment

Status: **historical experiment and evidence**, not current boot/recovery
instructions. References below to experimental menu entries, direct dom0
networking, passwords, and pending desktop validation belong to those trials.
Use [USB development and recovery](../usb-dev.md) and [USB lab](../usb-lab.md) for
current operations. [GNOME was subsequently verified](../sys-gui-gnome.md); GPU
reset effectiveness and broader hardware compatibility remain unproven.

This records the earlier standalone GPU experiment. The current USB image ships
the normal GNOME Airlock configuration and text recovery only; standalone and
hybrid GUI builders remain in source but their closures are no longer embedded.
References below to hybrid defaults and XFCE validation describe that earlier work.

An experimental physical GPU profile, with a successful automatic laptop desktop
boot; **not general hardware or reset certification**. The default USB
configuration still runs the hybrid XFCE desktop with AMD driven by dom0.
The historical lab name remains an internal Nix specialisation/profile name;
it is not a user-facing boot-menu entry or a runtime toggle.
Its current milestone is reproducible boot-time GPU assignment and automatic
firmware delivery, checked over SSH. The lab now also enables automatic
XFCE/LightDM login and selected input forwarding. Boot
`f34306b1-b7f6-483a-bfc6-7ef8cf298e1c` now confirms successful automatic startup,
framebuffer release, a working dom0 terminal, and hardware-accelerated AMD
OpenGL in sys-gui. See [the current milestone](amd-gpu-investigation.md#current-milestone--2026-09-14).
An independently repeated cold boot and further desktop integration remain.

The current evidence, investigation gates and completion criteria are in
[the AMD GPU investigation plan](amd-gpu-investigation.md). Generation 2 has
now booted successfully over SSH with the GPU reserved and bootstrap passed.
The [first assignment experiment](amd-gpu-first-assignment.md) found a missing
video BIOS. The [VBIOS follow-up](amd-gpu-vbios.md) resolved that: amdgpu now
initializes inside sys-gui, exposes DRM devices and sees the connected eDP
panel. A subsequent [input experiment](../sys-gui-input.md) connected the physical
keyboard and pointers, and the user logged into the visible desktop. The lab now
enables [automatic firmware preparation](../amd-vbios-automatic.md) at guest startup.
The reusable implementation discovers the assigned GPU; only the lab's boot-time
GPU reservation and input-device selection remain machine-specific.
The [assignment/reset review](amd-gpu-assignment-reset-findings.md) finds no
shared Xen DMA-requestor group for the GPU, but reset effectiveness remains
unproven. The conservative preflight failure is not a proof that assignment
is impossible, and successful guest startup would not prove safe reset.

## Boot choices

The user-facing systemd-boot menu is intentionally limited to:

| Entry | Behavior |
| --- | --- |
| `Airlock` | Latest normal Xen generation (internal profile: `usb-lab`); normal sys-gui, sys-dev and desktop-VM startup |
| `Airlock (recovery)` | Xen text console; no topology setup, guest startup, GPU handoff or automatic networking |
| `Airlock (generation N)` | A retained older normal Xen generation |

`Airlock` is always the default. The configured five-generation limit is retained;
only the newest recovery entry is shown. The old hybrid, GUI-only, Hyprland and
XFCE builders remain in the source tree but do not clutter the boot menu.

At the time of this experiment, GNOME and Hyprland were not hardware-verified.
The successful desktop evidence above is for XFCE; later GNOME evidence is
recorded separately in [the GNOME milestone](../sys-gui-gnome.md). See [the Hyprland milestone](../experiments/sys-gui-hyprland.md) for its
build, boot, shortcuts and diagnostic commands.

All manual entries retain direct dom0 networking, SSH and Tailscale. The lab
reserves **only** `0000:04:00.0`, not audio, USB, PSP, ACP or storage functions.
It blacklists dom0 AMD drivers but temporarily permits the firmware framebuffer. The
lab is specific to the inspected ThinkPad; do not select it on other hardware.
Airlock dom0 configures libvirt to shut guests down instead of saving their
memory at host shutdown, and to ignore previously running guests at boot.
Qubes controls their startup. Older USB builds used libvirt's suspend/resume
defaults, which could restore the hybrid guest while lab setup replaced its
configuration. Existing saved memory must be inspected and discarded for the
affected, stopped guest before its next cold start; this does not remove its
virtual disks.
Do not activate a specialisation with `switch-to-configuration switch`: GPU
ownership is established during boot. The Airlock menu normalizer makes the
latest normal Xen generation (internally `usb-lab`) the saved `Airlock` default.

After a fresh flash, log in with `user` / `password` and open **dom0 Terminal
(Development)** from the applications menu to configure dom0 networking and
Tailscale. Follow [the USB setup commands and security caveat](../usb-dev.md#first-boot).
The ordinary terminal remains a sys-gui shell. If desktop/input fails, select
`Airlock (recovery)`, log in as `airlock`, and run
`sudo airlock-recovery-network`; then reboot into `Airlock` **without reflashing**.
The lab now attempts to show bootstrap progress on dom0's firmware console.
There is still a display gap after framebuffer release while the guest starts.
If firmware-console initialization fails, it may remain blank throughout;
use SSH/log inspection rather than assuming the computer is frozen.

Implementation checks (2026-09-14): 32 focused pytest checks passed, one optional
SSH check skipped. The generated guest display-manager and dom0 startup units
built successfully. Evidence is under `.airlock-logs/amd-lab-desktop/`.
This does not yet establish that the combined workflow works on a fresh boot.

## Temporary bootstrap framebuffer

The existing AMD lab entry enables `airlock.guiGpu.bootFramebuffer.enable`.
This is an experimental boot-time exception to exclusive sys-gui display
access, not permission to run AMD's driver or a desktop in dom0. Other profiles
are unchanged. The generic option defaults off and can be disabled to restore
the previous headless handoff.

The entry allows Linux's EFI/simple firmware framebuffer, includes simpledrm
in the initrd's available modules, requests immediate fbcon takeover and
selects tty1 for the existing once-per-second bootstrap renderer. The tty1
getty is disabled to avoid competing with that renderer; other gettys remain.
Xen uses `vga=current` **without `keep`**, allowing its console to relinquish
control to dom0. See [Xen's console options](https://xenbits.xen.org/docs/4.19-testing/misc/xen-command-line.html#vga).

After bootstrap finishes and the GPU/assignment guard passes, the startup
service runs `boot-framebuffer.py` before `qvm-start`. It requires the selected
GPU to be the sole PCI display controller, accepts only EFI/simple platform
framebuffers, prevents subsequent driver probing, detaches fbcon, then unbinds
the firmware platform driver. It verifies that no framebuffer remains before
allowing guest startup. This follows the kernel's
[fbcon detachment ordering](https://docs.kernel.org/fb/fbcon.html#unloading-the-drivers).
It does not write PCI unbind/reset interfaces or restore VGA modes. Ambiguous
hardware or failed detachment stops startup with a journal error. Missing
firmware display is reported but does not prevent an otherwise clean start.

The result is printed to the journal and saved as
`/var/log/airlock/boot-framebuffer-handoff.json`. The override lasts for this
boot; there is no automatic reattachment on guest failure or shutdown. There
is no physical GPU handoff test in an ordinary emulator: fixture tests and
initrd/unit builds do not replace the next laptop boot test. Check for visible
bootstrap progress, then successful GUI login/input, and a `released` result.

Implementation checks (2026-09-14): 49 focused pytest checks passed, one
optional SSH check skipped; the updated initrd and GPU startup unit built.
Fixtures cover both firmware driver types, mismatched/multiple GPUs, unknown
framebuffers, failed console/driver detach, repeated handoff and inhibition of
late driver probing. Evidence is under `.airlock-logs/boot-framebuffer/`.
Hardware display and handoff remain unverified for this change.

### Firmware device hierarchy fix — 2026-09-14

USB boot `10919aa911814072be5c465d8ab4d42f` completed bootstrap in about five
minutes and passed the persistent GPU assignment check. At 16:30 BST the
handoff guard rejected `efi-framebuffer.0` before any framebuffer writes or
`qvm-start`. The kernel had initialized efifb at `0x8b0000000`, matching the
selected GPU's BAR 0. This was a startup failure, not a VM initialization hang.

The guard incorrectly required every platform device's physical sysfs path
to be under `/sys/devices/platform`. Platform devices can instead be nested
under a PCI parent. It now validates the platform bus's device entry and the
device's `subsystem` backlink, retaining firmware-driver allowlisting and
checking the driver's registration backlink. No GPU/reset checks are removed.

Regression fixtures cover both EFI/simple framebuffers under platform-root
and PCI-parent layouts, using a GPU address different from the laptop's.
The old code fails both PCI-parent release tests; the corrected code passes.
Verification: 75 checks passed, five opt-in checks skipped; startup unit built.
Evidence is under `.airlock-logs/boot-framebuffer-layout-fix/`, with original
USB logs under `.airlock-logs/usb-boot-10919aa9/`. The USB was not modified;
fresh-image laptop handoff remains to be verified.

## Historical reservation experiment

The following sequence documents the earlier manual investigation. Current
images assign and start the reviewed GPU on lab boot; use the
[fresh-flash acceptance checklist](../gui-gpu-assignment.md) for the next test.

1. Boot the default USB. Confirm the laptop's internal filesystems are not
   mounted, Xen is running, XFCE/input works and Tailscale SSH reconnects.
2. An optional recovery rehearsal is to boot **`Airlock (recovery)`**,
   verify console login and remote access, then return to the default entry.
   This rehearsal was skipped for the current experiment; see the plan's
   recovery notes before any persistent device assignment.
3. Older images exposed **`usb-gui-lab` with Xen** directly. Current images
   retain that builder in source but hide it from the concise boot menu.
   Expect a blank panel. Reconnect over SSH and collect evidence:

   ```sh
   mkdir -p /home/user/dev/nixome/.airlock-logs/usb-dev
   sudo airlock-usb-amd-preflight > /home/user/dev/nixome/.airlock-logs/usb-dev/amd-preflight.json
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
  when the connection ends; see [the input runbook](../sys-gui-input.md).
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
See [declarative assignment](../gui-gpu-assignment.md) for configuration, safety
boundaries and validation. Keep the persistent assignment for the reboot test;
the earlier unassign-after-start procedure was for manual experiments only.

This follows upstream's separation of [hybrid and GPU GUI domains](https://doc.qubes-os.org/en/latest/user/advanced-topics/gui-domain.html)
and its [HVM/input-proxy configuration](https://github.com/QubesOS/qubes-mgmt-salt-dom0-virtual-machines/blob/main/qvm/sys-gui-gpu.sls).
Upstream's permissive PCI suggestion is deliberately not enabled here.

## Recovery

If the GPU or the host wedges, a cold power cycle may be necessary. Select
`Airlock (recovery)`. This is the Xen text-recovery profile; it does not hide
the GPU and masks guest autostart even if Qubes' mutable state has stale links.
To return to the hybrid profile after a physical-GPU boot, inspect
`qvm-pci list --assignments` and unassign only the verified experimental
`dom0:00_08.1-00_00.0` assignment from sys-gui using Qubes' device tools. Do not delete
VM disks or `qubes.xml`. Then reboot the default USB entry.

An empty declarative PCI list does not currently remove runtime assignments.
The GPU-selection guard now stops hybrid bootstrap if sys-gui still has PCI
state from the lab. Unassign the verified device while halted in text recovery
before returning to the default profile, whose desktop relay starts sys-gui.
