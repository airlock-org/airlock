# USB development track

Status: implementation in progress; no builds or tests authorized for this change.

Scope: ThinkPad T14 Gen 3 AMD (21CFCTO1WW), persistent removable development
system, dom0 + sys-gui only. No sys-gpu, inference or Hyprland work yet.
The existing VM profiles are unchanged.

## Delivery sequence

1. `usb-dev` NixOS configuration and `airlock-usb-dev-image`: EFI raw image,
   hybrid XFCE, direct dom0 Wi-Fi/Ethernet, Codex/Git/Zellij, Tailscale and
   key-only SSH restricted by the firewall to tailscale0.
2. Guarded USB writer, offline personalization and a writable Git clone;
   separate encrypted credential storage and import. Never build secrets into
   the image or a Nix derivation.
3. Explicit AMD experiment and recovery workflow; do not change the default
   display ownership until a real hardware run succeeds.

## Hardware evidence

Observed over SSH on the laptop, not inferred from the desktop:

- Ryzen 7 PRO 6850U, Radeon 680M, GPU `0000:04:00.0` (`1002:1681`).
- GPU alone in IOMMU group 15; **bus-only reset**. Other functions on bus 04
  include audio, USB, PSP and ACP. Isolation groups do not prove reset safety.
- Built-in eDP panel; PS/2 keyboard/TrackPoint and I2C input devices.
- Qualcomm QCNFA765 Wi-Fi (`ath11k_pci`), Realtek Ethernet (`r8169`).
- Internal 1 TB NVMe must remain untouched; no USB target selected yet.
- Secure Boot was disabled.

## Acceptance gates (not yet run)

1. Build/evaluate the image and review its closure and boot entries.
2. Select an expendable USB SSD (at least 128 GB recommended), personalize,
   boot via the firmware's removable-media menu. No internal installation.
3. Confirm Xen, Qubes lifecycle, XFCE, inputs, networking and Tailscale SSH.
   Cold reboot and confirm repo, VM state and logs persist.
4. Confirm recovery from a text-only boot before any GPU experiment.
5. Reinspect PCI ownership/reset topology under Xen; attempt passthrough only
   with remote access and an explicit go/no-go decision on reset safety.

The raw OS filesystem is writable and unencrypted. Credentials require a
separate LUKS vault; this does not protect the OS against offline tampering.
Treat this as a developer system, not production Qubes security.
