# VM workspace visual prototype, 2026-10-02

Component `6eefad8`, pinned by parent `671ad8e`, adds the GNOME 50 extension
`airlock-vm-workspaces@airlock.local` to sys-gui. The panel shows VM names,
Qubes label colour dots, numbered workspace buttons and an adjacent-slot `+`.
New application windows receive one placement suggestion using the existing
trusted window identity reader. Moving windows between VM groups remains
unrestricted. A yellow `!` marks foreign or unidentified window contents;
the separate focused-VM indicator continues to identify the actual focused VM.
This prototype adds no enforcement, qrexec capability or guest-agent change.

Workspace objects remain attached to their logical slots when another group
grows. The extension temporarily uses static workspaces, retains existing
workspaces on first enable, and persists counts/order by VM UUID in the normal
user XDG state directory. VM renames retain slots; absent VM groups retain their
workspaces with an `Unavailable VM` label. The first prototype supports adding
and switching slots, up to 36, without removal or group-order controls. The
layout is confined to sys-gui and uses the existing app-menu inventory cache.

Local validation passed **832 quick tests, 59 skipped**, lint and Python
compilation. The tests cover UUID renames, persistence validation and limits,
contiguous insertion without changing neighbouring workspace objects, mixed
contents, unrestricted moves after initial placement, user movement during a
pending identity lookup, stale inventory callbacks, disable during lookup,
lock-screen hiding and preservation of externally changed preferences.

The whole-machine release built successfully:
`/nix/store/gmf4ilh882h9ydgvxijix2rqy5kdwz69-airlock-machine-release`.
The packaged extension includes the existing identity and inventory models,
its identity-reader substitution, stylesheet and GNOME 50 metadata; file hashes
were recorded. Guarded Airlock GC skipped collection below its threshold.

Source was pushed to the clean `/home/user/airlock-machine` checkout. Controller
`guest-plan` and `guest-apply --target sys-gui` selected only a sys-gui generation
change, restarting sys-gui, sys-agent and usr-home for activation. Detached
worker `airlock-machine-deploy-351d48a1a0da` owns transaction
`1790961280234058774`.

The worker finished inactive with `Result=success` and exit 0. The running and
selected sys-gui generation is
`/nix/store/5l10z8qxhnmg181z8hmp5pwjjgsz4r2b-nixos-system-sys-gui-26.11`.
The installed extension path and source SHA256 match the locally built package.
An early verification attempt, while the deployment worker was still verifying,
returned no result. After worker completion, `make machine-verify` succeeded:
all six running guest generations match their selections. Whole-machine state
retains the earlier `reboot-required` condition: host running `mckynwhd...` and
selected `9yrfxaq3...` are unchanged. No host reboot was requested for this UI
experiment, and no live configuration overlay or diagnostic mutation was used.

The desktop is at GDM. Session extension validation awaits the user's sign-in;
manual visual feedback on the panel and workspace controls remains pending.

Evidence: `.airlock-logs/vm-workspaces/` and the machine-build record beneath
`.airlock-logs/machine-deploy/671ad8ea518fdae19803052cf53f5dd8b843a097/`.
