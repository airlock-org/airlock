# Upstream Notes

Observed on 2026-09-05.

## NixOS Xen Support

Source:
https://github.com/NixOS/nixpkgs/tree/nixos-unstable/nixos/modules/virtualisation

NixOS has upstream Xen dom0 support through `virtualisation.xen.enable`.
The module configures Xen boot entries, Xen services, `/proc/xen`, kernel
module requirements, `xl`, Xen store, and a QEMU dom0 disk backend.

Important constraints from the NixOS module/docs:

- Xen dom0 support is documented as available since NixOS 24.11.
- The supported boot paths are systemd-boot, Lanzaboote with the relevant Xen
  support, or Limine.
- `boot.initrd.systemd.enable = true` is required. Legacy scripted stage 1 is
  rejected.
- NixOS checks software/kernel requirements, but it does not check hardware
  SLAT/IOMMU availability.

## CertainLach/nixos-qubes

Source:
https://github.com/CertainLach/nixos-qubes

Pinned upstream head inspected:

```text
d50ecf2185ed5232c63845d7fe8e16395e5096d7 refs/heads/trunk
```

The project is intended as the out-of-tree development home for Qubes packages
and NixOS modules, with a later possible merge into nixpkgs. The previous
in-tree attempt referenced by upstream was:
https://github.com/NixOS/nixpkgs/pull/341215

What it currently implements:

- `nixosModules.qubes`, including `virtualisation.qubes.dom0.enable`.
- Qubes-flavored Xen and QEMU integration for NixOS dom0.
- QubesDB dom0/domU services.
- `qubesd`, qmemman, qrexec policy daemon, Qubes core systemd units, libvirt,
  and Qubes policy/RPC directory wiring.
- Qubes packages for core-admin, core-admin-client, core-qrexec, QubesDB,
  core-libvirt, vchan-xen, GUI components, manager, Linux utils, notification
  proxy, Seabios, stubdom, and helper tools.
- `nixos-qubes-tools` for importing Qubes VM kernels and template RPMs.

Version target:

- README examples use Qubes R4.3 repositories and Fedora 43 templates.
- Key package versions inspected include:
  - `qubes-core-admin` 4.3.44
  - `qubes-core-admin-client` 4.3.30
  - `qubes-core-qrexec` 4.4.1
  - `qubes-core-qubesdb` 4.3.2
  - `qubes-core-admin-linux` 4.3.21
  - `qubes-linux-utils` 4.3.17
  - `qubes-gui-daemon` 4.3.15
  - `qubes-vmm-xen` 4.19.4

Known gaps/TODOs found in module/package comments:

- `virtualisation.qubes.secure = true` is asserted unsupported. The full Qubes
  OS security model is not implemented in the NixOS module yet.
- The upstream module sets `virtualisation.libvirtd.firewallBackend = "none"`,
  but current nixpkgs accepts only `iptables` or `nftables`. Nixome forces
  `iptables` for now because the upstream Qubes libvirt package is 10.5 and the
  upstream comment notes that it predates nftables backend support.
- Current nixpkgs runs `pythonMetadataCheckHook` for Python modules. Upstream
  `qubes-linux-utils.imgconverter` is named `qubes-linux-utils-imgconverter`,
  while the installed Python distribution is `qubesimgconverter`. The local
  Nixome fork sets `dontCheckPythonMetadata = true` for that subpackage; the
  import check for `qubesimgconverter` remains enabled.
- Current nixpkgs' Xen split-output install logic assumes it can derive one
  OCaml version directory with `ls $out/lib/ocaml/` before copying
  `site-lib/xsd_glue`. The Qubes-patched Xen build can make that guard true
  while the constructed path is absent. The local Nixome fork copies the actual
  `xsd_glue` directories found under `$out/lib/ocaml`.
- Dedicated `sys-net` and `sys-usb` are marked not ready for exclusive use in
  the dom0 module.
- Some Qubes policy and RPC paths are mutable under `/etc`; package removal can
  leave stale policy files.
- Qubes creates mutable systemd units for VM autostart behavior.
- The module notes unresolved Xen store/dom0 memory details around Qubes memory
  manager behavior.
- Some GUI/Xwayland/Xorg integration is still marked FIXME.
- Template/kernel installation is imperative for now, although helper tools
  exist and an OOBE module is present but incomplete.

Nixome now consumes a local source fork at `vendor/nixos-qubes`. The fork is
justified by concrete compatibility fixes needed for the POC build:
`qubes-linux-utils.imgconverter` metadata checking and Qubes-patched Xen OCaml
split-output installation. Keep this fork narrow and upstreamable. The fork's
`nixpkgs` input still follows Nixome's pinned `nixpkgs`; upstream's flake
records a developer-local `file:/home/lach/build/nixpkgs` input otherwise.

## Qubes Tests

Sources:

- https://doc.qubes-os.org/en/r4.3/developer/debugging/automated-tests.html
- https://github.com/QubesOS/qubes-core-admin
- https://github.com/QubesOS/qubes-core-qrexec
- https://github.com/QubesOS/openqa-tests-qubesos

Qubes uses Python `unittest` for both unit and integration tests. Integration
tests must run in dom0, assume dedicated hardware, and may remove `test-*` VMs.
Qubes documents running them with:

```bash
sudo -E python3 -m qubes.tests.run -v
```

Relevant existing integration test areas include VM lifecycle and qrexec tests,
for example:

- `qubes/tests/integ/basic.py`
- `qubes/tests/integ/qrexec.py`
- `qubes/tests/integ/qrexec_perf.py`
- `qubes/tests/integ/vm_qrexec_gui.py`

The current `release4.3` core-admin branch inspected was:

```text
1f407676892387a5113eec9dd663a945d395a223 refs/heads/release4.3
```

The `qrexec.py` integration file includes tests such as
`test_050_qrexec_simple_eof`, `test_060_qrexec_exit_code_dom0`, qrexec service
argument tests, socket tests, and VM-to-VM/dom0 qrexec behavior. These are the
right upstream tests to graduate toward once Nixome can boot the control stack
on a dedicated dom0 target.

Qubes also uses openQA for automated installation/system testing. The docs note
that Qubes-in-KVM installation testing is possible on suitable hardware with
VT-x and EPT, but nested HVM is unreliable in practice; PV is sufficient for
installation testing.

## Hardware And Test Boundary

Sources:

- https://doc.qubes-os.org/en/r4.3/user/hardware/system-requirements.html
- https://doc.qubes-os.org/en/r4.3/developer/debugging/automated-tests.html

Minimum real target expectations for the POC:

- x86_64 Intel or AMD CPU.
- VT-x with EPT or AMD-V with RVI.
- VT-d or AMD-Vi/IOMMU for non-PV/device-assignment work.
- At least 6 GB RAM for Qubes minimum, with 16 GB recommended upstream.
- Enough storage for dom0 plus at least one template and test qube.

What can be tested on a normal non-Xen development host:

- Nix flake evaluation.
- NixOS module contract and option assertions.
- Lightweight package builds such as `nixome-observe-dom0`.
- Host capability detection.
- Potentially some upstream Python unit tests once packaged for that purpose.
- A nested QEMU/KVM boot probe, if the host exposes nested VMX/SVM. This is a
  development signal only, not a production-quality Qubes security test.

What requires a booted Xen dom0 target:

- Xen runtime state via `xl`.
- `qubesd`, QubesDB, qrexec daemon/policy daemon runtime checks.
- Creating and starting a qube.
- `qvm-run --pass-io` and qrexec stdout capture.
- Full Qubes integration tests.

What likely requires physical hardware:

- Production-relevant Qubes security validation.
- Device assignment, sys-net/sys-usb isolation, IOMMU behavior.
- Reliable HVM testing.
- GUI/input/display integration beyond a narrow headless qrexec smoke test.
