# Application preservation across sys-gui updates, 2026-10-07

Machine component `fa48bfa`, pinned by parent `905d040`, supports restarting the
same assigned sys-gui without restarting capable, unchanged application VMs.
Their GUI agents preserve Xorg, detach display buffers while the GUI VM is
absent, retarget grants to the trusted QubesDB GUI domain ID, and replay existing
windows. The planner removes only the eligible GUI restart dependency;
networking, hardware and other provider dependencies retain the existing policy.
VMs without the reconnect capability, changed VM generations and changed GUI
assignments still require the previous restart behavior.

The coordinator records preserved guest boot IDs and checks them alongside the
new GUI domain ID and connection marker. Verification retries are bounded;
failure reports an incomplete deployment without restarting preserved guests.
GNOME supervises the upstream GUI forwarding watcher as a user service, with
its qrexec transport explicitly supplied in PATH. It retries early session/API
readiness failures instead of leaving application launchers disconnected.

The initial whole-machine installation, component `06288e1`, completed as
transaction `1791388405192059145`, worker
`airlock-machine-deploy-9a128ca805db`. Verification after the operator's reboot
confirmed the selected host and all six guest generations. The final corrected
GUI-only deployment used the documented controller guest-plan/guest-apply route:

- Transaction: `1791390751134003897`; worker
  `airlock-machine-deploy-93ac789ee70e`.
- Candidate: `baab4b38c60d1996bbc3ac1314eb07afdc1744c6f2bf3aec2bd168f3837dc549`.
- Only sys-gui stopped and changed, to
  `/nix/store/w5m3r3a4yv99par677nlrkcljimv0n1r-nixos-system-sys-gui-26.11`.
- `make machine-verify` reports **complete**: all six guests match selection;
  host running/selected remain
  `/nix/store/4l499576ybgdcwf44k6kclif80zxml40-nixos-system-dom0-26.11.20260904.801bef6`.

Physical acceptance preserved sys-agent and usr-home boot IDs, Xen domain IDs,
and the exact PIDs and start times of both GUI agents, both Xorg servers, Console
and Firefox. A single SSH connection sampled once per second throughout the
GUI-only update and exited successfully: 234 samples, maximum sample interval
1.003 seconds. The operator signed in without relaunching applications and
confirmed both original windows returned. Both agents report `connected 13`.
The persistent GUI watcher is active; workspace extension version 10 is enabled
without errors, with both VM groups populated and no pending launches.

The first GUI-only attempt exposed a missing qrexec executable in the watcher
service PATH. It reached the bounded reconnection timeout while preserving
guests and applications. The fix is committed in `fa48bfa`; ordinary GUI-only
rollback transaction `1791390220287228402` recovered the previous selection
before the final corrected deployment. Two temporary diagnostic starts of the
installed upstream watcher restored forwarding during investigation. No live
configuration or store files were edited. The transient recovery unit was
removed by GUI restart and its absence verified after final activation.

`make test-quick` passed **850 tests, 59 skipped**. Packaged coordinator validation
passed; the native buffer-retarget test checks stable addresses and pixels,
detached drawing and allocation-failure recovery. Physical reconnection also
exercised real Xen grant detachment and retargeting. Guarded `make gc-auto`
skipped collection below its threshold. Evidence, JSON acceptance summaries and
test reports are under `.airlock-logs/gui-reconnect/`.

This acceptance covers restarting the same assigned GUI VM. Workspace groups
are reconstructed from returning windows; exact workspace positions and focus
are not persisted across GUI restarts.
