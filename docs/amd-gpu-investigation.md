# AMD GPU ownership investigation

Establish whether this laptop can give its internal GPU to `sys-gui` and run
a usable physical desktop through the existing Xen/Qubes architecture.
Progress is measured by evidence and completed gates, not elapsed time.
Keep the initial desktop on XFCE/Xorg; Hyprland, inference and `sys-gpu` are
outside this investigation.

## Current milestone — 2026-09-14

Follow-up: [GNOME](sys-gui-gnome.md) is now configured as the `usb-amd-lab`
desktop, pending build and hardware validation. [Hyprland +
DankMaterialShell](sys-gui-hyprland.md) remains available as
`usb-amd-lab-hyprland`; the XFCE baseline described below remains available as
`usb-amd-lab-xfce`.

The rebuilt USB's **usb-amd-lab with Xen** now boots into a physical sys-gui
desktop with hardware-accelerated AMD rendering. Boot ID:
`f34306b1-b7f6-483a-bfc6-7ef8cf298e1c`. Running dom0 system:
`/nix/store/ly0qakmf1zdx5aa1bfgi4g1d97k4bh8h-nixos-system-dom0-efi-raw-26.11.20260904.801bef6`.

Read-only SSH checks confirmed:

- Bootstrap and ordered GPU startup succeeded. The firmware console
  (`efi-framebuffer.0`, `vtcon1`) was released before sys-gui started.
- Dom0's `0000:04:00.0` Radeon 680M (`1002:1681`) is bound to pciback.
  Sys-gui sees it at `00:10.0`, uses amdgpu and exposes `card0`/`renderD128`.
- The guest display manager and all three input senders (keyboard, TrackPoint,
  touchpad) are active. The user reported reaching login and the desktop.
- The development dom0 terminal reached its PTY as `airlock`, UID 1000.
  Tailscale SSH to `root@dom0` works. Dom0 reported no failed systemd units.
- `glxinfo -B`, run as the desktop user on display `:0`, reported
  `direct rendering: Yes`, `Accelerated: yes`, AMD Radeon Graphics
  (`radeonsi, rembrandt, ACO`), and OpenGL 4.6 with Mesa 26.2.2.
  This verifies hardware rendering inside sys-gui's Xorg session.

Evidence: `.airlock-logs/gpu-milestone-f34306b1/verification.log`.
This successful AMD-lab boot followed an earlier default/hybrid boot on the
rebuilt USB. An independently repeated cold boot remains to be recorded; this
is not a claim of universal hardware compatibility or safe GPU reset.
The handoff log proves release, not that every bootstrap status update was
visible. Per-device input behavior and suspend/resume remain separate checks.

The next desktop experiment can trial **GNOME**, while retaining Hyprland and
XFCE as separate fallbacks. GNOME startup and displaying other Qubes VMs'
windows need their own validation; the OpenGL result does not prove those
integrations. AI inference and sys-gpu remain deferred. The sections below
preserve the earlier investigation and its evidence.

## Verified starting point — 2026-09-13

The ThinkPad T14 Gen 3 AMD has booted USB Generation 2, `usb-amd-lab` with Xen.
SSH to `airlock@airlock-usb` works. The deployed lifecycle fix is `123164a`:
libvirt uses `ON_BOOT=ignore` and `ON_SHUTDOWN=shutdown`. Its boot service
succeeded, bootstrap passed, and `sys-gui` remained stopped with HVM mode and
autostart disabled. The update was deployed over SSH without reflashing.

The Radeon 680M at `0000:04:00.0` (`1002:1681`) is bound to `pciback` and was
reported by `xl pci-assignable-list`. Dom0 no longer exposes its DRM device.
This proves reservation, not successful guest assignment or graphics.
A black screen is expected until the guest drives the panel.

Xen reported its AMD IOMMU and interrupt remapping enabled. Linux's
`/sys/kernel/iommu_groups` was empty in dom0; earlier Ubuntu inspection
reported the GPU alone in group 15. Neither observation establishes the
complete assignment/reset boundary under the running Xen configuration.

The GPU reports only `bus` in `reset_method` and `FLReset-` in its PCIe
capabilities. Bus 04 also contains audio, PSP, USB controllers and ACP.
At this baseline no manual reset or guest assignment had been performed.
Later assignment and driver results are recorded in stages 4–5 below.
pciback can attempt resets automatically during initialization; reservation
does not prove one succeeded. The completed read-only review is recorded in
[assignment/reset findings](amd-gpu-assignment-reset-findings.md).

## Investigation sequence

### 1. Capture a reproducible baseline

Save the boot ID, running system store path, repo commit, Xen/kernel versions,
PCI topology, device drivers, Xen log and bootstrap result. Map the USB root
disk, network connection and input devices to their controllers, so the
consequences of affecting a sibling device are concrete.

Keep command output and a machine-readable summary under
`.airlock-logs/usb-dev/<boot-id>/`, with a copy on the development workstation.
Record commands and exit codes; exclude authentication caches and passwords.
Do not reinstall or mount the internal SSD for these checks.

Completion: another developer can identify the exact configuration and
hardware behind every subsequent result.

Status: captured, with the assignment/reset supplement completing controller
mapping and command exit-code recording for boot ID
`3a19d380-405e-4e8e-8c3f-c2ddba13091a`. The
capture is on the USB under `.airlock-logs/usb-dev/<boot-id>/` and was copied
to the development workstation. It records USB root `/dev/sda2`, an unmounted
internal NVMe, Xen 4.19.4, the Generation 2 AMD-lab command line, the running
system path, the deployed repo commit, PCI topology, input inventory and boot
journal. The laptop-side repo predates this document; this is expected because
the baseline was captured before synchronizing the documentation commit.

### 2. Establish Xen's assignment boundary

Inspect the pinned Xen, libvirt and Qubes implementations and their upstream
documentation. Identify how this stack determines device isolation and
dependencies, and collect the relevant Xen evidence using read-only queries.
Check bridges, aliases and sibling functions rather than relying solely on
the earlier Ubuntu group number.

Audit `amd-preflight.py`: its Linux sysfs-group check and FLR-only gate are
conservative prerequisites, not a complete Xen capability assessment. Do not
turn absent evidence into a pass. If the probe changes, distinguish observed
failure, unavailable evidence and a reviewed hardware decision.

Completion: a documented assignment boundary, or a precise explanation of
which evidence is still unavailable. IOMMU enablement alone is insufficient.

Status: read-only boundary review complete; actual assignment untested.
Xen exposes only `0000:04:00.0` through
`xl pci-assignable-list`, and Qubes identifies it as
`dom0:00_08.1-00_00.0` (`sbdf: 0000:04:00.0`). The upstream Qubes device API
enumerates the GPU and every bus-04 sibling individually when queried with
`qvm-device pci list dom0 --with-sbdf`; the earlier empty frontend-oriented
query was not evidence of a broken enumeration API. The GPU and siblings sit below
the same `00:08.1` PCIe bridge, but that is topology rather than proof that
they share an IOMMU or reset boundary. Under this Xen/dom0 boot, none of
`04:00.0` through `04:00.6` expose a Linux `iommu_group` symlink, while Xen
does report AMD IOMMU and interrupt remapping enabled. Read-only
`xc_get_device_group` queries against domain 0 now report zero same-requestor
peers for each of the seven functions, with successful return codes. The
pinned AMD implementation and dom0 IOMMU setup were checked to interpret that
result. This supports a separate DMA assignment boundary, not independent
reset or complete hardware-isolation certification. The USB root disk is
behind `05:00.3`, not the GPU's bus-04 bridge. See the findings for evidence.

### 3. Determine the actual reset behavior

Trace what Qubes/libvirt/Xen and pciback would do on assignment, guest start,
failed start, guest shutdown and host shutdown for this device. Determine
which resets are attempted, their scope, and whether the stack refuses an
unsafe reset. A `bus` listing alone does not answer all of those questions.

Review applicable upstream support or quirks for this PCI ID and platform.
Assess whether a reviewed cold-boot-only ownership model could work if
reusable guest stop/start is unsupported. This is a separate outcome from
general reset-safe passthrough, not an automatic reason to disable checks.

Completion: a written decision with one of these outcomes:

- Proceed with the normal upstream reset checks and a justified reset scope.
- Investigate a specific upstream fix or a restricted ownership model, with
  its limitations and any required policy exception stated explicitly.
- Keep GPU passthrough blocked on this hardware, with the missing capability
  or evidence identified.

No reset writes, broad controller assignment, permissive mode, ACS override
or `no-strict-reset` bypass is part of the evidence-gathering phase.

Status: **read-only review complete; reset effectiveness unproven.** The
decision is to investigate a restricted, single-owner cold-boot experiment,
subject to accepting its limitations before assignment. The previous claim
that normal reset handling necessarily rejects this GPU is withdrawn.

Pinned libvirt reads the GPU's advertised PM reset capability directly and
would try D3hot-to-D0, despite Linux's broad AMD VGA quirk excluding that
method. Linux's bus-reset path is expected to reject the present siblings,
but Xen libxl does not make its reset helper's failure fatal to assignment.
pciback initialization and teardown also attempt resets without establishing
that GPU state was erased. Thus successful reservation or guest startup is
not sufficient evidence of a safe reusable lifecycle.

The [findings report](amd-gpu-assignment-reset-findings.md) records the pinned
source trace, read-only results, uncertainties and proposed experiment.
No policy bypass, reset write or guest assignment was performed in this
review. Hardware reset efficacy remains untested; neither general support
nor impossibility has been established.

### 4. Prepare and perform the first guest assignment

Only after the assignment/reset decision permits it, verify the lab guest's
HVM configuration, Qubes kernel, matching `amdgpu` modules and firmware.
Check that qrexec diagnostics can be reached without a display and arrange
dom0/guest log collection before starting the display manager.

Use upstream Qubes device tools. Resolve the actual device identifier from
`qvm-device pci list dom0 --with-sbdf`; do not assume an older short BDF identifier is accepted
by the installed version. Inspect existing assignments and saved state first.
Make the smallest justified assignment and record how to undo it. Recheck
that dom0's GPU driver remains unbound.

Completion: the guest sees the intended GPU, loads its driver and exposes
DRM devices, with no unexpected device ownership or host failure. On failure,
capture the first actionable error before changing another variable.

Status: **assignment/driver gate completed on this laptop.** The
[first assignment](amd-gpu-first-assignment.md) found missing video BIOS.
A subsequent [VBIOS experiment](amd-gpu-vbios.md), on a new host boot,
supplied only the matching firmware data through Xen ACPI delivery. The guest
loaded amdgpu, exposed card0/renderD128 and enumerated the built-in eDP panel.
Default PCI security flags were retained and siblings stayed in dom0.
This is not yet a validated desktop or restart-safe lifecycle.

### 5. Bring up the panel and input

Start the guest display manager manually. Inspect firmware loading, PCI BAR
mapping, DRM connectors/modes and the built-in eDP panel. Investigate missing
platform/firmware dependencies if the GPU appears but the panel does not.
Prove rendering uses the AMD GPU rather than a software fallback.

Package and configure the upstream Qubes input proxy for the laptop's actual
PS/2 and I2C input paths, with narrowly scoped policy. Verify keyboard,
TrackPoint/touchpad, login and a normal application in the guest desktop.

Completion: physical display and input work with the GPU owned by `sys-gui`.
Remote SSH access alone is not this milestone.

Status: **physical GUI login and input confirmed on this boot.** After the
successful VBIOS attempt, the guest display manager was started manually and
the upstream Qubes input proxy connected the dom0 keyboard, TrackPoint and
touchpad. Xorg/libinput registered all three; the user confirmed visible GUI,
typing, pointer movement and successful login. See [input findings](sys-gui-input.md)
for the reusable implementation, portable qrexec packaging fix and validation.
Accelerated user-space rendering, a normal application and detailed pointer
behavior remain unchecked. No GPU restart was needed for input deployment.

### 6. Validate lifecycle and separate general support from this machine

Verify a fresh cold boot and host shutdown from the GUI. Test guest restarts
and suspend/resume only if the reviewed reset model supports them; otherwise
record those operations as unsupported. Check persistent PCI assignments
before returning to the default hybrid profile.

Keep generic lifecycle, input-proxy and diagnostic changes in shared modules.
Keep PCI addresses, reset decisions and panel quirks in the hardware lab
configuration. Document unsupported hardware and the evidence required for
another machine. One successful laptop does not establish general AMD support.

Completion: a repeatable supported workflow, stated limitations and a clear
next step toward the general product rule: `sys-gui` owns the sole/internal
GPU; optional `sys-gpu` is for an additional GPU.

Status: one failed-initialization guest shutdown completed without losing
dom0. Fresh-power-cycle repeatability, guest restart, suspend/resume and reset
state isolation remain unvalidated. The findings do not reject the general
product rule; keep the working hybrid profile as the fallback.

Update — 2026-09-14: [automatic AMD VFCT delivery](amd-vbios-automatic.md)
now discovers the assigned GPU's firmware and maps it to an explicit guest
PCI slot through Qubes/libvirt startup. A real laptop start initialized amdgpu
at `00:10.0` using the newly generated table, without the old provisioned
VBIOS artifact. This generalizes firmware preparation; hardware reservation,
reset review, additional-machine testing, and a rebuilt-image boot check remain
separate milestones.

## Working method and recovery

Make and commit changes in the local repo, build there, and transfer only
missing store paths over SSH. Keep the writable USB repo in sync. Stage
GPU-ownership changes for the next boot; do not live-switch between the
hybrid and AMD-lab ownership configurations. Reflash only when an image or
partition-layout change actually requires it.

A separate recovery-boot rehearsal was deliberately skipped for this
experiment. A cold power cycle and manual boot selection remain the fallback.
They do not remove persisted PCI assignments: once assignment experiments
begin, inspect/unassign the experimental device in the text-recovery profile
before booting the hybrid desktop. Preserve VM disks and `qubes.xml`.

Record findings and next actions at each gate. Distinguish a confirmed defect
from an untested hypothesis; do not change several subsystems in response to
a black screen without logs locating the failure.

See [the AMD lab runbook](usb-amd-lab.md) for profile details and
[Qubes device-handling security](https://doc.qubes-os.org/en/latest/user/security-in-qubes/device-handling-security.html)
for the reset trust model to review alongside the pinned implementation.
