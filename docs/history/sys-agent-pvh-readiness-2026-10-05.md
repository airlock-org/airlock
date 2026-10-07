# Sys-agent PVH and deployment readiness, 2026-10-05

The investigation of the failed native Activities deployment found sys-agent
unresponsive after a hot restart, with qrexec and remote access unavailable until
host reboot. HVM was retained from its former physical-network-provider role;
its emulated GUI connection ended at the last guest console output. The precise
fault remains unproven. See the separate native Activities postmortem.

Component `09282ba` changes sys-agent to PVH. It has no PCI assignments and uses
sys-firewall for networking. Component `4586c91` makes deployment verification
share the bounded boot readiness loop: up to 180 seconds per required guest,
with bounded qrexec probes and retries for command failures/timeouts. A returned
wrong generation fails immediately. Failed verification prevents successful host
boot binding/selection and leaves the transaction journalled for recovery.
The parent pins both changes in `9d542bf`.

Validation passed **842 quick tests, 59 skipped**, lint and Python compilation.
Five new regression cases exercise transient Qubes state, qrexec connection and
process timeouts, eventual success, deadline failure, and immediate generation
mismatch through actual coordinator apply. Existing post-reboot checks continue
to distinguish required guests from the prior session's manually running VMs.
The Nix configuration check also asserts PVH; it was not run as a quick-test gate.

The normal local whole-machine build produced
`/nix/store/a4hdzjqv6bicklfza3phajisj0a0mig1-airlock-machine-release`, revision
`4586c91bdb98a8c4d1b13af77c1a867c72e6bb33`. Its topology declares sys-agent PVH,
and its coordinator is `k72c09ibr0dm4chs4v9h465r2dmz9l7d`. Guarded automatic GC
skipped below its threshold. The normal controller `make machine-plan` route
built the identical release in 215.9 seconds and prepared the candidate in
404.3 seconds. The plan restarts all six active guests, retires none and requires
a later host reboot. The normal `make machine-apply` reused preparation with no
further build or transfer. Candidate `fa501bba7a9e624b4e874f6fb0587e52ade4ae37dea94f0cbb4e6f596085e62b`,
worker `airlock-machine-deploy-3fa715d4d61c`, transaction `1791214406533086342`.

The retained worker journal confirms successful activation at 15:38:42 UTC.
The worker used the new coordinator; it passed staging, shutdown, configuration,
startup and exact-generation verification before binding/selecting the host.
Transient unit removal means its later `LoadState=not-found` default properties
alone are not success evidence. `make machine-verify` independently confirms
all six guests match the selected release. The state is **reboot-required**:
dom0 runs `9yrfxaq3g6x28ywkcjfwjdf9bqgv62r8`, and selected dom0 is
`aaq4px7jylgzpa21v4mg6phd2zlq7cr6`. Complete host verification is pending reboot.

The pre-deployment dom0 boot ID is `eba7098a-1080-48eb-86d8-f4e53a658e97`;
sys-agent's boot ID is `6c755252-14e4-41c6-ba93-5172811aa677`, UUID
`a8c14e6d-05c6-412a-952d-28f41c910aca`, and virtualization mode HVM.
Tailscale and qrexec are active. The home guidance checksum is recorded for
comparison after restart.

Qubes reports sys-agent **PVH** after activation, with its original UUID and
home guidance checksum. Tailscale restored remote access without enrollment;
qrexec, Tailscale and the GUI agent are active with no failed guest services.
The first new guest boot ID was `450113af-dc65-4861-949c-a5cd73f9135c`.

A second controlled restart exercised sys-agent alone while signed-in GNOME and
`qvm-start-daemon --all --watch` remained running. A temporary unprivileged
detached dom0 process called ordinary `qvm-shutdown --wait --timeout 120` and
`qvm-start`, recording both return codes as 0. Shutdown took 13.9 seconds;
startup took 8.5 seconds. The next SSH probe succeeded without enrollment.
Sys-agent's boot ID changed to `af2fe522-8478-4ca1-b8af-6bdc78760579`; the dom0
boot ID and GNOME shell PID 1426 remained unchanged. Qrexec, Tailscale and the
GUI agent remained healthy, the original home checksum and UUID still matched,
and a repeated whole-machine verification again matched all six guests.
This demonstrates recovery for this restart; it does not prove the earlier
HVM stall's precise cause or guarantee all future restarts.

The native Activities extension is now **ACTIVE** in GNOME, and its JavaScript
and CSS hashes match the built artifacts recorded in the earlier history.
Visual/click acceptance remains manual.

Diagnostic cleanup: the attempted transient user service could not launch
because dom0 has no user session bus and made no service changes. The detached
restart process has exited. Its temporary `/home/user/.cache/airlock-sys-agent-pvh-restart.log`
was captured and removed. No executable, policy edit, systemd override or
configuration overlay was left in the live machine. All persistent changes
come from the committed release. Final host verification was pending reboot at this observation.

Post-reboot verification on 2026-10-07 reports **complete** for transaction
`1791214406533086342`: dom0 runs selected host `aaq4px7jylgzpa21v4mg6phd2zlq7cr6`,
and all six guests match selection. Remote access returned after the operator's
restart. Evidence: `post-reboot-verify-20261007.json` in the directory below.
The subsequent [workspace bar deployment](vm-workspaces-compact-2026-10-07.md)
also completed its sys-agent restart and automatic remote-access recovery.

Evidence: `.airlock-logs/sys-agent-pvh/` and
`.airlock-logs/machine-deploy/9d542bf2c531f07f296ffe10c7f0553f33106f3f/`.
