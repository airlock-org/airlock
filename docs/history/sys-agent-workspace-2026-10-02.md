# sys-agent Console workspace acceptance, 2026-10-02

Console development workspace (2026-10-02): component `fdfca6b` assigns
sys-agent to sys-gui and installs the existing Qubes application GUI support,
GNOME Console, Codex, Make, Python, ripgrep, uv, and Zellij. Console is published
through generic application discovery. Codex state stays on the private home
volume; its global `AGENTS.md` links to generation-managed instructions under
`/etc/airlock/sys-agent/`. Existing user guidance is preserved. The checkout's
instructions now document the standalone controller deployment route as well
as the external parent/submodule workflow. No credentials are provisioned.

Validation passed **802 quick tests, 59 skipped**, including initialization,
generation-following guidance and preservation of custom files/symlinks. A
focused Nix configuration check confirms GUI routing, readiness, installed
tools, private-state environment and guidance. Lint and compilation passed.
The normal local build produced
`/nix/store/24zjalkp8nfszwbrmdwgckwrymsy8nsn-airlock-machine-release`, bound to
component `fdfca6b` and parent `44a5ba9`. Guarded automatic GC skipped collection
below its threshold. Operator instructions are in
`airlock-machine/docs/sys-agent.md`.

Normal `make machine-plan` built in 142.1 seconds and prepared in 196.2 seconds.
The plan restarts all six guests and selects updated host metadata requiring a
reboot. It reused both root artifacts and transferred no root payload. Normal
`make machine-apply` reused prepared token `6f3b6cf4...` with zero build/transfer
work, submitting worker `airlock-machine-deploy-e6950a120a62` for candidate
`708c0a26...`.

The worker exited 0 with `Result=success`. Transaction
`1790939877500803355` verifies all six running guest generations against their
selections. The host remains `mckynwhd...`, with `9yrfxaq3...` selected for the
next reboot; whole-machine verification reports `reboot-required`.
Live sys-agent has Codex 0.151.0, Zellij 0.45.1 and Console 50.0. Its workspace,
GUI agent and builder services are active. Private Codex state has mode 0700
and the generation-managed guidance link. No Codex credentials were imported.

After GNOME login, its automatic menu timer is active and refresh exits 0.
GLib search resolves the UUID-based `sys-agent: Console` launcher to its cached
512x512 application icon and `qubes.StartApp+org.gnome.Console`. A normal
launch succeeds. The packaged menu reconciler was run once to inspect discovery
before login; this only refreshed normal application caches.

The first live quick suite exposed two mocked configuration fixtures using the
real Qubes runtime directory: 708 passed, 35 skipped, 228 deselected, 2 failed.
Component `d865073` isolates their socket paths from the live runtime.
The operator then reported the running Console icon missing. Read-only window
inspection found `kgx` inside sys-agent and correctly forwarded
`sys-agent:kgx` in sys-gui, while its launcher incorrectly fell back to
`sys-agent:org.gnome.Console`. Component `0f40092` infers a missing
StartupWMClass from the original guest executable metadata, preserving explicit
application hints. There is no Console-specific rule or forwarding change.
Nine additional regression cases bring the local quick suite to
**811 passed, 59 skipped**. Lint and compilation pass. The final live quick
suite passes **719 tests, 35 skipped, 228 deselected** from the committed
sys-agent checkout, including both corrected configuration fixtures.

The follow-up local build produced
`/nix/store/i56ljrbrf5j2j97mk78ccs7966g1fr5b-airlock-machine-release`, bound to
component `0f40092` and parent `090e40d`. Guarded GC again skipped collection
below its threshold. The documented single-guest controller plan/apply route
updated usr-home, then sys-agent, preserving sys-gui and the selected host.
Workers `airlock-machine-deploy-c4b1e4815d9c` and
`airlock-machine-deploy-c1e3d9277a50` both exited 0 with `Result=success`.
Final transaction `1790941875954591923` verifies all six running guests against
their selections. Sys-agent runs `3clzq6j3...`; usr-home runs `r9620gyz...`.
Whole-machine verification still reports `reboot-required` for the unchanged
pending host selection from the initial workspace deployment.

After each guest restart, GNOME's automatic refresh published its updated
Console launcher without a manual cache refresh. Both use their VM-prefixed
`kgx` class. A normal sys-agent Console launch produces window `0x2400054`
with `WM_CLASS("sys-agent:kgx", "sys-agent:kgx")`, matching its launcher, and its
cached application PNG exists. The timer is active and its service exits 0.
No live configuration overlays or diagnostic configuration mutations were used.
Visual icon acceptance and host verification after reboot remain pending.

Evidence: `.airlock-logs/sys-agent-workspace/`.
