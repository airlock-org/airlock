# AMD GPU ownership investigation

Establish whether this laptop can give its internal GPU to `sys-gui` and run
a usable physical desktop through the existing Xen/Qubes architecture.
Progress is measured by evidence and completed gates, not elapsed time.
Keep the initial desktop on XFCE/Xorg; Hyprland, inference and `sys-gpu` are
outside this investigation.

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
We have not performed a reset or assigned the GPU to a guest.

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

### 4. Prepare and perform the first guest assignment

Only after the assignment/reset decision permits it, verify the lab guest's
HVM configuration, Qubes kernel, matching `amdgpu` modules and firmware.
Check that qrexec diagnostics can be reached without a display and arrange
dom0/guest log collection before starting the display manager.

Use upstream Qubes device tools. Resolve the actual device identifier from
`qvm-device pci list`; do not assume an older short BDF identifier is accepted
by the installed version. Inspect existing assignments and saved state first.
Make the smallest justified assignment and record how to undo it. Recheck
that dom0's GPU driver remains unbound.

Completion: the guest sees the intended GPU, loads its driver and exposes
DRM devices, with no unexpected device ownership or host failure. On failure,
capture the first actionable error before changing another variable.

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
