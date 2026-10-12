# Validate physical hardware for sys-gpu

Status: deferred experiment; physical PCI assignment and GPU rendering are
unverified. Extracted from the sys-gpu experiment notes on 2026-09-30.

## Baseline and scope

The [existing diagnostic image](../experiments/sys-gpu.md) provides an on-demand
NixOS guest and JSON probes without physical PCI assignments. Its nested
lifecycle checks do not establish physical GPU rendering or reset safety.

The intended development-host split is AMD integrated graphics for `sys-gui`
and the additional RTX 3090 for `sys-gpu`. Keep the desktop GPU with `sys-gui`;
`sys-gpu` must not borrow it when its own device is absent. Preserve Xen, qrexec,
QubesDB, qubesd, libvirt and Qubes policy.

## Trial and acceptance

1. Review IOMMU boundaries, companion functions and reset behavior on the actual
   Xen machine. Check guest kernel/module compatibility before assignment;
   installing a NixOS NVIDIA package alone does not satisfy the shared Qubes
   kernel's module requirements.
2. Commit the machine configuration and use the
   [repository deployment workflow](../machine-deployment.md). Validate boot
   reservation, guest driver binding and GPU API initialization while keeping
   the AMD display path working. Record the exact hardware and recovery route.
3. Render and read back a known image. Verify a hardware renderer; CPU rendering
   and a successful diagnostic collection do not meet this criterion.
4. Review and test guest restarts only when the reset model supports them.
   Record repeatability and unsupported lifecycle operations explicitly.
5. Add inference and model storage only after hardware ownership and rendering
   pass. Define its own service and resource acceptance before implementation.

Cross-VM graphics remains optional research, outside these
acceptance criteria. The earlier notes identify
[Qubes' Rust virtual GPU backend](https://github.com/QubesOS/qubes-virtgpu-host)
as an upstream research starting point; that observation is dated and does
not establish support for the intended deployment.

Keep machine-readable evidence under `.airlock-logs/`. Report nested tests,
builds, physical deployment and operator acceptance separately.
