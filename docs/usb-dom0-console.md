# Xen + NixOS dom0 console baseline

This separate, generic x86-64 UEFI image is the first physical boot checkpoint.
It uses the existing Xen/Qubes dom0 core with no guest topology, automatic VM
preparation, desktop, or machine-specific GPU configuration. Linux retains
its normal hardware detection and console support. There are no experimental
GPU boot entries. The ordinary non-Xen recovery entry is explicitly labelled.

```sh
make usb-dom0-console-image
make usb-dom0-console-write USB_SERIAL=YOUR_USB_SERIAL
```

The writer unmounts the identified USB and overwrites it with the console
image, using the existing sparse writer. This replaces previous personalization.
The fixed disk capacity is 12 GiB including a 3 GiB EFI partition for boot
generations and installer staging. This layout applies to newly built images;
existing installations are not repartitioned by a deployment. No partition
or filesystem expansion runs on boot. No personalization is needed: local
login is `airlock` / `password`. No repository or credentials are copied.

USBGuard is disabled only in this diagnostic profile: the inherited dom0
default-block policy otherwise blocks the USB root disk during boot. This is
a development-only relaxation of USB restrictions, not a production policy.
Validate this image with a virtual USB boot disk, not just virtio storage.

The persistent journal uses `SyncIntervalSec=1s` to reduce buffered log loss
during forced shutdowns. This changes disk synchronization, not log verbosity,
and cannot guarantee the final messages survive a kernel or storage failure.
Rebuild and rewrite the USB to apply this setting; older images are unchanged.

Boot the default Xen entry, log in, and run:

```sh
cat /proc/xen/capabilities
sudo xl list
systemctl --failed
```

Expected: `control_d`, only `Domain-0` in the Xen domain list, and a usable
console. Verify another cold boot before adding any guest VM.

For remote diagnostics, connect Ethernet or use `nmcli --ask device wifi
connect YOUR_SSID`, then `sudo tailscale up --ssh --hostname=airlock-console`.
Tailscale SSH requires the tailnet's SSH policy to allow the connection.
Alternatively install an SSH public key in `~/.ssh/authorized_keys` and use
OpenSSH over Tailscale. OpenSSH password authentication is disabled and port
22 is opened only on the Tailscale interface. The public development password
is intended for this console experiment.

QEMU can validate this image's UEFI/Xen boot and console login, but cannot
prove the laptop's firmware and physical display work. Physical boot remains
the acceptance gate. No internal disk filesystems are configured for mounting.

Validation on 2026-09-13: built this raw image and booted it through OVMF/QEMU
with a writable overlay. The VGA console showed a login prompt; serial console
login with `airlock` / `password` succeeded, `/proc/xen/capabilities` reported
`control_d`, `xl list` contained only Domain-0, and there were no failed units.
NetworkManager, tailscaled, sshd and qubesd were active. Logs and screenshot
are under `$AIRLOCK_LOG_DIR/usb-console-{build,qemu}.log` and
`$AIRLOCK_LOG_DIR/usb-console-screen.png`. The laptop is not yet validated.
