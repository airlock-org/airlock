# Declarative sys-gui GPU assignment

The optional physical-GPU profile now connects a machine's GPU selection to
early Xen reservation, existing Qubes bootstrap, and one guest start. Firmware
delivery remains [a separate Qubes startup extension](amd-vbios-automatic.md).
This is not automatic hardware discovery, reset certification, or Hyprland.

## Configuration boundary

`airlock-machine/lifecycle/misc/modules/gui-gpu.nix` is reusable. A machine configuration supplies
`airlock.guiGpu.device`: the host PCI `bdf`, Qubes topology `port`, expected
four-digit `vendor` and `device` IDs, and dom0 `driver`. Declaring this inventory
alone does not reserve or assign anything. Enable ownership separately:

```nix
airlock.guiGpu = {
  enable = true;
  startOnBoot = true;
  # device = { ... }; supplied by the machine configuration
};
```

The ThinkPad selection lives in
`airlock-machine/lifecycle/image/hardware/thinkpad-t14-gen3-amd.nix`; only `usb-gui-lab` enables
it. The normal USB still uses the hybrid dom0-driven display. The existing USB
flake imports this ThinkPad hardware fixture: it is not yet a universal USB
installer. Future installation/provisioning should generate machine-local
selection rather than require editing shared OS defaults.

The current module supports one segment-zero GPU for HVM `sys-gui`, with
`video-model=none`. The machine must not explicitly load its graphics driver
in the initrd when ownership is enabled. Unrelated early modules are retained.
Other vendor choices exercise the selection mechanism, not certified guest
driver/firmware support. The physical AMD guest still needs compatible VFCT.

## Boot sequence and checks

1. Nix generates `xen-pciback.hide=(BDF)`, loads xen-pciback early, blacklists
   the selected dom0 driver, disables the host firmware framebuffer by default, and
   requires Xen's IOMMU. It does not reserve sibling audio/USB/security devices.
2. Before bootstrap mutates VM/storage/settings, `airlock-gui-gpu-check` checks
   the PCI function exists, vendor/device and display class match, pciback owns
   it, and Qubes resolves the selected port to exactly that host BDF. It checks
   the generated topology agrees and rejects conflicting existing ownership,
   options or assignments. Its API calls and sysfs access are read-only.
3. The existing `airlock-setup-config` creates/configures sys-gui and applies
   its sole persistent **required**, port-matched Qubes assignment. Matching
   assignments are reused; stale or conflicting assignments are not silently
   removed. No reset/permissive options are supplied.
4. `airlock-start-gui-gpu.service` requires successful bootstrap, checks that
   `qubesd` is active, rechecks the hardware and persistent assignment, then
   runs ordinary `qvm-start sys-gui`
   once. Qubes/VFCT/libvirt perform the normal guest start. Qubes autostart is
   disabled for this VM so it cannot race this ordering; no retry loop or
   automatic guest restart is configured.

The USB AMD lab now opts into a temporary firmware console for bootstrap
progress. Its startup precondition detaches that console and firmware driver
before `qvm-start`; failed release blocks the guest. This sole-display-GPU
experiment does not change the default reservation behavior for other users
of the module. See [the framebuffer handoff](history/usb-gui-lab.md#temporary-bootstrap-framebuffer).

Boot reservation is address-based and happens before userspace identity
validation. If hardware changes, a stale BDF can reserve the wrong function;
validation prevents assigning that mismatch to sys-gui, not the earlier
reservation. Review/regenerate the machine selection after hardware changes.
This is declarative application of a reviewed choice, not a hotplug manager.
Root can still change assignments or use Qubes commands manually; these checks
are boot-workflow safeguards, not a restriction on dom0 administrators.

Xen owns the IOMMU, so Linux dom0 `iommu_group` links are not a valid universal
precondition. Reset/isolation enforcement remains with Xen/Qubes/libvirt. No
ACS override, forced live unbind, `no-strict-reset`, or permissive flag is added.
Normal Qubes lifecycle operations can themselves reset the GPU.

## Profile switching and recovery

Persistent Qubes assignments survive reboot. An empty hybrid PCI declaration
does **not** delete the physical lab's assignment. On machines with declared
GPU inventory, the disabled profile's bootstrap guard rejects leftover
sys-gui PCI state instead of allowing the desktop relay to start it with dom0's
display GPU. Text recovery does not run bootstrap or start the guest.

To return to hybrid mode, boot text recovery, confirm sys-gui is halted, inspect
`qvm-device pci list --assignments`, and unassign the verified sys-gui GPU through
Qubes. Then reboot the default entry. Do not remove VM disks or `qubes.xml`.
Reflashing/reprovisioning also creates fresh runtime state, but is not necessary
just to remove a reviewed assignment.

## Boot acceptance (one successful AMD-lab boot recorded)

Boot `f34306b1-b7f6-483a-bfc6-7ef8cf298e1c` passed bootstrap, GPU startup and
framebuffer release on the rebuilt USB; the guest uses amdgpu and hardware
OpenGL. The user reached the desktop, and input senders are active. This
followed a default/hybrid boot; repeat cold-boot acceptance remains pending.
See [the recorded milestone](history/amd-gpu-investigation.md#current-milestone--2026-09-14).
The procedure below remains the checklist for subsequent acceptance runs.

```sh
make usb-dev-image
make usb-dev-provision USB_SERIAL=25110814640207
```

Boot the normal **Airlock** entry. `usb-gui-lab` is an internal profile name,
not a user-facing boot-menu entry. No manual `qvm-device` or firmware copy
should be needed. Reconnect with SSH and inspect:

```sh
sudo systemctl status airlock-setup-config airlock-start-gui-gpu --no-pager
sudo journalctl -b -u airlock-setup-config -u airlock-start-gui-gpu -u qubesd --no-pager
sudo qvm-device pci list --assignments
sudo qvm-ls --raw-data --fields NAME,STATE
```

Bootstrap's `gui-gpu-check.json` lives beside `setup-config.json` in its
configured log directory (the start guard also prints JSON to the journal).
Confirm the active libvirt hostdev, generated VFCT manifest and guest
`amdgpu`/`/dev/dri` evidence agree. After the fresh-flash pass, one further host
reboot should retain the same assignment and succeed without manual changes.
Do not use repeated guest stop/start loops as a substitute for that check.

The USB AMD lab now opts into the guest's physical XFCE/LightDM boot and starts
the input target only after successful `qvm-start`. A blank panel by itself
does not identify which stage failed; check the guest display manager and each
input sender separately. Hyprland remains a subsequent milestone.

Implementation verification uses pytest fixtures with a different PCI address,
port and vendor, plus Nix evaluation of normal, laptop and alternate selections.
The current successful boot was inspected read-only. Earlier implementation
checks below are historical; the current milestone records the newer evidence.

### Implementation checks — 2026-09-14

90 focused pytest checks passed, including selection/boot wiring, firmware
regressions, USB/login and minimal-GUI configuration, bootstrap runtime PATH,
idempotence and network-assignment ordering. The lab's generated `/etc` built,
and the full USB system derivation evaluated successfully.

Read-only validation resolved the real laptop's `0000:04:00.0` through Qubes
port `00_08.1-00_00.0` and verified its IDs and pciback ownership. The checker
correctly refused the current running guest without its persistent assignment.
A fresh-bootstrap projection (only sys-gui omitted from the queried inventory,
no real VM removed) passed against the real host hardware; a wrong vendor was
rejected. `qubes.xml` was unchanged. No services, assignments, guest state or
boot selections were changed on the laptop during this implementation.

Evidence is preserved under `.airlock-logs/gui-gpu-assignment/`. These results
do not replace the pending real fresh-flash and subsequent reboot checks.

### Bootstrap daemon stop/start cancellation — 2026-09-14

On laptop boot `1384e043-2b30-4042-b3c5-ed4b15ed85bf`, bootstrap completed and
the persistent GPU assignment passed validation, but sys-gui remained halted.
The guest startup service had no invocation: it had not attempted `qvm-start`.
Bootstrap's storage adoption stops `qubesd`, updates storage, then starts it
again. The startup service's `Requires=qubesd.service` propagated that explicit
stop to its pending start job while it was still waiting for bootstrap.

The startup service now uses `Wants=qubesd.service` and retains `After=` for
ordering. It still requires successful bootstrap, and an explicit
`systemctl is-active --quiet qubesd.service` precondition fails startup if the
daemon did not return. No guest retries, reset changes, or laptop-specific
workarounds were added.

`airlock-machine/tests/test_gui_gpu_boot_order.py` reproduces the cancellation with inert,
uniquely named local user-systemd units and verifies the corrected dependency.
It also verifies that failed bootstrap or an unavailable daemon blocks the
guest. The fixture deliberately uses separate **stop and start** operations,
matching storage adoption; a single `systemctl restart` did not reproduce the
cancelled job. Run it explicitly with:

```sh
AIRLOCK_RUN_SYSTEMD_ORDER_TEST=1 make test-quick PYTEST_DEFAULT_TARGETS= \
  PYTEST_SELECTED='airlock-machine/tests/test_gui_gpu_boot_order.py airlock-machine/tests/test_gui_gpu_assignment.py'
```

Result: 31 passed, one optional SSH check skipped. Evidence is under
`.airlock-logs/gui-gpu-boot-order/`; laptop boot evidence is under
`.airlock-logs/gpu-boot-1384e043/`. The fix has not yet passed a real fresh boot;
the laptop's guest and GPU state were left unchanged during this fix.
