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
