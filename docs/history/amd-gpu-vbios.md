# AMD laptop VBIOS delivery experiment

This page records the original manual experiment. The current implementation
is [automatic AMD VFCT preparation](../amd-vbios-automatic.md): it discovers the
assigned device at startup and explicitly places it at the guest address used
in its generated table. No pre-extracted laptop firmware is required.

Follow-up to [first assignment](amd-gpu-first-assignment.md). This is a
hardware-specific lab mechanism, not general AMD passthrough certification.
It addresses the observed `Unable to locate a BIOS ROM` driver failure.

## Mechanism

Read the laptop's existing ACPI VFCT table through sysfs. Select only the
VBIOS image matching the host GPU's PCI address and vendor/device IDs. Verify
the source hash, ACPI checksum/bounds, ROM checksum/length, ATOM header and
PCI ROM identity. Construct a new VFCT containing only that image, labelled
with the GPU's observed *guest* PCI address. The ROM bytes are unchanged;
unrelated ACPI tables, UUID and library entries are not forwarded.

`lifecycle/image/tools/amd-vbios.py` implements this offline transformation.
It contains no PCI writes, reset operations, firmware flashing, device
assignment or VM-start commands. Outputs require a new directory; existing
files are never overwritten. Firmware bytes remain in local artifacts, not
Git or a generic USB image. Validation is a structural/identity check, not a
cryptographic certification that firmware is harmless.

The AMD lab's per-VM Jinja template extends Qubes' existing `libvirt/xen.xml`
and adds one ACPI table under `<os>`. It activates only with the explicit
`airlock-amd-vfct=1` feature, in sys-gui HVM, with the reviewed laptop GPU port
as the sole PCI assignment. It reads the root-owned, read-only file
`/var/lib/airlock/amd-lab/sys-gui.vfct`.

Pinned libvirt **10.5/Xen** uses `<table type="slic">` as a raw ACPI table
transport, passing its path to libxl `acpi_firmware` without changing the
table's signature. Xen's table builder inserts the supplied table. This is
an unfortunate compatibility spelling, not a SLIC table or licensing change.
Modern libvirt calls the Xen transport `rawset`; do not substitute the old
spelling on the QEMU libvirt driver, which has different behavior.
See [libvirt's documented compatibility](https://libvirt.org/formatdomain.html#common-os-element-configuration),
[pinned libvirt mapping](https://github.com/libvirt/libvirt/blob/v10.5.0/src/libxl/libxl_conf.c#L585),
and [pinned Xen table insertion](https://github.com/xen-project/xen/blob/c2ece6c994a236e9ba51c9ec99085ae99347d552/tools/libacpi/build.c#L298).

This uses the existing Qubes/Xen/libvirt control path. It does not introduce
a new GPU service or replace the hypervisor/device model. The alternative
Linux initrd ACPI-override mechanism was considered but is unnecessary for
this pinned stack and would require changing the shared guest boot artifact.

## Validated laptop data — 2026-09-13

| Item | Value |
| --- | --- |
| Host VFCT | 44,676 bytes, one nonempty VBIOS entry plus an empty trailing entry |
| Host GPU | `0000:04:00.0`, `1002:1681` |
| Observed guest GPU | `0000:00:05.0`, `1002:1681` |
| VBIOS | 44,544 bytes, valid ROM checksum, ATOM and PCIR identity |
| Filtered VFCT | 44,648 bytes, valid ACPI checksum |

Hashes:

```text
Host VFCT: 933b77e50ac746abddba4e5779993ea93e7f381559422687b7fb34e85fe9d8a1
VBIOS:     a8ddd317b991160a01df7141a5598af35324eb520e0d4f40cc8e45d872e47c33
Guest VFCT:c3d6e50174efb225c6fa610458a6d43c1b256a7ebacd0568283234807951329e
```

Local capture/preparation command (paths are example local artifacts):

```sh
python3 lifecycle/image/tools/amd-vbios.py \
  --input .airlock-logs/amd-vbios/host-vfct.bin \
  --sha256 933b77e50ac746abddba4e5779993ea93e7f381559422687b7fb34e85fe9d8a1 \
  --host-bdf 0000:04:00.0 --guest-bdf 0000:00:05.0 --device 1681 \
  --out-dir .airlock-logs/amd-vbios/prepared
```

Omit `--out-dir` for read-only validation. Re-enumerate hardware and record
the source hash before preparing for another machine. Do not reuse these
addresses or the laptop firmware on another system. If guest PCI layout
changes, regenerate its VFCT mapping; a mismatch should fail driver discovery.
Upstream [AMD VFCT lookup](https://github.com/gregkh/linux/blob/v6.12.64/drivers/gpu/drm/amd/amdgpu/amdgpu_bios.c#L328)
checks the guest BDF and IDs before using an image.

## Controlled runtime procedure

Keep the GPU reserved to pciback in a fresh host lab boot. Keep sys-gui's
autostart disabled, use the usual Qubes kernel/modules, and leave all sibling
devices in dom0. Arm the verified text-recovery entry for the next boot only
before making a persistent assignment. Back up preferences/features/XML and
record undo commands before changing them.

For the already-built guest, append `systemd.unit=multi-user.target` to its
existing kernelopts for this attempt. The source configuration now makes
multi-user the default for the physical lab, avoiding upstream
`graphical.target` automatically pulling in the display manager. Preserve the
rest of kernelopts and restore the original value after the experiment.

Install the validated filtered VFCT root-owned/read-only at the fixed path,
enable the lab feature, assign only the GPU with default upstream flags, and
start once. Remove the persistent assignment immediately after the start
returns; that does not hot-detach the running device. Inspect the guest VFCT,
driver logs, DRM devices and display-manager state before manually starting X.
Do not retry driver initialization in a loop or add reset/permissive bypasses.

Cleanup: remove the experimental PCI assignment and `airlock-amd-vfct`
feature, restore kernelopts, stop the guest if the attempt failed, and clear
the one-shot recovery selection only after cleanup is verified. Keep logs and
firmware artifacts for inspection. No default boot-order change is required.

The intermediate USB workflow installed a prepared VFCT during personalization.
That workflow is superseded by automatic startup preparation; current normal
provisioning no longer copies the local artifact. Older images can still use
the lower-level personalization `--amd-vfct` option to reproduce this experiment.

## Runtime result — successful driver initialization

On 2026-09-13 the host was rebooted once into Generation 2 `usb-gui-lab`.
New boot ID: `75eadf68-6b62-4c42-8ce7-5960acb3809a`. This was a host reboot,
not a measured removal of platform power. No guest was automatically started.
The original host VFCT hash was unchanged after reboot.

The exact repository template and validated data were installed manually on
the running USB; the source Nix profile now installs the same template on
future builds. No kernel, guest closure, image or hypervisor rebuild was
needed. Qubes' actual template rendering and libvirt XML-to-native conversion
were checked before starting: the latter produced the expected
`acpi_firmware = "/var/lib/airlock/amd-lab/sys-gui.vfct"`.

One `qvm-start --verbose sys-gui` succeeded. The kernel reported:

```text
amdgpu 0000:00:05.0: amdgpu: Fetched VBIOS from VFCT
amdgpu: ATOM BIOS: 113-REMBRANDT-X37
[drm] Initialized amdgpu 3.61.0 for 0000:00:05.0 on minor 0
```

The VFCT read back from inside the guest has exactly the prepared table's
SHA-256. `lspci` shows `Kernel driver in use: amdgpu`; `/dev/dri/card0` and
`/dev/dri/renderD128` exist. The built-in `card0-eDP-1` connector reports
`connected` and includes `3840x2400` among its modes. Dom0 retains pciback
ownership of the physical GPU function and normal drivers for its siblings.
The guest display manager is `inactive`, as intended with the temporary
multi-user kernel argument.

Full privileged guest logs were obtained through the existing dom0
`qrexec-client` direct-command interface, without changing guest RPC policy:

```sh
sudo qrexec-client -d sys-gui \
  'root:/run/current-system/sw/bin/journalctl -b -k --no-pager' </dev/null
```

That successfully runs as root, unlike `qubes.VMShell` with its forced-user
configuration. Redirect stdin for noninteractive diagnostics so the client
does not consume subsequent shell commands.

Final state: **sys-gui remains running with the GPU attached**, ready for a
separate desktop/input experiment. No X server, graphical login, rendering
workload or guest restart was attempted. Panel enumeration is not proof of
a usable physical desktop; reset/restart safety is still not certified.

The persistent PCI assignment was removed immediately after startup, the
VBIOS feature was unset, original kernelopts restored and the temporary
recovery selection cleared. None of those changes hot-detaches the current
GPU or changes the running guest's boot arguments. `qubes.xml` confirms no
persistent PCI assignment; the CLI still shows the current *attached* device.
The normal hybrid boot remains the default. Do not restart this guest and
expect this experimental configuration to persist automatically.

Evidence on the laptop:
`/home/airlock/dev/nixome/.airlock-logs/usb-dev/75eadf68-6b62-4c42-8ce7-5960acb3809a/vbios-attempt/`.
Workstation copy: `.airlock-logs/amd-vbios/run/`. Both have a machine-readable
`result.json`; firmware artifacts and raw logs remain outside Git.
Guest kernel-log SHA-256:
`7bd0a91e1c4e54bc2eaf3f946280d2434908abff0152abdf8317f5e5a911a6d0`.

Validation performed: real firmware parsing and output re-validation, actual
Qubes/libvirt configuration conversion, Nix syntax checks, Python formatting
and lint, and this single hardware start. Synthetic pytest coverage was added
for malformed input, wrong identity/hash and ambiguous images, but the test
suite was not run. No generic image build was performed.
