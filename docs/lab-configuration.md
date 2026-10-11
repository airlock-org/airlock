# Lab configuration and other hardware

The named machine and USB outputs retain the existing ThinkPad T14 Gen 3 AMD
lab example. They are not a hardware-discovery installer. Inspect hardware and
GPU/IOMMU topology before building or booting a physical image.

## Development-host settings

Set `AIRLOCK_DATA_ROOT` to your own storage location before invoking Make or the
Nix wrapper. The existing `/run/media/user/data/airlock` mount is an example
fallback, not a required username or mountpoint. The isolated store lives under
`$AIRLOCK_DATA_ROOT/nix-root`; VM disks, caches and logs have separate subdirectories.
You can override `AIRLOCK_LOG_DIR`, `AIRLOCK_VM_DIR`, and `AIRLOCK_RESULT_LINK_DIR`.
Quick tests need no store and fall back to `.airlock-logs/` when no data root exists.

`USB_SERIAL` selects exactly one expendable USB disk. `USB_PUBLIC_KEY` selects
one SSH public key and defaults to `~/.ssh/id_ed25519.pub`. Provisioning prompts
for a dom0 password; `USB_INSECURE_DEV_PASSWORD=1` explicitly selects the public
development password. No application credentials are provisioned.

`DEPLOY_TARGET` is the SSH target for your enrolled sys-agent. Tailscale setup
is unchanged: enroll interactively after first boot; the private VM volume
retains enrollment across updates. No enrollment key is embedded in source or images.

## Machine hardware

The component's `lib.mkMachineConfiguration` accepts `hardwareModules` and
`extraModules`. Its default hardware module is
`lifecycle/image/hardware/thinkpad-t14-gen3-amd.nix`. Replace that list when
constructing another machine; append normal configuration through `extraModules`.
For example, in your own flake consuming airlock-machine:

```nix
nixosConfigurations.my-machine = airlock-machine.lib.mkMachineConfiguration {
  hardwareModules = [ ./hardware.nix ];
  extraModules = [ ./local-settings.nix ];
};
```

Declare `airlock.guiGpu.device` in your hardware module: `bdf`, the matching
Qubes PCI `port`, `vendor`, `device`, and `driver`. The normal physical GUI
assignment derives its port from this declaration. The GPU module validates the
selection and requires an explicit device when enabled. Supplying no device does
not make the physical GNOME profile usable; use a suitable nested/hybrid profile
for a machine without that assignment.

Review firmware, initrd modules, input forwarding, display scaling, storage,
network device discovery, and boot/recovery support together. Preserve upstream
Xen/Qubes mechanisms. Modules do not establish physical compatibility by themselves.
The parent USB example still uses the default machine; create a deliberate
machine/image profile for other hardware and verify it through the existing workflow.
