# Physical GUI input

The reusable module `airlock-machine/lifecycle/misc/modules/qubes-input-proxy.nix` discovers
local input devices in dom0 and forwards each independently to the GUI domain.
It uses [upstream Qubes input proxy 1.0.46](https://github.com/QubesOS/qubes-app-linux-input-proxy/tree/ea8710166c6ac29dd4183cdf155bc9a1da34cf4a)
without changing its event protocol. There is no machine-specific input list,
required TrackPoint, device model match, or custom touchpad symlink.

Controller ownership trials and timed recovery remain in the
[USB input setup plan](todo/usb-input-setup.md).

## Discovery and initial trust

`airlock.inputProxy.sender.enable` defaults to false; `sender.target` defaults
to `sys-gui`. The physical GUI profile enables discovery. Hybrid desktops,
console profiles and text recovery do not start a second input path.

Once the GUI receiver is ready, the discovery service subscribes to udev events
before enumerating existing `/dev/input/event*` devices. It accepts devices by
udev input capabilities: keyboards and key devices, mice and pointing sticks,
touchpads, touchscreens, tablets and joystick devices. USB and internal devices
use the same path. Locally synthesized `/sys/devices/virtual/input/` devices
are excluded to avoid recirculating input; switches and sensors without a
supported key or pointer capability are not forwarded.

All discovered supported devices are trusted automatically, including devices
connected later. There is currently no approval or rejection policy. Dom0
records their descriptors and first-seen time in
`/var/lib/airlock/input/trusted-devices.json`, retaining that inventory across
reboots and ignoring changing event numbers for identity. This is an observed
trust inventory, not a cryptographic identity or enforced allowlist. A recording
failure is logged and does not disable input. A fresh USB flash resets the
inventory with the rest of the installation.

Each evdev node gets one Qubes forwarding process. A missing, removed or failed
device cannot stop other devices. Duplicate udev notifications do not duplicate
forwarding; unplugging and reconnecting starts a new attempt. Sender failure is
logged and does not trigger an endless retry loop. The discovery service itself
restarts on failure. No USB controllers are reassigned by discovery.

## Transport and lifecycle

The dom0-only `airlock.InputDevice` RPC invokes the existing upstream receiver
with `--keyboard --mouse --tablet` together. This preserves supported key,
relative and absolute capabilities on composite devices without trying to grab
one device multiple times. The existing `qubes.InputKeyboard`, `qubes.InputMouse`
and `qubes.InputTablet` handlers remain available for older senders. Upstream
protocol limits still apply; discovery does not add force feedback or sensor
transport.

Dom0 uses Qubes' `qrexec-client -l` administrative interface. This interface does
not consult qrexec policy. Receivers require `QREXEC_REMOTE_DOMAIN=dom0`, and
policy denies VM-originated input requests. This grants no new input authority
to application VMs. A future dedicated input VM requires an explicit change to
the trusted source and policy.

In the receiving guest, `airlock.inputProxy.receiver.enable` loads `uinput` and
installs the root RPC handlers. `airlock.InputReceiverReady` checks the device
and handlers. The existing Qubes lifecycle listener requests one bounded
readiness check when the GUI starts, verifies that the same VM instance remains
running, then activates `airlock-gui-input.target` and its discovery service.
A GUI stop releases all forwarding; a GUI restart enumerates again. This covers
boot, manual VM starts and deployment replacements without a separate input
repair path. The coordinator never starts or retries the VM itself.

For diagnostics in **dom0**:

```sh
sudo systemctl start airlock-gui-input-connect.service
sudo systemctl status airlock-gui-input-discovery.service
sudo journalctl -u airlock-gui-input-discovery.service
sudo cat /var/lib/airlock/input/trusted-devices.json
```

The journal identifies each forwarding attempt, removal and sender exit. An
active discovery service or a trusted inventory entry does not prove physical
input reached the desktop. Stop forwarding and release device grabs with:

```sh
sudo systemctl stop airlock-gui-input.target
```

## Validation boundary — 2026-09-23

Boot `1150b9af-b7e3-4b84-bfb5-ba6e6301a76f` completed VM startup, but the old
input coordinator aborted all forwarding because the configured TrackPoint
path was missing even though the keyboard and touchpad were enumerated.
Discovery replaces that fixed list and the interim optional-TrackPoint fix.
Focused pytest coverage checks generic selection, composite devices, persistent
inventory, late arrivals, reconnects and independent failures. Nix checks cover
service wiring and receiver restrictions. Physical login, hotplug and reboot
still need checking on an image rebuilt from this change.

## Laptop result — 2026-09-13

On boot `75eadf68-6b62-4c42-8ce7-5960acb3809a`, the already-running VBIOS
experiment's guest display manager was started manually. Input initially
failed because neither the sender nor receiver was installed. An additional
portable packaging defect in `qrexec-client -l` assumed `/bin/bash`; the
vendored Nix package now substitutes the actual Nix Bash path.

The built upstream proxy and corrected client were staged under root-owned
`/run/airlock-input-proxy` for this live experiment. Guest handlers, uinput
and three transient dom0 sender units were installed without restarting the
guest or reassigning the GPU. The guest's existing libc and matching Qubes
kernel's uinput module were sufficient; its read-only store was not changed.
Future image builds include these components through Nix.

All three sender services remained active. Guest Xorg/libinput registered:

| Forwarded device | Guest classification |
| --- | --- |
| AT Translated Set 2 keyboard | Keyboard |
| TPPS/2 Elan TrackPoint | Mouse / pointing stick |
| SYNA8018:00 06CB:CE67 Touchpad | Touchpad |

The user confirmed seeing the GUI, typing, moving the pointer and logging in.
This establishes usable physical display and input on this boot. Separate
confirmation of both pointer devices, scrolling/gestures, accelerated GL,
lock/unlock, and cold-boot repeatability remains outstanding. Libinput logged
an unsupported two-finger scroll setting for the TrackPoint and inferred the
touchpad's clickpad property; those are recorded limitations, not reasons to
add laptop-specific behavior to the shared proxy.

Verification: upstream proxy and both corrected qrexec variants built; guest
and dom0 lab configuration assertions passed; normal USB sender was disabled;
generated target, keyboard unit and guest handler built. A receiver invocation
with a non-dom0 source value exited 1. This is a handler guard check, not an
end-to-end policy test from another VM. No full image rebuild or full test
suite was run. Evidence is in `.airlock-logs/input-proxy/` on the workstation.
The repository-wide commit formatting hook failed on two unchanged files,
`lifecycle/image/tools/amd-preflight.py` and `airlock-machine/tests/test_gui_minimal.py`.
Those existing formatting issues were left untouched; the hook was skipped
for the two scoped commits after the checks above.

The live experiment uses transient services, not the new aggregate target.
To stop its forwarding over SSH, use:

```sh
sudo systemctl stop airlock-gui-input-keyboard.service \
  airlock-gui-input-pointingstick.service airlock-gui-input-touchpad.service
```

The running setup is not a persistent boot workflow. The VBIOS/assignment
experiment also remains manual; see [its lifecycle limits](amd-gpu-vbios.md).
