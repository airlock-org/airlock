# Investigate AMD GPU reset and lifecycle safety

Status: open investigation. Assignment, firmware delivery, a physical desktop
and input have been demonstrated on the lab laptop; effective GPU reset and
state isolation across lifecycle transitions remain unverified. Extracted from
the dated AMD investigation on 2026-09-30.

## Evidence boundary

Read the [historical investigation](../history/amd-gpu-investigation.md),
[assignment/reset source review](../history/amd-gpu-assignment-reset-findings.md)
and [first assignment](../history/amd-gpu-first-assignment.md), alongside the
later [GNOME evidence](../sys-gui-gnome.md) and
[whole-machine deployment evidence](../machine-deployment.md).

Later successful host boots do not prove effective GPU reset, erasure of prior
GPU state, suspend/resume or compatibility with another machine. The source
review found reset paths that can report or ignore success without establishing
hardware-state sanitization. Do not reopen demonstrated display/input gates
merely because the historical report predates those results.

## Outstanding checks

### Display freeze investigation, 2026-10-12

The live sys-gui uses Qubes kernel `6.12.64-1.qubes.fc41.x86_64`, Radeon 680M
`1002:1681`, DCN 3.1 and DMCUB firmware `0x0400004c`. The recorded frozen
panel logged `flip_done timed out` while GNOME and root qrexec remained alive.
After reboot, DMCUB errors recurred at 13:34:53 and 13:40:12 UTC without a new
flip timeout at inspection. The eDP-1 debugfs capability reports sink PSR
support `0x03`, driver support yes, PSR state 6 and
`disallow_edp_enter_psr=0`. Graphics/compute/DMA emitted and signalled fences
matched at inspection; this is a healthy-session sample, not a freeze trace.
PSR is therefore a concrete hypothesis, not an established cause.

Prevention trial: disable PSR alone with `amdgpu.dcdebugmask=0x10` in the
machine-specific sys-gui guest kernel options, preserving the rest of its
generated kernel command line. The upstream 6.12.64
[debug-mask definition](https://github.com/gregkh/linux/blob/v6.12.64/drivers/gpu/drm/amd/include/amd_shared.h)
defines that bit for PSR v1 and selective update. The module parameter is
read-only after load; this is not a live sysfs change. Review the deployed
Qubes kernel source/patches before implementation, and verify effective PSR
state after the new guest boot. Prefer one planned host reboot over claiming
that repeated passthrough VM restarts are safe. Measure the power impact and
ordinary window/idle/lock behavior before accepting the mitigation. Keep a
single-variable trial before considering firmware/kernel updates or other
display-feature masks.

Recovery experiment, during the next confirmed freeze only:

- Preserve bounded kernel/compositor logs, PSR information and ring fences.
  Do not glob-read debugfs: some apparently readable nodes perform mutations.
- Confirm sys-gui still owns `1002:1681` and root qrexec answers. A process
  timeout bounds our client wait; it cannot guarantee cancellation of blocked
  kernel recovery or prove hardware recovered.
- Consider one in-driver recovery request using the existing root route to
  the exact GPU's `amdgpu_gpu_recover` debugfs node. The node exists on this
  live guest. **Reading it triggers a reset**, rather than observing status.
  [Upstream documentation](https://docs.kernel.org/gpu/amdgpu/debugfs.html)
  states GPU work is lost. This does not reassign the PCI device; it is still
  an unverified hardware reset experiment and may fail.
- Record the reset result and physical panel/input response. If driver recovery
  finishes but GNOME remains unusable, attempt one bounded display-manager
  restart, requiring sign-in and application-VM GUI reconnection. This loses
  the sys-gui desktop session; do not claim all visible application state is
  preserved. Leave application VMs running where possible.
- Stop after failure. Do not escalate automatically to PCI unbind, bus reset,
  VM kill/restart or an endless watchdog loop. Earlier VM restart attempts
  have required cold power recovery after PSP initialization failure.

The upstream [PSR recovery RFC](https://mail-archive.com/amd-gfx%40lists.freedesktop.org/msg148317.html)
describes DMCUB errors and a permanently pending page flip while rendering-ring
timeouts do not trigger recovery; manual GPU recovery was insufficient on
that author's different Lenovo system. A
[Rembrandt report](https://lists.openwall.net/linux-kernel/2026/07/17/1591)
also describes this signature, but neither report proves this laptop's cause
or that their proposed patches are merged or applicable to the pinned kernel.
Do not backport an experimental recovery series as an established fix.

This investigation performed no GPU reset, PSR write, PCI change or desktop
restart. The current display remains available. The subsequent implementation
adds a laptop sys-gui PSR boot mitigation and a manual recovery tool; see
[deployment and recovery instructions](../gpu-display-recovery.md). Successful
mitigation and recovery still require separate measured evidence.

- Review the currently pinned Xen, Qubes, libvirt and kernel reset paths against
  the actual device and sibling topology. Separate predicted behavior from
  measured reset effectiveness and state isolation.
- Determine which guest shutdown/restart, reassignment and host suspend/resume
  operations are supported by that model. Define observable acceptance and an
  explicit recovery route before any disruptive physical experiment.
- Check cold-power-cycle repeatability for the same declared GPU configuration
  and firmware path, with exact host/guest generation evidence. Reuse later
  records only where they explicitly exercise the same behavior.
- Run guest restarts or suspend/resume only where the reviewed model supports
  a trial; otherwise document them as unsupported. Preserve normal upstream
  reset/policy settings. Startup success alone cannot close reset-safety checks.
- Keep machine-specific PCI selection, panel quirks and reset findings in the
  hardware configuration. Test another machine separately before making broader
  AMD compatibility claims.

## Completion and working method

Produce a supported lifecycle matrix, reproducible evidence and explicit
limitations. Preserve Xen, qrexec, QubesDB, qubesd, libvirt and Qubes policy.
Use [repository-first deployment](../machine-deployment.md), recording any
unavoidable diagnostic mutation and removing it before final verification.
Do not make resets, repeated assignments or suspend tests an automatic retry
loop in response to a black screen. GPU ownership changes require a reviewed
next-boot configuration and a usable recovery route.
