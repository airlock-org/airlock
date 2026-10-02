# sys-agent home workspace deployment, 2026-10-02

Component `c7a9820`, selected by parent `102ddda`, moves sys-agent's editable
checkout to `~/airlock-machine` and installs a brief `~/AGENTS.md` introduction.
The introduction links to checkout instructions and the existing detailed
generation-managed guidance. Startup moves the complete old checkout before
source seeding, preserving Git state and local files. Existing destinations
and user-written guidance are preserved. Source import, first-boot seeding,
builder defaults and deployment clients use the configured checkout location.
The controller keeps its own existing checkout location.

Validation passed **824 quick tests, 59 skipped**, lint and compilation, and
three focused Nix checks for workspace configuration, first-boot source
seeding and packaged source/build tools. Regression coverage exercises relocation with uncommitted work,
guidance overrides, destination collisions and explicit deployment paths.
The local release build produced
`/nix/store/nknf3gz00ssk9xydns30db9s956qjli9-airlock-machine-release`.
Guarded automatic GC skipped collection below its threshold.

The committed source was pushed to sys-agent. Normal controller
`guest-plan` / `guest-apply --target sys-agent` changed and restarted only
sys-agent. The initial migration worker `airlock-machine-deploy-d3c7eabe06c1`
exited 0. A follow-up corrects shared source import to create the configured
checkout's parent directory, including the controller's existing nested path.
Final worker `airlock-machine-deploy-916aec7e44db` exited 0 with
`Result=success`. Transaction `1790949355064001773` verifies all six running
guest generations against their selections. Sys-agent runs
`/nix/store/dj08sgz5phsz89izskd2bpvfpry5i50s-nixos-system-sys-agent-26.11`.

After reconnect, the workspace service reports active, success, exit 0.
`~/AGENTS.md` links to `/etc/airlock/sys-agent/home-AGENTS.md`, and Codex's
existing guidance link follows the updated detailed instructions. The old
checkout path is absent; the new checkout has the expected revision and no
tracked or untracked changes. Source status and normal builder preflight both
resolve `/home/user/airlock-machine`. The optional offline preflight reports no
pinned offline flake inputs in this guest-only generation.

The existing `.venv` contained launchers with the old absolute path.
`uv sync --locked --reinstall` refreshed its pinned dependencies, and the pytest
launcher now points to the new checkout. The live quick suite passed
**724 tests, 43 skipped, 229 deselected** from the relocated checkout.
After the final restart, all **30 focused live workspace/deployment checks**
passed, preflight uses the new default path, and the checkout is clean.

The host remains `mckynwhd...`, with the earlier workspace deployment's
`9yrfxaq3...` selection awaiting reboot. This guest update preserves that
selection; whole-machine verification still reports `reboot-required`.
No live configuration overlays or diagnostic configuration mutations were used.

Evidence: `.airlock-logs/sys-agent-home/`. Operator guidance:
`airlock-machine/docs/sys-agent.md`.
