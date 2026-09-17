# Physical GUI input

The reusable module `airlock-machine/lifecycle/misc/modules/qubes-input-proxy.nix` forwards
selected physical input devices from dom0 to a GUI domain. It packages
[upstream Qubes input proxy 1.0.46](https://github.com/QubesOS/qubes-app-linux-input-proxy/tree/ea8710166c6ac29dd4183cdf155bc9a1da34cf4a),
without changing its event protocol or filtering. It contains no GPU address,
AMD condition, laptop model or hardcoded input-device path.

Dom0 reads selected evdev devices. Qubes qrexec transports their events to
`qubes.InputKeyboard`, `qubes.InputMouse` or `qubes.InputTablet` in the GUI
domain. The receiver creates Linux uinput devices, which the guest's normal
input stack detects, including before login. Touchpads need the tablet filter
because they produce absolute events. This kernel interface is usable by Xorg
and Wayland compositors; only XFCE/Xorg has been exercised here.

The sender uses upstream's dom0 `qrexec-client -l` administrative interface.
That interface does not consult qrexec policy. Separate policy denies
VM-originated input requests to the selected GUI domain, and each receiver
checks that `QREXEC_REMOTE_DOMAIN` is dom0 before running as root. This does
not grant application VMs input injection or access to physical devices.
Future forwarding from a dedicated input VM requires an explicit extension
of the trusted-source configuration and policy.

## Configuration and activation

`airlock.inputProxy.sender.enable` defaults to false. When enabled, it
requires `sender.devices`: named entries with a stable `/dev/input/by-id/`
or `/dev/input/by-path/` evdev symlink and a `keyboard`, `mouse` or `tablet`
filter. `sender.target` defaults to `sys-gui` and can name another GUI domain.
It must already be running. There is no automatic device discovery, USB
controller assignment, guest startup or retry loop.

Device selection belongs to the machine configuration. The ThinkPad profile
declares its keyboard, TrackPoint and Synaptics touchpad; its touchscreen is
not selected. Only the AMD lab enables this sender. Normal hybrid USB,
console and VM profiles do not start it. Another machine supplies its own
stable device paths without modifying the shared implementation.

In the receiving guest, enable `airlock.inputProxy.receiver.enable`. The
physical GUI lab does this by default; the hybrid guest does not. This loads
`uinput` and installs root RPC handlers with the source check above. It also
provides `airlock.InputReceiverReady`, restricted to dom0, which succeeds only
when `/dev/uinput` and all three upstream receiver handlers are available.

The sender subscribes to Qubes lifecycle events. When its selected GUI domain
starts, one bounded coordinator call waits for `airlock.InputReceiverReady`,
confirms that the same domain is still running, and starts all declared sender
services. A domain stop releases the devices. This one path covers normal boot,
manual VM starts and deployment-driven replacement; deployment code has no
input-specific repair step. The coordinator never starts or retries the VM.

If debugging an already-running physical guest, run the same coordinator in
**dom0**:

```sh
sudo systemctl start airlock-gui-input-connect.service
sudo systemctl status 'airlock-gui-input-*.service'
```

Inspect individual services: the target being active alone does not prove
that every device connected. Missing paths skip their sender; they do not
cause selection of another device. Stopping the target releases the grabs:

```sh
sudo systemctl stop airlock-gui-input.target
```

Losing the qrexec connection ends the sender and releases its device. Sender
services use `Restart=no`: a later failure remains visible rather than entering
an endless retry loop. A subsequent Qubes start event runs the bounded
coordinator again. Automatic physical-device hotplug remains future work.
Never enable forwarding alongside the hybrid desktop's existing input path.

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
`lifecycle/image/tools/amd-preflight.py` and `tests/test_gui_minimal.py`.
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
