# Automatic AMD VBIOS delivery

Airlock can prepare an AMD GPU's VBIOS from the booted host's ACPI VFCT table
during ordinary Qubes VM startup. There is no laptop address, product ID,
pre-extracted firmware file, or source-table hash in the configuration.

The opt-in Nix module is `lifecycle/misc/modules/amd-vbios.nix`:

```nix
airlock.amdVbios = {
  enable = true;
  domains.sys-gui = { }; # guestSlot defaults to 16 (PCI slot 0x10)
};
```

The AMD USB lab enables it. The default hybrid profile does not. Device
reservation, assignment, and the decision about isolation/reset safety remain
separate: the lab still reserves its reviewed GPU through its hardware boot
configuration. Another machine must supply its own reservation and input
selection. This feature does not discover which GPU the user wants to assign.

## Startup behavior

1. Qubes' normal `qvm-start` runs the extension's pre-start validation. The
   opted-in domain must have exactly one required PCI assignment from dom0.
   The extension resolves that assignment through Qubes, reads the device's
   actual PCI identity in sysfs, and requires an AMD display controller already
   reserved to pciback. It never binds, assigns, or resets hardware itself.
2. Before libvirt creates the guest, the extension reads the current host
   `/sys/firmware/acpi/tables/VFCT`. The shared validator selects exactly one
   image matching the GPU's host PCI address and vendor/device IDs. It checks
   ACPI lengths/checksum, image boundaries, ROM checksum/length, ATOM header,
   and PCIR identity. Ambiguous or unsupported tables stop startup.
3. It constructs a filtered VFCT containing only that image. The VBIOS ROM
   bytes remain unchanged. The table's PCI address is changed to the chosen
   guest slot, and its ACPI checksum is recomputed. A new root-only directory
   under `/run/airlock-amd-vbios/` contains `vfct.bin` and `manifest.json` for
   that attempt. No previous prepared table is reused.
4. A per-domain template renders the upstream Qubes Xen XML. The extension
   preserves that definition and adds only the matching hostdev's explicit
   guest PCI address and the generated VFCT via the existing ACPI transport.
   Conflicting custom ACPI tables, addresses, or device identities are errors.
5. The extension checks libvirt's XML-to-XL conversion before creation. It
   requires the explicit guest slot and the generated table path to survive
   conversion; an old libvirt daemon cannot silently discard the slot. Qubes
   then performs its normal VM creation and PCI assignment.

The firmware hash records provenance and detects changes, but does not
authenticate the manufacturer's firmware. Structural validation is not a
claim that firmware is harmless. Raw firmware stays on the machine; manifests
and the journal contain identities, lengths, and hashes.

Pre-start and hotplug guards use Qubes instance handlers, which run before
class-level PCI extension handlers for pre-events. Attaching/detaching PCI
devices while an opted-in VM is running is rejected. Removing a persistent
assignment without detaching the running device remains available for the lab.
Failed/stopped starts clear the in-memory prepared state; artifacts remain in
the root-only runtime directory for diagnosis until reboot. Resuming saved VM
state is outside this workflow; the USB lab disables managed save/auto-restore.

## Why a libvirt patch is needed

Pinned libvirt 10.5's `libxlMakePCI` passed the host address to Xen but omitted
the requested guest address. Guessing QEMU's automatic allocation would make
firmware identity depend on the device layout.

Airlock's `0020-libxl-honor-explicit-guest-pci-slot.patch` passes an explicit
guest slot to existing Xen `vdevfn` support and preserves it in XML/XL
roundtrips using `host-address@slot`. It rejects unsupported guest addresses.
Devices without an explicit address retain automatic placement. No Xen patch
is added, and no reset, permissive, or reserved-memory security flags are changed.

Source references: [pinned libvirt PCI conversion](https://github.com/libvirt/libvirt/blob/v10.5.0/src/libxl/libxl_conf.c#L2102),
[Qubes startup events](https://github.com/QubesOS/qubes-core-admin/blob/v4.3.44/qubes/vm/qubesvm.py),
[Qubes event ordering](https://github.com/QubesOS/qubes-core-admin/blob/v4.3.44/qubes/events.py).
The old `slic` spelling is the pinned Xen driver's raw ACPI-table transport;
see [the original VFCT investigation](amd-gpu-vbios.md).

## Supported boundary

- One required, segment-zero AMD display-controller assignment from dom0.
- A compatible VFCT with one unambiguous matching ATOM/PCIR ROM image.
- Qubes HVM, SeaBIOS, Linux stubdom, `video-model=none`, and no custom QEMU
  arguments. No VFCT library section, multiple GPU images for the same device,
  or alternative firmware acquisition mechanism is supported yet.
- Explicit guest bus zero/function zero, slot 8–31 (default 16). This is a
  virtual machine layout contract, not a host hardware address.
- Firmware delivery at normal VM creation; no GPU hotplug, live migration,
  managed-save resume, or reset-safety certification.

Other vendors, missing VFCT, unsupported table layouts, absent devices, and
conflicting VM layouts produce an error through Qubes. GPU discovery is not
general AMD hardware certification; a second physical machine remains untested.

## Laptop validation — 2026-09-14

Host boot `80202319-c266-4f62-acdc-f0e0f2be2074` was the AMD lab following a
host reboot. sys-gui was halted throughout implementation and the negative
checks. The built extension, generated Nix config/template, and patched
libvirt were staged on the running host without a full image rebuild.

Observed results:

- No assignment: ordinary `qvm-start` failed with the explicit required-PCI
  error, leaving the guest halted.
- With the reviewed assignment and the old libvirt daemon: preparation
  succeeded but startup stopped before domain creation because conversion
  discarded the guest slot. No GPU guest start occurred.
- Patched libvirt: 14 read-only pytest conversion checks passed, covering
  slots 8/16/18/31, native roundtrips, unchanged automatic placement, and
  invalid addresses/slots.
- One ordinary `qvm-start sys-gui` then succeeded. The extension read the host
  VFCT automatically and generated it for `0000:00:10.0`. The guest enumerated
  the GPU at precisely that address, reported `Fetched VBIOS from VFCT`, bound
  `amdgpu`, and created `card0` and `renderD128`.
- Guest VFCT hash matched the generated table:
  `c53b20cfb087f59a036e9dc9b8b99f2f0a6cf3adc45b953e7c013c49e69fae12`.
  This differs from the old manually prepared table for slot 5. The old
  provisioned `/var/lib/airlock/amd-lab/sys-gui.vfct` was not used.
- The persistent experimental PCI assignment was removed after startup;
  the running GPU remains attached. Dom0 retains pciback for that function and
  its normal drivers for the sibling functions. Display manager/input were
  intentionally not started: this test concerns firmware automation.

Raw evidence: `.airlock-logs/amd-vbios-auto/auto-80202319/` on the workstation
and `/var/lib/airlock/amd-lab/auto-80202319/` on the laptop. The manifest and
kernel log record the firmware and PCI identities; firmware bytes are not in Git.

The final pre-event guard ordering was hardened after this GPU start and
checked separately against the actual Qubes event dispatcher. This did not
require another GPU start/reset. A freshly rebuilt image exercising the entire
workflow without runtime staging remains the next reproducibility check.

Final verification: 58 focused pytest checks passed (43 synthetic firmware,
discovery and XML checks; 14 real libvirt conversions; one real Qubes event
ordering check). Both packages and the AMD lab's generated `/etc` built;
Nix assertions passed and the default hybrid profile retained automation disabled.

## Build and checks

```sh
make test-quick PYTEST_DEFAULT_TARGETS= \
  PYTEST_SELECTED='tests/test_amd_vbios.py tests/test_amd_vbios_runtime.py'
AIRLOCK_VBIOS_LIBVIRT_SSH=root@HOST \
  uv run pytest tests/test_amd_vbios_libvirt.py
make usb-dev-image
make usb-dev-provision USB_SERIAL=YOUR_SERIAL
```

The libvirt checks only convert synthetic configurations; they do not define
or start a domain. Without the explicit SSH environment variable they skip.
The normal provisioning command no longer copies workstation VBIOS artifacts.
The offline `lifecycle/image/tools/amd-vbios.py` command is retained for inspection
and historical experiments, using the same validator as the startup extension.

The running laptop uses temporary systemd overrides under `/run`; a reboot
returns to the flashed image's binaries. Rebuild/reflash to make this version
the bootable baseline. The staged test made no permanent boot-order change.
