# Lab AMD display mitigation and recovery

The ThinkPad T14 Gen 3 AMD physical sys-gui configuration disables panel
self-refresh with `amdgpu.dcdebugmask=0x10`. This is a mitigation trial for
the recorded DMCUB and page-flip stalls, not proof of their cause or a general
AMD setting. Generated Qubes and closure-store kernel options are preserved.
The setting requires a new sys-gui boot. Verify the effective command line,
module value and eDP PSR capability after deployment. Keep ordinary window,
idle and lock/unlock observation separate from generation verification.

`airlock-gpu-recovery` is installed in physical lab sys-gui. It uses the existing
root administrative route and adds no Qubes RPC, policy rule or automatic
watchdog. When the physical panel freezes but SSH to sys-agent still works,
enter dom0 with `airlock-dom0-shell` and become root. Capture evidence first:

```sh
timeout 20 qrexec-client -d sys-gui \
  'root:airlock-gpu-recovery capture' </dev/null
```

During a confirmed display freeze, request one in-driver recovery:

```sh
timeout 90 qrexec-client -d sys-gui \
  'root:airlock-gpu-recovery recover --confirm-display-stall' </dev/null
```

The command requires exactly one amdgpu-owned `1002:1681` device and a display
flip timeout in the previous five minutes. It writes pre-reset evidence before
triggering the exact GPU's debugfs recovery node. Merely reading that node
triggers a reset and loses work on the GPU; it is not an observational file.
One attempted reset is allowed per sys-gui boot. The reset subprocess inherits
the operation lock. A timed-out kernel call may continue running despite a
termination request; timeout is not recovery success and blocks escalation.

If the reset command finished successfully but the desktop still does not
respond, explicitly restart GNOME's display manager once:

```sh
timeout 90 qrexec-client -d sys-gui \
  'root:airlock-gpu-recovery restart-desktop --confirm-display-stall' </dev/null
```

This ends the sys-gui desktop session. Sign in again and verify application-VM
GUI reconnection. Application VMs are not shut down by this command, but their
visible windows may require relaunch/reconnection. Do not claim recovery until
the physical panel and pointer work. If either operation fails or times out,
preserve its report and stop. The tool never resets a PCI bus, reassigns a GPU,
restarts a VM or reboots the host. Reboot/cold-power recovery remains a fallback.

Reports are private root JSON files in sys-gui's persistent
`/var/log/airlock/gpu-recovery/`; attempt markers and locks live under
`/run/airlock/gpu-recovery/`. Successful command return does not set
`physical_recovery_verified`: physical acceptance must be recorded separately.
Future log retention must preserve unresolved incident evidence.

The implementation and automated tests do not establish that the GPU reset
recovers this laptop. Use the next naturally occurring freeze for the first
measured attempt; do not deliberately reset the working desktop. Investigation
and upstream references are in [the lifecycle TODO](todo/amd-gpu-lifecycle.md).
