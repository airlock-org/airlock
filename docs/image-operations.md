# Image operations

Run these commands from the parent airlock checkout with its initialized
component at the committed pin. The source must be clean. Set `AIRLOCK_DATA_ROOT`
to the external development store location.

## Build and provision

```sh
make usb-dev-image
lsblk -o NAME,TRAN,SIZE,MODEL,SERIAL,MOUNTPOINTS
make usb-dev-provision USB_SERIAL=YOUR_USB_SERIAL
```

Provisioning erases the selected USB disk. Verify its serial; the provisioner
unmounts its partitions before writing. `USB_PUBLIC_KEY` selects the SSH public key; provisioning
prompts for the dom0 password. The provisioner writes the image, grows its root
filesystem, and installs the committed source and key. It leaves internal disks
alone. If it reports stale partition information, unplug/replug and restart
provisioning; do not improvise partition changes or resume a partial write.

`make usb-builder-offline-check` explicitly repeats the pinned offline-builder
trials. Image assembly runs its own input and storage checks.

The provisioner's partition backups are recovery metadata for the newly written
image, not backups of the USB's previous data. Keep them outside the checkout.
They are written below the configured log directory or `/var/log/airlock`.

## Boot recovery

Keep the machine's screen and keyboard available for hardware changes. If the
normal graphical entry cannot start, select the text recovery entry in the
boot menu. Inspect the failed services and deployment status before attempting
resume or rollback. Preserve diagnostics for an unresolved failure.

Changing boot profiles does not remove persistent Qubes PCI assignments. Do not
start sys-gui with a display GPU still owned by dom0 or delete Qubes state to
force a boot. Inspect the declared hardware configuration and Qubes assignments
from recovery before changing ownership.

For an active display stall, the GUI service keeps the bounded
[display recovery procedure](../crates/airlock-sys-gui/service/src/gpu/README.md)
beside its tool.
