# Laptop GPU assignment/reset findings

Subsequent hardware result: [first assignment experiment](amd-gpu-first-assignment.md).
The account below preserves the preceding read-only review and its limits.

Evidence review: 2026-09-13. Scope: the ThinkPad T14 Gen 3 AMD's Radeon 680M,
reserved for Xen in USB Generation 2 `usb-amd-lab`. No guest assignment,
manual reset, device unbind, policy bypass or production configuration change
was performed during this review. Automatic pciback reset attempts at boot
are a separate matter, described below.

## Decision

**Assignment is not ruled out; a reliable, independently resettable GPU
lifecycle is not established.** The earlier investigation decision that
normal Qubes handling necessarily rejects this laptop, and therefore the
iGPU must remain in dom0, was premature and is superseded by this report.

The read-only assignment/reset review is complete. This is not a successful
passthrough or reset test. There are three distinct results:

- Xen reports no other dom0 device sharing the GPU's AMD DMA-requestor group.
  This supports investigating assignment of the GPU function alone.
- The installed libvirt would select a PM reset that Linux deliberately
  excludes for this class of AMD device. Successful VM startup would not
  establish that the GPU was actually reset or its previous state erased.
- The kernel's available bus-reset path is expected to refuse with the
  present siblings. Xen's userspace caller does not necessarily propagate
  that refusal as an assignment failure.

Do not change the general product objective: internal/sole GPU to `sys-gui`,
optional additional GPU to `sys-gpu`. Nor should this laptop be advertised as
supporting that objective yet.

## Captured machine and evidence

- Boot ID: `3a19d380-405e-4e8e-8c3f-c2ddba13091a`.
- Running system:
  `/nix/store/1d3dx84q49ffvln8cg21y9gfgjv05nh3-nixos-system-dom0-efi-raw-26.11.20260904.801bef6`.
- Xen 4.19.4, Qubes Xen patchset `v4.19.4-7`; dom0 Linux 6.18.49;
  Qubes libvirt `10.5.0-2`. Only Domain-0 was running throughout the queries.
- Xen reports AMD IOMMU and interrupt remapping enabled, with a PV dom0.
- GPU `0000:04:00.0`, vendor/device `1002:1681`, driver `pciback`;
  the sole entry in `xl pci-assignable-list`. Dom0 has no AMD DRM device.
- Qubes identifies it as `dom0:00_08.1-00_00.0` with SBDF `0000:04:00.0`.
  Use `qvm-device pci list dom0 --with-sbdf` to resolve identifiers, not an
  assumed legacy BDF spelling. An empty earlier frontend-oriented query
  was not evidence of a broken PCI enumeration API.

Raw capture on the laptop:
`/home/airlock/dev/nixome/.airlock-logs/usb-dev/3a19d380-405e-4e8e-8c3f-c2ddba13091a/assignment-reset.json`.
Workstation copy: `.airlock-logs/amd-assignment-reset.json`, SHA-256
`4b7647df6a7f9c56e78a86e88e353c50caa658990022d7fa04cbf592f76ef24f`.
These local logs are not committed. This document preserves the decision-
relevant results. The capture includes commands and exit codes for Xen logs,
domain/assignable lists, PCI inventory/capabilities, mounts, routes and setup
journal; all returned zero. Supplementary read-only sysfs/`setpci` inspection
at 15:15 UTC on the same boot confirmed the slot and register values below.

### Controller boundaries

| Function/controller | Observed use | Relationship to GPU bus reset |
| --- | --- | --- |
| `04:00.0` | Radeon 680M, pciback | Target function |
| `04:00.1`, `.5`, `.6` | HDA / ACP / HDA, dom0 drivers | Same bus |
| `04:00.2` | PSP/CCP, `ccp` | Same bus; do not hand to guest |
| `04:00.3`, `.4` | USB, `xhci_hcd` | Same bus; keep in dom0 |
| `05:00.3`, behind `00:08.3` | USB containing root `/dev/sda2` | Different bus |
| `01:00.0`, `02:00.0` | Ethernet / Wi-Fi | Different buses |
| `03:00.0` | Internal NVMe, unmounted | Different bus |
| Platform i8042 / AMDI0010 I2C | Keyboard / pointing-device paths | Not the GPU PCI function |

The GPU's bridge is `00:08.1`, secondary/subordinate bus `04`. A secondary
bus reset would affect its siblings, but the USB boot disk is **not** behind
that bridge. This topology does not guarantee survival of a broader platform
failure, and does not justify deliberately resetting the whole bus.

## Xen assignment boundary

The read-only diagnostic used the installed `libxenctrl.so.4.19` and
`xc_get_device_group(handle, 0, sbdf, 65536, &count, peers)` for every
`04:00.0`–`04:00.6`. All seven returned `rc=0`, `errno=0`, `count=0`.
Input SBDF is `(segment << 16) | (bus << 8) | (device << 3) | function`;
here `0x400`–`0x406`. The call returns *other* devices, excluding the queried
function. The diagnostic did not call `xc_assign_device`,
`xc_test_assign_device`, or any reset API.

In the pinned Xen implementation, AMD groups by the DMA requestor ID derived
from IVRS mappings. The generic query scans devices owned by the queried
domain. Dom0 is created with its IOMMU flag when the global IOMMU is enabled,
and the AMD group callback is implemented; the observed boot satisfies these
conditions. An empty result must not generally be interpreted without those
checks, because disabled IOMMU/missing callback also returns zero.
See pinned Xen [group query](https://github.com/xen-project/xen/blob/c2ece6c994a236e9ba51c9ec99085ae99347d552/xen/drivers/passthrough/pci.c#L1614),
[AMD grouping](https://github.com/xen-project/xen/blob/c2ece6c994a236e9ba51c9ec99085ae99347d552/xen/drivers/passthrough/amd/pci_amd_iommu.c#L701),
and [dom0 creation](https://github.com/xen-project/xen/blob/c2ece6c994a236e9ba51c9ec99085ae99347d552/xen/arch/x86/setup.c#L918).

This is evidence against a shared DMA-requestor dependency, not proof of a
successful assignment hypercall, complete firmware/ACS correctness, or
independent reset. Linux `iommu_group` symlinks remain absent in this dom0;
the old Ubuntu group number is not the basis for this decision.

## What the installed reset stack would do

Read-only PCI evidence: `HEADER_TYPE=80` (multifunction endpoint),
`CAP_PM+4.w=0000` (D0, NoSoftRst clear), `CAP_EXP+4.l=00008fa1` (no FLR).
Linux reports only `bus` in `reset_method`. There is no `physical_slot`,
`/sys/bus/pci/slots` is empty, and pciback's legacy `do_flr` file is absent.

### Two different reset decisions

Libvirt 10.5's `virPCIDeviceReset` is not a wrapper around Linux's
`reset_method`. For pciback it reads PCI capabilities itself. After finding
no FLR, it accepts the advertised PM capability/clear NoSoftRst bit and would
write D3hot, wait 10 ms, write D0, wait 10 ms, then restore saved configuration
space. Its success result does not measure GPU-state erasure. Only if this
method fails does it try the secondary-bus reset, whose sibling-device guard
would refuse the present active dom0 siblings.
See [libvirt's detection and reset implementation](https://github.com/libvirt/libvirt/blob/v10.5.0/src/util/virpci.c#L757).

Linux 6.18.49 applies `quirk_no_pm_reset` to non-root-bus AMD/ATI VGA devices,
including this GPU by vendor/class. The quirk records older AMD GPUs falsely
advertising PM reset while retaining framebuffer contents and monitor sync.
It prevents Linux's PM reset selection. It is a broad quirk, **not proof that
this particular Rembrandt chip was tested and found defective**.
See [the quirk](https://github.com/gregkh/linux/blob/v6.18.49/drivers/pci/quirks.c#L3807)
and [PM reset eligibility](https://github.com/gregkh/linux/blob/v6.18.49/drivers/pci/pci.c#L4433).

Linux's bus method rechecks scope at execution: the slot path rejects a
multifunction device; the parent-bus path rejects any sibling device on that
bus. With this topology it is expected to return `-ENOTTY` before a bus reset.
We did not write the sysfs reset file to test that prediction. A cached
`reset_method=bus` listing is not proof that a runtime reset can succeed;
enumeration-time eligibility may differ after sibling discovery.
See [kernel bus-reset guards](https://github.com/gregkh/linux/blob/v6.18.49/drivers/pci/pci.c#L4796).

### Lifecycle trace

| Stage | Reset attempt and failure handling |
| --- | --- |
| pciback initialization / device release | `pcistub_reset_device_state` calls Linux's function-reset helper but discards its return; on this PV dom0 it then returns zero. Reservation success does not prove reset success. |
| Qubes pre-start | Qubes binds assigned PCI functions to pciback and supplies managed hostdev XML. Default configuration has no reset/permissive bypass. |
| Libvirt prepare | Detach, then `virHostdevResetAllPCIDevices`; the PM path above can report success. A propagated reset error aborts preparation. |
| Xen libxl add | `libxl__device_pci_reset` tries `do_flr`, then sysfs `reset`. The add path does not use its return as a stop condition. Qubes' stubdomain patch moves initial attachment work earlier; it does not make reset failure fatal. |
| Failed start / guest teardown | Applicable unwind/cleanup paths call libvirt hostdev reattachment, which attempts resets again. Xen removal also attempts reset for virtual function zero without making its result fatal. Which cleanup paths run depends on how far startup got. |
| Host shutdown | Guest shutdown/destruction invokes teardown; no additional verified GPU-state sanitization guarantee was found. Cold power-off is recovery, not evidence that earlier resets worked. |

Sources: [pciback](https://github.com/gregkh/linux/blob/v6.18.49/drivers/xen/xen-pciback/pci_stub.c#L101),
[libvirt hostdev lifecycle](https://github.com/libvirt/libvirt/blob/v10.5.0/src/hypervisor/virhostdev.c#L586),
[libxl domain cleanup](https://github.com/libvirt/libvirt/blob/v10.5.0/src/libxl/libxl_domain.c#L838),
[Xen PCI lifecycle](https://github.com/xen-project/xen/blob/c2ece6c994a236e9ba51c9ec99085ae99347d552/tools/libs/light/libxl_pci.c#L1544),
and [Qubes stubdomain patch](https://github.com/QubesOS/qubes-vmm-xen/blob/v4.19.4-7/0607-libxl-add-pcidevs-to-stubdomain-earlier.patch).
Managed-device reattachment depends on original binding metadata; do not
assume cleanup will load dom0 `amdgpu`, which the lab blacklists.

The pinned Qubes patches do not make libvirt consult Linux's AMD reset quirk.
`nostrictreset` only suppresses the final reset error, not the reset attempts;
it also relaxes libxl reserved-device-memory policy. Guest PCI power-management
filtering is separate from these host-side reset writes. None of these
options was changed. See [reset-error patch](https://github.com/QubesOS/qubes-core-libvirt/blob/v10.5.0-2/0005-Add-nostrictreset-attribute-to-PCI-host-devices.patch),
[RDM patch](https://github.com/QubesOS/qubes-core-libvirt/blob/v10.5.0-2/0007-libxl-plug-workaround-for-missing-pcidev-group-assig.patch),
and [guest PM filtering patch](https://github.com/QubesOS/qubes-core-libvirt/blob/v10.5.0-2/0017-Add-powerManagementFiltering-for-PCI-PM-control.patch).

## Proposed experiment after this read-only review

Subsequently performed once with user authorization; see the linked hardware
report above. Assignment/startup succeeded, but video-BIOS discovery failed.

The justified next step is to review a **single-owner, cold-boot-only lab
assignment** to `sys-gui`, retaining normal upstream flags. This accepts
unproven reset effectiveness for an experiment; it is not certification of
the normal reusable lifecycle. Do not enable `no-strict-reset` merely because
Linux lists only `bus`: the default path may already proceed.

Before that experiment, arrange persistent host/guest diagnostics, verify the
HVM/kernel/firmware setup, capture assignment state and document its exact
undo command. Keep sibling devices in dom0. Stop after the first failure;
do not loop guest restarts, reassign the GPU to another trust domain, or boot
the hybrid desktop with a persistent experimental assignment still present.
Automatic cleanup can attempt resets even if the experiment avoids explicit
reset commands. A cold boot also does not prove the integrated GPU loses all
state; platform reset efficacy and repeatability remain hardware questions.

Unresolved: actual assignment success, guest initialization, physical panel,
and reset/state isolation across lifecycle transitions. Those require a
separately accepted hardware experiment, not more confidence inferred from
sysfs listings. Hyprland and input forwarding remain outside this review.
