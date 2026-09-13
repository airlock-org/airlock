# Laptop GPU: first assignment experiment

2026-09-13, 15:24–15:27 UTC. Follow-up to the
[assignment/reset review](amd-gpu-assignment-reset-findings.md), explicitly
authorized as one single-owner lab attempt, with normal upstream PCI flags.

## Result

**PCI assignment and HVM startup succeeded. GPU initialization failed because
the guest could not find the video BIOS.** The experiment stopped at that
failure, without a second start, driver reload, manual reset or bypass.

The guest console records at 15:24:17 UTC:

```text
amdgpu 0000:00:05.0: amdgpu: Unable to locate a BIOS ROM
amdgpu 0000:00:05.0: amdgpu: Fatal error during GPU init
amdgpu 0000:00:05.0: probe with driver amdgpu failed with error -22
```

This advances the assignment question; it does not validate graphics or
reset effectiveness. The immediate blocker is video-BIOS discovery, not an
observed strict-reset rejection. Successful startup/shutdown and pciback's
`reset done` messages do not demonstrate erasure of previous GPU state.

## Exact experiment and observations

The laptop remained on boot ID `3a19d380-405e-4e8e-8c3f-c2ddba13091a`,
running system and software versions recorded in the preceding review.
No host reboot was performed: this was the first guest GPU assignment in
that existing lab boot, not a separate cold-power-cycle reproducibility test.
Before starting, only dom0 ran; the GPU was reserved by pciback and sys-gui
had no PCI assignment or managed save. Qubes preferences selected HVM, 4 GiB,
four vCPUs, no GUI VM and no autostart.

Host journal persistence/one-second sync and Xen guest-console logging were
already enabled. Backups of `qubes.xml`, guest preferences, inactive libvirt
XML, logs and boot selections were saved before mutation. A live console
stream was also captured on the workstation. The matching shared Qubes
kernel image contained `amdgpu.ko`; the guest closure contained the AMD
`yellow_carp` firmware files and qrexec agent.

The experiment used the currently enumerated Qubes device identifier:

```sh
qvm-device pci assign --required sys-gui dom0:00_08.1-00_00.0
qvm-start --verbose sys-gui
qvm-device pci unassign sys-gui dom0:00_08.1-00_00.0
```

Each returned zero. The last command removed only the persistent assignment;
it did not detach the running GPU. It was deliberately run immediately after
startup to avoid carrying an experimental assignment into another boot.

| Check | Observed result |
| --- | --- |
| Guest start | `qvm-start` returned 0; qrexec commands worked |
| Active libvirt definition | HVM (`xenfv`), managed PCI hostdev `0000:04:00.0`, no `nostrictreset` |
| Xen PCI backend | GPU exposed through device-model stubdomain `sys-gui-dm` (domid 2) for sys-gui (domid 1) |
| Backend options | `msitranslate=0,power_mgmt=0,permissive=0,rdm_policy=strict` |
| Guest PCI enumeration | Radeon 680M `1002:1681`, Lenovo `17aa:50b6`, at `0000:00:05.0` |
| Guest kernel | `6.12.64-1.qubes.fc41.x86_64`; amdgpu module loaded, probe failed, no driver bound |
| DRM | `/dev/dri` absent; no card/render devices under `/sys/class/drm` |
| Platform BIOS clue | Dom0 has a 44,676-byte ACPI `VFCT` table; guest ACPI inventory has no VFCT |
| Other devices | Bus-04 audio/PSP/USB/ACP retained dom0 drivers; no sibling assigned |
| Teardown | One `qvm-shutdown --wait --timeout 60 sys-gui`, exit 0; guest and stubdomain disappeared |
| Final host state | SSH working, only dom0 running, GPU still bound to pciback, no persistent sys-gui PCI assignment |

`xl pci-list sys-gui` was empty despite guest GPU enumeration. For this
Qubes HVM/stubdomain configuration the useful corroborating query was
`xl pci-list sys-gui-dm`, together with
`xenstore-ls /local/domain/0/backend/pci` and the active libvirt XML.
Do not interpret the first command's empty result as failed passthrough.

The next-boot-only text-recovery selection was armed before assignment using
`bootctl set-oneshot` with the verified Generation 2 Xen recovery entry.
After successful cleanup it was cleared with `bootctl set-oneshot ''`.
The original `loader.conf` was compared byte-for-byte and remained unchanged;
no persistent default or firmware boot order was changed.

## What this does not establish

The driver module and redistributable firmware being present are not the
same as providing the GPU's platform video BIOS. Upstream Linux's AMD BIOS
discovery includes an ACPI VFCT path and other ROM-reading paths. The host
table's presence and guest table's absence identify a candidate to investigate,
not proof that copying the table unchanged would work. VFCT selection checks
the GPU's PCI location and IDs; the guest and host BDFs differ here.
See [Linux 6.12.64 AMD BIOS discovery](https://github.com/gregkh/linux/blob/v6.12.64/drivers/gpu/drm/amd/amdgpu/amdgpu_bios.c#L328).

No ROM was extracted, injected or enabled through sysfs. No ACPI table,
kernel, reset policy, device-model configuration or firmware was changed.
The precise libvirt PM-reset result was not instrumented; the preceding
source trace remains a prediction, not a measurement of reset efficacy.

Two preparation assumptions also need correction before another attempt:

- The physical guest's display manager started automatically. The built
  `graphical.target` still depends on it; clearing the service's `wantedBy`
  did not keep it manual. Its `active` status was not a working GPU display.
- `qubes.VMShell` forces `user`, so `qvm-run --user root` did not provide root
  journal access. The service PATH also selects the unwrapped store `sudo`
  before `/run/wrappers/bin/sudo`. No guest privilege policy was changed to
  work around this. Console logs supplied the decisive driver error.

Guest shutdown additionally logged a failed `/lib/modules` unmount, but
continued to power down and Qubes reported successful shutdown. Record this
separately from the GPU probe failure; it is not evidence of a reset hang.

## Evidence and next action

Laptop evidence directory:
`/home/airlock/dev/nixome/.airlock-logs/usb-dev/3a19d380-405e-4e8e-8c3f-c2ddba13091a/first-assignment/`.
Workstation copy: `.airlock-logs/amd-first-assignment/`.
Machine-readable outcome: `result.json`. Raw logs/state remain uncommitted.
The final console log SHA-256 is
`3496bed7bf0606628e8281284b2e70c1f50b40777f54a82f9a8f3491ba47f769`.

Next: review how to obtain this laptop's matching VBIOS and expose it through
the pinned Xen/Qubes HVM stack, while fixing the lab's manual-display and
diagnostic-access assumptions. Do not use a random downloaded GPU ROM or
relax reset checks. A further hardware start needs a reviewed concrete change;
this experiment deliberately did not become a retry loop.
