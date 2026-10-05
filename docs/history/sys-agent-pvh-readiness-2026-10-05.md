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
built the identical release in 215.9 seconds. Candidate admission and live
activation are pending; no successful deployment is claimed yet.

The pre-deployment dom0 boot ID is `eba7098a-1080-48eb-86d8-f4e53a658e97`;
sys-agent's boot ID is `6c755252-14e4-41c6-ba93-5172811aa677`, UUID
`a8c14e6d-05c6-412a-952d-28f41c910aca`, and virtualization mode HVM.
Tailscale and qrexec are active. The home guidance checksum is recorded for
comparison after restart. No temporary configuration overlay was applied.

Evidence: `.airlock-logs/sys-agent-pvh/` and
`.airlock-logs/machine-deploy/9d542bf2c531f07f296ffe10c7f0553f33106f3f/`.
