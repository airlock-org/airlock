# PSR mitigation and recovery tool deployment, 2026-10-12

Component revision `fe437e1fc973a7b4f23a8b66c59184f89d4515b7` adds the
ThinkPad physical sys-gui kernel option `amdgpu.dcdebugmask=0x10` and the manual
`airlock-gpu-recovery` tool. `make test-quick` passed with 899 passed and 59
skipped, including eight recovery tests. Nix syntax parsing passed. No
`test-nix` or deliberate hardware reset was performed.

Normal whole-machine preparation succeeded. The prepared host reused the
running host's exact `lxqz5hm8fzs08g9k87y09261ckfxxblp` initrd, avoiding another
large EFI payload. Apply transaction `1791813726723625741` restarted only
sys-gui, leaving sys-agent and usr-home running. After physical sign-in, both
application-VM GUI channels reconnected and the coordinator reached
`reboot-required` with no error.

Live evidence from the new sys-gui:

- Running system: `/nix/store/lldxm2rzc8nv0rzj5bl05lpybv3sk3d4-nixos-system-sys-gui-26.11`.
- Kernel remains Qubes `6.12.64-1.qubes.fc41.x86_64`; its command line includes
  `amdgpu.dcdebugmask=0x10` alongside the original Qubes/closure-store options.
- `/sys/module/amdgpu/parameters/dcdebugmask` reads 16.
- Internal eDP PSR capability reports sink support yes and driver support no.
- The boot kernel log showed no DMCUB or flip-timeout matches at inspection.
- The installed recovery command captured a private report at
  `/var/log/airlock/gpu-recovery/1791813893320187097.json` in sys-gui.
  No reset-attempt marker exists; the working GPU was not reset.

The selected host is
`/nix/store/fr876fcp25d8x7kxsx5qacc3fi34dscx-nixos-system-dom0-26.11.20260904.801bef6`.
The host is still running the previous retention generation, so whole-machine
post-reboot verification remains pending. The PSR mitigation is already active
in the live guest, but absence of early errors is not evidence of long-term
stability. Recovery without reboot remains an unmeasured fallback for the next
naturally occurring freeze, not a demonstrated fix.

Use [the recovery instructions](../gpu-display-recovery.md). Next checks are
post-reboot host/guest identity, effective PSR state, ordinary window
open/close/drag behavior, and idle/lock/unlock behavior. Observe power impact
and freeze recurrence before accepting the mitigation.
