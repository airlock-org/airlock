# Machine deployment lab evidence through 2026-10-09

Historical notes moved from the deployment runbook on 2026-10-11. They retain
trial-specific results and pending checks as recorded. Follow the
[current runbook](../machine-deployment.md) for operations and
[current status](../status.md) for validation boundaries. Local evidence paths
refer to the operator's logs, which are not included in Git.

Usr-home khaki label deployed, host reboot pending (2026-10-09):
component `a5e2270` declares label khaki and managed feature
`airlock-label-color=0x6b7a3d`. The existing setup converger creates this label
through Qubes admin.label.Create before assigning it; an existing conflicting
colour is rejected. Standard labels remain untouched and no new RPC authority
or deployment route was added. The border width remains 10px. Shell syntax
parsing and the controller build succeeded; no tests were run.

Normal whole-machine plan/apply selected candidate
`465cea6c0913023d95d48e79a64c0f0b6866e954d6b4ad77e0899e915b54af60`,
transaction `1791503679918108115`. Only usr-home restarted, with no system
closure staging; sys-gui and sys-agent stayed running. Live label is khaki,
and the new usr-home GUI daemon PID 7566 uses colour 0x6b7a3d and label index 9.
Calculator was reopened. Whole-machine verify reports **reboot-required**,
with all six guest systems matching selection. Selected host is
`s7y5vd30f6n4a30r2jwnjzlf8hsvgccc`; running host remains
`sb7i9bnyvmp0w4bim6y4bzag35zvahwy`. Operator was told reboot is now ready;
post-reboot verification and physical colour acceptance remain pending.
Evidence: `.airlock-logs/usr-home-khaki/`.

Gray-label post-reboot verification and GUI recovery (2026-10-09):
normal `make machine-verify` now reports **complete**, with selected host
`sb7i9bnyvmp0w4bim6y4bzag35zvahwy` running. The operator reported usr-home
could not start its graphical session. It was running with its fixed agent,
but no usr-home GUI daemon/Xorg session was attached. User-journal evidence
shows the GUI watcher initially failed before DISPLAY was ready, restarted,
then lost its qubesd connection during boot with Request refused responses.
This is a startup/reconnection issue; its exact causal sequence remains under
investigation. Missing icon-path warnings also occur, but do not prevent the
recovered daemon starting.

Restarting only the existing user `airlock-gui-daemons.service` rediscovered
both running application VMs. Usr-home daemon PID 3728 uses `-c 0x555555 -l 5`;
its Xorg PID 1572 started, and Calculator was reopened. No VM, GNOME or host
restart, configuration overlay, or process-memory write was used. This is
operational recovery, not a claimed permanent startup-race fix. Physical
Calculator acceptance is pending. No tests were run. Verification evidence:
`.airlock-logs/usr-home-gray/post-reboot-verify.log`.

Usr-home gray label deployed, host reboot pending (2026-10-09): after
operator approval of the broader prepared plan, normal `make machine-apply`
reused the admitted candidate without a rebuild or payload transfer. Transaction
`1791502047384920876` completed guest activation and GUI reconnection after
the operator signed in. `make machine-verify` reports **reboot-required**:
all six running guests match selection. Live `qvm-prefs usr-home label`
returns **gray**. Sys-gui now runs `93fmcjj6n7k7ksf0rn5lxk4ha6c48pxq`;
usr-home and sys-agent retain their fixed agent systems. The selected host is
`sb7i9bnyvmp0w4bim6y4bzag35zvahwy`; the running host is still
`gmdygzm515zaa4m7m84yxmwaqd7biwc3`. The operator was told it is now time to
reboot. Whole-machine post-reboot verification remains pending. No custom
update route was added, and no tests were run. Evidence:
`.airlock-logs/usr-home-gray/apply.log`, `status.json`, and `verify.log`.

Usr-home gray label prepared, not applied (2026-10-09): component
`b600a0a` changes both the default personal VM declaration and bootstrap runtime
configuration from blue to standard Qubes gray. The 10px border width is unchanged.
Normal `make machine-plan DEPLOY_TARGET=user@SYS_AGENT_ADDRESS` built and admitted
candidate `fe031d68a5031828a9dd0a80b3e8bb283c1de2d5e90593f2543a04c1d4910350`.
The concrete plan stops usr-home, sys-agent and sys-gui, stages all three,
and reports host_reboot_required=true. Existing separate guest updates mean
this whole-machine candidate has a broader scope than the requested label.
No apply was submitted: the operator prefers avoiding restarts, and sys-gui's
recent AMD failures make this particularly relevant. The live label remains blue.
No tests were run. Evidence: `.airlock-logs/usr-home-gray/plan.json` and `plan.log`.
A narrower durable label deployment route or approval of the broader plan is
needed before live activation; do not report the committed source as deployed.

Black display recurred (2026-10-08): the recovered sys-gui boot
`926e271c-74a9-46f1-8f33-65f3f5ebd883` developed AMD fence fallback timeouts
at 22:45:05 UTC and a flip_done timeout at 22:45:16. GNOME workspace-index
assertions occurred at the same time; their causal relationship is unknown.
Both application VMs already contain the generic input notification fix.
SSH/qrexec remained responsive. PSR was supported but its observed state and
residency were zero; no speculative graphics parameter changes were made.

Ordinary sys-gui shutdown timed out after 120 seconds, leaving the domain
Transient without qrexec. A bounded qvm-kill stopped only sys-gui, followed by
ordinary qvm-start. New boot `f6b825a0-cd0d-4c98-906e-879894fefb06` failed AMD
PSP hardware initialization with error -22 and GPU mapping/walker errors;
no GNOME process started. This recovery therefore **failed**. The operator
was asked for the documented cold-power-cycle fallback. The host and both
application VMs remained running throughout these operations. No configuration
overlay or manual PCI reset was used. No tests were run. Evidence:
`.airlock-logs/black-screen-recurrence/`. Physical recovery remains pending;
the recurring GPU failure is unresolved.

Sys-agent Console input recovery deployed (2026-10-08): the old agent also
hit the reconnection deadlock after the graphics recovery. Read-only channel
metadata showed a full 4096-byte incoming ring with write notifications disabled.
A diagnostic SIGCHLD through its existing handler drained the ring and armed
notifications without a restart; the operator confirmed typing and dragging
worked again. No runtime configuration overlay or process-memory write was used.

The existing committed fix `0de6d4c` was built and deployed through controller
single-guest plan/apply. Only sys-agent restarted. Transaction
`1791499464910648864` selected candidate
`59bc4e4ffc3fd3007c4d0ea56ffcf5b80d36cb36702e55e64b038ca6df4700eb`.
Whole-machine verification reports **complete**. Sys-agent runs
`yv9hkm6jg9mm4b0gx9nqlvwrw657d9sc`, with agent package
`irxg1cbbhal2h6wmw70nyzjbs1gy23v2`; Console was reopened. Its input ring was empty
with notifications armed. GNOME retained PID 1411 and sys-gui boot ID
`926e271c-74a9-46f1-8f33-65f3f5ebd883`; usr-home retained boot ID
`5bfa934d-33b5-48d6-8c50-ee806f91e043`. No desktop or host restart was needed.
No tests were run. Evidence: `.airlock-logs/sys-agent-input-freeze/`.

Black display recovered (2026-10-08): the operator reported a completely black,
unresponsive panel after the Calculator deployment. SSH and GNOME remained
responsive; the session was locked and display power saving was active. Neither
setting display power on nor waking GNOME's lock screen restored the panel.
Restarting only display-manager produced a running greeter but still no image.
Kernel logs showed AMD fence fallback timeouts from 22:22:57 and a `flip_done`
timeout at 22:23:06, before usr-home activation was submitted at 22:23:41.
Generation verification did not establish physical display health.

After checking the only running GUI clients (sys-agent/usr-home) and their
reconnect features, a bounded ordinary sys-gui shutdown/start restored the
physical greeter. No GPU ownership change, manual PCI reset, host reboot or
configuration overlay was used. The operator signed in and confirmed the
desktop and Calculator both work. Sys-gui boot ID is now
`926e271c-74a9-46f1-8f33-65f3f5ebd883`; both application VM boot IDs and Calculator
PID 966 were preserved. The fixed usr-home input ring was empty with write
notifications armed after this real reconnection. New sys-gui boot logs had no
graphics timeout matches at inspection; whole-machine verification remained
**complete**. The underlying AMD graphics failure remains unresolved; this was
recovery, not a permanent GPU fix. No tests were run. Evidence:
`.airlock-logs/black-screen-recovery/`.

Calculator input recovery deployed (2026-10-08): component `0de6d4c` checks
GUI vchan readiness before waiting, including after the reconnect checkpoint.
Readiness also arms Xen write notifications. Live observations found a full
4096-byte client-to-agent input ring with notifications disabled: GNOME and
the daemon received clicks/keys, while usr-home's agent only polled. This was
not CPU pressure: usr-home had 2 vCPUs, load 0.03 and 3319MiB available RAM;
sys-gui had 4 vCPUs on an 8-core host.

A diagnostic SIGCHLD to the existing agent interrupted its wait, drained the
queued input and armed notifications. The operator confirmed both Calculator
buttons and title-bar dragging recovered without a restart. The temporary
diagnostic strace copy in sys-gui `/run/airlock-guid-strace` was removed before
deployment; no process-memory writes or configuration overlay were used.

The controller build succeeded; no tests were run at the operator's request.
Normal controller single-guest plan/apply then installed the durable fix in
usr-home, restarting only that VM. Transaction `1791498221099446980` selected
candidate `9314593a95a9dda5d3396135dbc4738f9b6ef2e7fc076d913f81413a69c3c9fe`.
Whole-machine verification reports **complete**, with the host and all six guest
systems matching selection. Usr-home now runs `rzj55dy19qks3pcy2w5srgr11ipcl315`
and agent package `irxg1cbbhal2h6wmw70nyzjbs1gy23v2`. Calculator was reopened.
Sys-gui retained boot ID `d849a246-6bdc-47f4-be12-e132a3eb31f5`, GNOME PID 1449
and its 10px border setting; no desktop or host reboot was needed. Other guests'
agent packages remain unchanged by this single-guest deployment. Evidence:
`.airlock-logs/calculator-input-freeze/`.

Live GUI style route deployed (2026-10-08): component `9622f29` makes the trusted
VM colour width reloadable through the GUI daemon's existing SIGHUP path,
redrawing forwarded windows. The fixed black outer edge remains 1px, widths
are restricted to 1–16px, and tray icons remain excluded. The Nix source is
`sys-gui/desktop/gnome/gui-style.json`, currently 10px. A root-owned durable
selection and journal live under `/rw/airlock`; the booted Nix generation
restores its declared default at sys-gui startup.

Parent `machine-gui-style-plan` / `machine-gui-style-apply` use the existing
password-protected development dom0 shell and administrator qrexec transport,
without adding production control authority. Apply only changes this style,
signals supported daemon processes via pidfds, and records the source revision;
it does not restart VMs, select a host, or replace the running OS identity.
Plan describes the desired change; a reload-requested receipt is not physical
acceptance. Unsupported running daemon packages are rejected before mutation.

No tests were run at the operator's request. Python syntax parsing and the
controller build succeeded. After the operator rebooted into the previous 3px
release, a fresh controller single-guest plan/apply installed the reload support
with one sys-gui restart. Transaction `1791496641801964133` selected candidate
`b59824d9f487ff409251db537947c581b5ede39c37b025518148cc35b35d246a`.
Whole-machine verification reports **complete**, with the host and all six
guest systems matching selection. Sys-gui runs `yw2dbjplm08dqm7xbq57ph9jcgbmn9q4`.
The host boot ID remained `6aa4ab21-3d07-419c-a175-19737938a497`, and sys-agent
and usr-home reconnected with their original boot IDs. No host reboot was needed.

The new `machine-gui-style-apply` route then applied the committed 10px setting
and returned `reload-requested` for both supported daemons. Sys-gui's boot ID,
GNOME PID 1449 and daemon PIDs 2542/2591 stayed unchanged. Live Calculator
window `0x1c00055` identified itself as usr-home; a horizontal pixel sample
confirmed one black outer pixel followed by exactly ten blue `#3465a4` pixels.
Physical visual acceptance remains with the operator. Runtime selection uses
the committed helper and its durable journal; no ad-hoc overlay was used. Evidence:
`.airlock-logs/gui-style-reload/`. This route handles border settings, not
arbitrary live sys-gui binary upgrades.

VM border contrast deployed live (2026-10-08): component `22904f7` gives every
forwarded VM window a 1px black outer edge and 3px of its trusted Qubes label
colour. Tray icons remain excluded. The GUI daemon allocates a separate black
graphics context and retains application-owned chrome. No tests were run at
the operator's request; the controller build completed successfully after a
patch-header correction. Changes and parent pointers were committed before
normal whole-machine plan/apply.

Initial transaction `1791487168215437058` timed out awaiting GUI reconnection.
Normal resume transaction `1791490315607184785` succeeded after GNOME login.
Verification reports **reboot-required**, with all six guests matching their
selections. Sys-gui runs `4d3rhbq04rcdy7fwnxrmpn3rd4l7yaax` and its sys-agent
and usr-home GUI daemons both run package `3d88wvkycp707x67il5j0filpkxj0k5p`.
The selected host is `gmdygzm515zaa4m7m84yxmwaqd7biwc3`; the running host remains
`vjki1cdlwqkhlz2j6bh7kvynvc4iawxf`. Live visual acceptance and subsequent
whole-machine boot verification remain pending. No runtime configuration
overlay was used. Evidence: `.airlock-logs/window-border-contrast/`.

Stock GNOME applications deployed to usr-home (2026-10-08): component
`f472c43` opts this guest out of the minimal profile and uses the pinned
NixOS GNOME core application selection. GNOME Web replaces Firefox, Loupe
provides Image Viewer, and normal documentation, Nano, completion and convenience
packages return. Nix/channels, installer tools and Flatpak remain disabled.
Component `84bd4cb` adds standard Mesa/EGL software rendering after the first
live WebKit launch exposed missing EGL drivers. No GPU assignment was added.
Every other role's evaluated system derivation remained identical to the
pre-change source. The parent pointers were committed before source transfer.

The quick suite passed **871 tests, 59 skipped**. All four focused personal-GUI
configuration checks passed; the stock application/default-package comparison
passed again after the graphics change. The final local usr-home build is
`/nix/store/kz7v8k989f573rw5dj1phhhg02fjc57r-nixos-system-usr-home-26.11`.
The documented controller single-guest plan/apply route changed only usr-home,
with no host reboot. The host and the five other guest generations were
preserved. The host and other guest boot IDs matched after the first activation;
sys-agent's boot ID was also confirmed unchanged after the graphics recovery.

A diagnostic `qvm-run` accidentally auto-started usr-home during the second
activation, so the coordinator stopped with deferred configuration work.
All diagnostic calls were changed to `--no-autostart`. Normal
`make machine-resume` recovered the activation; transaction
`1791461355992655519`, worker `airlock-machine-deploy-1258e395527c`, completed
successfully. Its selected candidate is
`874dfadbd8a00937b8f52dad3ae4360c28521fc71d8dadb3a5ff7a776177be6b`.
The controller reports **complete**, with the host and all six running guest
systems matching selection. The running usr-home exactly matches the local
build above. No live configuration or policy overlay was used.

Live checks confirmed automatic application menu publication including Web and
Image Viewer, no Firefox launcher, expected image/PDF/HTTPS MIME defaults,
no failed system/user units in usr-home, and persistent private storage across
updates. A PNG opened in Loupe and the local smoke HTML page loaded in GNOME Web;
forwarded windows retained their usr-home identity. WebKit's sandbox was retained.

The pre-existing cross-VM `airlock.OpenUrl` launcher is still broken: sys-gui
puts a raw URL in the service argument, and the pinned Qubes daemon sanitizes
`:` and `/` to underscores before the receiver validates the scheme. This
requires a separate sender/receiver transport fix; no sys-gui update or restart
was attempted in this pass. Browser HTTPS MIME selection itself is correct.
Physical audio/camera/scanner integration and native capture delivery were not
accepted by these application smoke checks.

Before final diagnostic cleanup, Tailscale SSH required interactive
reauthentication, blocking new connections. The temporary persistence sentinel
`~/.local/state/airlock/usr-home-stock-gnome-sentinel`, smoke directory
`~/.local/state/airlock/usr-home-stock-gnome-smoke/`, and owned user transient
units named `airlock-stock-smoke-*` still need cleanup in usr-home once access
returns. The final boot-ID comparison and post-cleanup verification are pending;
the complete generation check above precedes this access interruption.
Guarded `make gc-auto` skipped collection below its threshold. Evidence:
`.airlock-logs/usr-home-stock-gnome/`.

Failed boot investigated and corrected release prepared (2026-10-08): the
saved initrd evidence for boot `56146084-0929-4b18-b0f2-33ff303900e9`
confirms `host generation has no machine boot binding`. Menu generation 34
(`6c04c37`, host `c47s40197s7qpbap1qdfqm5fn11mqriy`) uses host-store generation
35, whose `machine.json` is absent. Both capture transactions timed out at
GUI reconnection before host binding/selection. The recovered menu generation
33 remains running and the persistent default; its selected deployment reports
complete. No live configuration mutation was used.

Normal `make machine-plan DEPLOY_TARGET=user@SYS_AGENT_ADDRESS` prepared corrected
component `4e83114` as candidate
`/var/lib/airlock/machine/candidates/6c55cefd2165dfcb67a2e1af148c06229df9a3363eb3f8c04b42f3afb80b77ba`,
token `b43ede10b98e6d35c9e9226931e348e36edf69246c958fafdb4be86b1ee20d90`.
It changes/restarts only sys-gui and usr-home, preserves sys-agent for GUI
reconnection, retires no VMs, and requires a subsequent host reboot. All guest
generations and roots were reused. Preparation completed without activation.
Normal apply reused the prepared candidate with zero builds/transfers. After
the operator logged into the restarted GNOME session, sys-agent reconnected to
GUI XID 9. Transaction `1791450959865485535`, worker
`airlock-machine-deploy-cb21da499c93`, succeeded with exit 0. Normal verification
reports reboot-required with all six guests matching selection. Menu generation
35 (`4e83114`, host `vjki1cdlwqkhlz2j6bh7kvynvc4iawxf`) is the persistent
default; host-store generation 36 has the exact successful transaction binding.
A normal host reboot loaded the selected host; post-reboot verification reports
**complete**, with all six guests matching. Boot ID
`e731b7c5-dad4-4a46-923c-6726dff0cf1b` reached supervised readiness in 201.5
seconds, and dom0 has no failed units. Menu generation 35 is now verified live.
The guarded prune plan for failed generation 34 remains blocked by unfinished
transactions `1791393510320779863` and `1791394533580047023`; no pruning or
manual transaction alteration was performed. Working generation 33 is retained.
Native capture delivery still requires its separate functional acceptance.
Guarded `make gc-auto`
skipped collection below its threshold. Evidence:
`.airlock-logs/boot-failure-20261007/`.

GNOME capture handoff prepared (2026-10-07): component `6c04c37` hooks the
native screenshot save path and successful screencast EOS, queues completed
files, and imports them into usr-home over dedicated qrexec services. Native
notification actions open or reveal the received file in usr-home. Login retries
pending captures and imports existing captures once; there is no capture timer
or filesystem watcher. Original files remain in sys-gui as recovery copies.

The normal whole-machine plan/apply installed sys-gui generation
`g9dv1wcsxji1pmmd3dw8kadz2cmagj67` and usr-home generation
`g14y990191g9iyylx0i1xpkivydw90ak`. Transaction `1791393510320779863`
failed at GUI reconnection because sys-gui remained at the greeter; normal
resume transaction `1791394533580047023` timed out at the same boundary. An attempted
handoff retained the existing recording in the sender queue. A direct RPC
probe returned `Request refused`: the earlier GUI policy's default deny
shadowed the new topology-managed allow rules.

Component `4e83114` places the four capture permissions before that deny in
`50-airlock-sys-gui.policy`, restricted to sys-gui targeting usr-home with
`+image` or `+video`. The regression check uses Qubes' packaged policy parser.
Validation passed **871 quick tests, 59 skipped**, all **21** capture transfer
tests, and **11** packaged interface/minimal-GUI checks. Lint and compilation
passed. The corrected normal local machine build produced
`/nix/store/mmcnpix5f0x350yahw3339v4ajhm76pa-airlock-machine-release`.
Guarded automatic GC skipped collection below its threshold.

This is not yet verified live capture delivery. Finishing the interrupted
transaction requires a GNOME login after resume restarts sys-gui to reconnect
the preserved sys-agent windows. Preparation of the corrected release is
blocked by the unfinished transaction. It then needs normal plan/apply and a host reboot
to activate its static Qubes permissions. Native screenshots of the desktop
and overview, video recording, file delivery and notification actions remain
pending. No live policy overlay or diagnostic configuration edit was used.
Evidence: `.airlock-logs/gnome-capture/`.

Automatic application discovery deployed to guests (2026-10-01): component
`c323fba` replaces the fixed personal application launchers with discovery for
VMs assigned to sys-gui. Guest RPCs use active system/user profiles and XDG
desktop entries consistently for discovery, launching and icons. The GUI
refresh timer retains stopped-VM catalogs, withdraws deleted/retired/reassigned
VMs, and preserves launcher IDs across VM rename using UUIDs. New Nix VMs
default to `menu-items = "*"`; explicit selections remain available. Browser
URL activation uses the chosen VM's MIME default rather than a Firefox command.

The final quick suite passed **796 tests, 59 skipped**, including 26 new
application discovery/lifecycle tests. Six checks against the packaged Qubes
receiver/menu code passed, as did the three personal/desktop configuration
checks and the minimal-GUI configuration check. A stale boot-supervisor test
was corrected to check the desktop-ready hook in the split startup stage.
Commit-hook lint and Python compilation passed. The normal local
`make machine-build` produced
`/nix/store/br2f0dza4qcp83csd2dcxdvcal3qqx8l-airlock-machine-release`, bound to
component `c323fba` and parent `6d13a92`. Guarded GC remained below its automatic
threshold.

The normal controller `make machine-plan` / `make machine-apply` route built the
same release in the live session and activated transaction
`1790888403410125759`. Worker `airlock-machine-deploy-bd79c9d6f6d8` exited 0 with
`Result=success`. `make machine-verify` checked all six guest selections against
their running generations. The host selection is
`/nix/store/mckynwhdk9bjn1d52zgf0cjk0s988dyv-nixos-system-dom0-26.11.20260904.801bef6`;
After the physical reboot on 2026-10-02, verification confirmed this host was
running and reported `complete`. No VMs were newly retired.

The controller build took 199.7 seconds and preparation took 273.1 seconds.
Host transport sent eight paths (457,608 NAR bytes; 119,349 compressed bytes).
Guest admission sent 67 package objects (23,328,768 payload bytes), reused every
root artifact, and sent no root payload. The two unique root artifacts total
1.5 GiB logical and 4,136,960 allocated bytes. Apply reused the prepared token
with zero further build/transfer work. Live guest RPC discovery returned Firefox
and 19 GNOME application entries; usr-home has `menu-items = "*"` and its
readiness feature enabled. sys-agent networking returned and its failed-unit
list was empty. sys-gui was at its greeter after restart, with the refresh timer
installed and enabled but awaiting a graphical session. This was the pre-login observation; post-login validation and corrections are
recorded below. No runtime configuration mutation was used.

Post-reboot functional validation (2026-10-02) found and fixed three gaps:
component `a5b3aba` uses the permitted `admin.vm.tag.List` read instead of the
denied membership query; `6e89b24` projects `share/icons` and `share/pixmaps`
into headless application profiles; `4ecca40` renders SVGs with librsvg before
GraphicsMagick probes them. All fixes are generic. Two retirement-listing
regression cases bring the quick suite to **798 passed, 59 skipped**. A
packaged icon-RPC test passed with the Contacts, Logs and Fonts SVGs that failed
GraphicsMagick's original parser. The final normal local machine build produced
`/nix/store/1ki33v0v2vz3qnzlf45nnwjksh7jsgh5-airlock-machine-release`.

The documented controller single-guest plan/apply route deployed the reader to
sys-gui and the profile/rendering fixes to usr-home, preserving the running host
and other guest selections. Its plans required only sys-gui/usr-home restarts
and no host reboot. Final transaction `1790936410103085952`, worker
`airlock-machine-deploy-d973b68bbf5d`, exited 0 with `Result=success`.
`make machine-verify` reports **complete** with the host and all six guests
matching their selections. sys-gui runs `fnmzbs2n...`; usr-home runs
`8mvqhzaw...`. The candidate records the separate guest source revisions.

After GNOME login, the automatic timer published usr-home's catalog and refresh
exited 0. All 20 application entries now have cached application icons; the
reserved Start qube entry uses its VM theme icon. GLib sees 21 total entries,
19 visible in GNOME, and searching Firefox returns the UUID-based usr-home
launcher with its application icon and `qubes.StartApp+firefox` target. The three
problem SVG RPCs each returned a valid 512x512 RGBA payload. The packaged
`airlock-sync-qube-appmenus usr-home` command refreshed cached fallback icons
after each icon fix rather than waiting for the five-minute icon interval.
No live configuration overlays or diagnostic configuration edits were used.
GUI launching, favorites, window grouping and a second application VM still
need manual acceptance. Evidence: `final-live-verify.log`,
`final-published-menus.log`, `final-svg-rpc-refresh.log`, `post-reboot-summary.json`
and the `tag-list-*`, `icon-profile-*`, `svg-icons-*` logs in the directory below.

Icon fidelity correction (2026-10-02): Firefox's published icon was a blue,
16x16 image. Component `7a90952` preserves application colours in upstream
appmenus instead of applying the VM label tint. Byte comparison replaces
existing tinted cache files even when their timestamps are newer, while
unchanged files retain their timestamps. The same correction applies to every
application VM. A pyxdg patch fixes missing icon-theme size defaults and scalable
size matching/distance, so the generic guest resolver selects Firefox's supplied
128x128 icon. No Firefox-specific discovery or launcher rule was added.

Validation passed **798 quick tests, 59 skipped**, and all **nine** focused
packaged interface checks, including the real Firefox RGBA receiver and cached
icon replacement. Lint and Python compilation passed. The normal local machine
build produced `/nix/store/qvjs0hiylnysg43650bqy2rbhlll6bvp-airlock-machine-release`,
bound to component `7a90952` and parent `ef4c4b5`. Guarded automatic GC skipped
collection below its threshold.

The documented controller single-guest plan/apply route updated usr-home first,
then sys-gui, keeping the running host and other guests selected. The final
transaction `1790938179646689025`, worker `airlock-machine-deploy-772283cba526`,
exited 0 with `Result=success`. `make machine-verify` reports **complete** for
the host and all six guests. usr-home runs `9qc77q7l...`; sys-gui runs
`4w4ykaf1...`. No host reboot was needed.

The packaged `airlock-sync-qube-appmenus usr-home` command refreshed the cache
without waiting for its five-minute interval. All 20 published application PNGs
match their original source caches byte for byte. Firefox's icon is 128x128,
and visual inspection confirms its normal colours. GLib search resolves the
usr-home UUID launcher to this PNG. After login the automatic refresh timer is
active and its service exits 0. No live configuration overlay or diagnostic
configuration edit was used. Evidence: `icon-colour-*.log` and
`firefox-icon-before.png` / `firefox-icon-after.png` in the directory below.

Manual acceptance after deployment should cover GNOME search/icons, two VMs
with Firefox, favorites and window grouping, install/removal or profile
switches, launching from a stopped VM, and VM rename/retirement. New VMs need
their first startup before their catalog is available; build-time catalog
seeding is not implemented. Normal healthy refresh rounds repeat after thirty
seconds. Evidence: `.airlock-logs/application-menus/`; component design and
diagnostic commands: `airlock-machine/docs/sys-gui.md`.

Connected personal desktop and agent handoff deployed (2026-10-01): component
`e8dd762` creates `sys-agent`, copies the stopped `sys-dev` private volume,
retains its home and former builder cache, and recoverably retires `sys-dev`
and `usr-browser`. Firefox and URL launchers now run in connected `usr-home`
through `sys-firewall`. The quick suite passed **770 tests, 59 skipped**.
Normal plan built in 321 s and prepared in 377 s; apply reused the candidate
with zero further builds/transfers. Worker `airlock-machine-deploy-9019252a4495`
succeeded, transaction `1790869300566467587`, with all six running guests
matching selection. Firefox reports 155.0; usr-home DNS and an external TCP
connection succeeded. Its original UUID is unchanged. Sys-agent reconnected at
`SYS_AGENT_ADDRESS`, its builder and Tailscale services are active, and no guest
units failed. A copied private-data marker matched, and the old builder cache
is preserved at `/rw/airlock-builder-sys-dev-builder`.

After the operator's restart, the handoff transaction verifies **complete**:
running/selected host both match `rw6krh25y2qrn2wfi55p99l4xp6d4sx6`, all six
guests match, and renamed sys-agent management access works. The builder and
Tailscale survived reboot. Usr-home DNS and an external TCP connection passed
again; its Firefox profile was still uninitialized, confirming a fresh start.
A temporary unprivileged dom0 observer relayed the existing controller outcome,
worker exit status and home network checks across SSH disconnection. Its script
and remaining cache files are removed. The source's Tailscale enrollment was
copied into its private volume before handoff; this intentional data preparation
is reproduced in sys-agent's declarative private-state bind. The active agent's
test marker and relay file are removed. Upstream `qvm-appmenus --remove` cleared
the retired sys-dev and usr-browser entries.

Component `d0fa841` clears the completed handoff declarations and is also
committed, pinned and deployed. Normal cleanup plan built in 84 s and prepared
in 95 s, with no changed guests or stop list. Apply reused it with zero further
builds/transfers. Worker `airlock-machine-deploy-12b0adcc8299` succeeded;
transaction `1790870916451415092` verifies **reboot-required**, with every guest
matching and unchanged VM states, XIDs and UUIDs. Selected cleanup host is
`6cb9j09q630qvqsg6z6mm7wpgchbkhbs`; the running host is the verified handoff
host above. Its metadata will activate at the next normal restart. No runtime
configuration overlay remains. Cleanup-host verification after that restart
and manual Firefox acceptance are pending. Evidence:
`.airlock-logs/home-default-agent/`.

Application-owned GUI chrome deployed (2026-10-01): machine revision `f52366c`
applies one GNOME policy to every forwarded window: no outer title bar or frame,
and no GUI-daemon colour border. Application chrome remains inside its VM;
the trusted focused VM panel indicator is the identity display. Quick tests
passed **750 tests, 59 skipped**, and commit-hook lint passed. Normal plan
built the release in 407 s and prepared it in 193 s, changing only sys-gui.
Apply reused the prepared release with zero further builds/transfers and
restarted sys-gui and its running usr-home/usr-browser dependents. Worker
`airlock-machine-deploy-6bf7d90f1d29` succeeded; typed verification of transaction
`1790864713181133896` reports **reboot-required**, with all seven running guests
matching their selections. Sys-gui runs `7gafnhba97jg6pz7p8j7plf387jfnjdl`;
selected host is `i5sq1cvrk4ivi38r02vijmf9kahjqwn0`, while dom0 still runs
`vxa5n5j8nyql4bkdafikd2mnr0px1v05`. Live Calculator carries the no-decoration
Motif hint, has no outer frame-extents property, and preserves `usr-home`
metadata. The panel extension is **ACTIVE**, and the operator reported
"that looks good". No diagnostic configuration mutation was used. Broader
move/resize/maximize, dialog and login/lock acceptance and host verification
after reboot remain pending. Evidence: `.airlock-logs/gui-no-decorations/`.

Usr-notes trial cleanup deployed (2026-10-01): the operator requested removal
after acceptance. Normal rollback worker `airlock-machine-deploy-e6d2d2e10cd3`
succeeded, transaction `1790853218723382023`, retiring only usr-notes. Upstream
`qvm-appmenus --remove usr-notes` removed its desktop entries. Component
`1255f94` documents the existing purge protections and supplies the next
committed no-trial release; it changes no VM declaration or runtime code.
Normal plan built it in 89 s and prepared it in 50 s, with no changed guests
or stop list. Apply reused the candidate with zero builds/transfers; worker
`airlock-machine-deploy-55b38e27a455` succeeded. Typed verification of transaction
`1790853674535062419` reports **reboot-required**, selecting host
`x6fqp88gsd44sfgq202ads7p1k1d2bk0` and preserving the no-trial baseline as the
immediate rollback choice. Existing VM states, XIDs and UUIDs are unchanged.

Usr-notes remains stopped in recoverable trash, record
`1790853218723382023-usr-notes`. The effective trial-release prune plan waives
only its unexpired-trash deadline and reports one remaining blocker:
`running or booted host`, because dom0 still runs the trial's
`p08d4fmqhpy199ay2jff5952w039q88f`. A normal host restart is needed before
safe trial pruning and permanent purge. No protection was bypassed and no data
was purged. Evidence is under `.airlock-logs/live-vm-trial/boot-hang-20261001/`,
including `usr-notes-removal.json`, `usr-notes-cleanup-plan.json`,
`usr-notes-cleanup-verify.json` and `usr-notes-removal-prune-before-boot.json`.

Live VM addition acceptance complete (2026-10-01): after the no-reboot addition,
a normal host restart loaded selected host `p08d4fmqhpy199ay2jff5952w039q88f`.
Usr-notes remained halted as declared; an explicit start found the same private
volume and exact home sentinel. Normal rollback recoverably retired only
usr-notes, and normal reapply restored its original UUID, volume and sentinel
without another reboot or restarting other VMs. Final transaction
`1790849824713060611` verifies **complete**, with all seven running guests matching
selection, usr-browser halted and no active trash records. The operator
confirmed the desktop and application. [Plan 12](../completed/12-live-vm-additions.md)
is complete; the committed experimental declaration remains on the separate
trial branch. Evidence: `.airlock-logs/live-vm-trial/boot-hang-20261001/`.

No-reboot VM addition verified (2026-10-01): the separate committed usr-notes
trial (`4e36175`) passed normal plan/apply. The plan changed only usr-notes and
stopped no VMs; worker `airlock-machine-deploy-be0e7c5dfa81` succeeded. Typed
verification of transaction `1790848522636123019` reports **reboot-required**:
the running host remains `4v44y6607wp1c087dgkl8id5pj8cxnrw`, with unchanged boot
ID, while host `p08d4fmqhpy199ay2jff5952w039q88f` is selected for later boot.
Existing guests kept their states, XIDs and UUIDs. Sys-gui imported the new
launcher live, the operator saw Text Editor, and an exact home sentinel survived
usr-notes stop/start with the same private volume. Host reboot persistence and
rollback/reapply remain pending. See [TODO 12](../completed/12-live-vm-additions.md#no-reboot-addition-verified-2026-10-01)
and `.airlock-logs/live-vm-trial/boot-hang-20261001/usr-notes-live-acceptance.json`.

Recovery baseline boot verified (2026-10-01): the operator restarted normally
and confirmed the desktop looks good. Dom0 now runs selected host
`4v44y6607wp1c087dgkl8id5pj8cxnrw`, boot ID
`b1866698-e8d6-42a9-b6a2-e27d064e387b`. Typed verification of transaction
`1790847116434715227` reports **complete**; all six running guests match their
selections and usr-browser is halted. The generic candidate-preparation and
admission RPCs are present. The separate usr-notes addition is now being
prepared; no further host reboot has occurred. See [TODO 12](../completed/12-live-vm-additions.md#baseline-boot-verified-2026-10-01)
and `.airlock-logs/live-vm-trial/boot-hang-20261001/`.

Live VM addition baseline failure (2026-10-01): saved USB diagnostics confirm
worker `airlock-machine-deploy-5b42fb9f7eb7` failed in `host-boot-selection`,
transaction `1790786804188630165`. `seal-boot` rejected a previously sealed boot
entry without source provenance after the affected guests had stopped. Boot
installation had already installed host `frp44hh7h9g5bv54khr4x5kys3mhgsnn`;
its subsequent startup rejected the bound failed transaction and entered initrd
emergency mode. The host store assembled successfully before this guard, and
the filesystem check reported clean after journal recovery. The operator has
booted the earlier `h8v0fz89xmdxjz5sh6ppnr7cyciv71j8` generation. Normal typed
verification reports transaction `1790781773748219122` **complete**, with all six
running guests matching their selections; usr-browser is stopped. Dom0 boot ID
is `dc910c03-fb3f-4bab-bae6-baa80e649d4f`.

Machine commits `e0f9081` and `0935cde` preserve authenticated retained entries
without source labels and move boot preparation before guest shutdown. Upstream
loader edits are staged in its existing private mount namespace, then published
with the previous default preserved. Preparation first restores the booted
host's verified default, including after a manual recovery boot. Sealing keeps
that default; target binding and explicit boot selection follow guest
verification. New entry formatting still requires committed provenance.

The quick suite passed **735 tests, 59 skipped**; targeted boot/deployment tests
and lint also passed. Normal plan built revision `0935cde` in 235 s and prepared
the candidate in 117 s. Apply reused it with zero further builds/transfers.
Worker `airlock-machine-deploy-2d09764d26d0` exited successfully; typed verification
reports transaction `1790847116434715227` **reboot-required**, with all six running
guests matching their selections and usr-browser stopped. Selected host is
`4v44y6607wp1c087dgkl8id5pj8cxnrw`; running host remains `h8v0fz89xmdxjz5sh6ppnr7cyciv71j8`.
The journal confirms boot preparation completed before guest shutdown, then host
selection followed verification. An initial host boot is needed to load the new
admission RPCs; the separate usr-notes trial has been rebased and rebuilt but
remains unapplied.

The final binding, boot selection and transaction completion remain separate durable writes, so abrupt power loss
in that final window can still require recovery; this is not an atomic machine
transaction. The usr-notes trial remains unapplied. See
[TODO 12](../completed/12-live-vm-additions.md#live-baseline-installation-attempt-2026-09-30)
and `.airlock-logs/live-vm-trial/boot-hang-20261001/`.

Focused VM indicator cache follow-up (2026-09-30): machine revision `b867b9a`
caches verified identities for each live window and removes entries on closure
or extension disable. Repeated switches avoid both reader startup and the
intermediate `Unknown VM` text; a new Xwayland window still needs its first lookup.
The quick suite passed **665 tests, 58 skipped**, including a regression with
200 repeat switches and no additional reader calls. Normal plan/apply built and
deployed the revision, the worker exited successfully, and GNOME reports the
extension **ACTIVE** with no extension errors. The operator retested and reported
that it looks good. No live overlay or diagnostic configuration mutation was used.

Typed verification of transaction `1790781773748219122` is **reboot-required**:
all seven running guest systems match their selections, including sys-gui
`24ymjdyixagdsx6q4smyaqbf0643x3q4`. Host-generation verification after reboot and
the broader GUI acceptance matrix remain pending. Evidence is under
`.airlock-logs/gnome-focused-vm/window-cache/`; see the
[indicator plan](../completed/gnome-focused-vm-indicator.md).

Focused VM indicator Calculator follow-up (2026-09-30): machine revision
`3750010` fixes application-actor and Shell focus-transition handling after the
operator reported that Calculator hid the indicator. Calculator's Qubes metadata
and the packaged identity reader were already correct. The quick suite passed
**664 tests, 58 skipped**. Normal plan/apply restarted sys-gui and its running
usr-home/usr-browser dependents; GNOME reports the extension **ACTIVE**, and
the operator confirmed the Calculator case works after the update. No live
overlay or diagnostic configuration mutation was used.

Typed verification of transaction `1790780281071143141` reports
**reboot-required**, with all seven running guest systems matching their
selections, including updated sys-gui `54dhx8c2018szghq3qlqk0bpdqccy7np`.
Host verification after reboot and the broader GUI acceptance matrix remain
pending. Evidence is under `.airlock-logs/gnome-focused-vm/calculator-fix/`;
see the [indicator plan](../completed/gnome-focused-vm-indicator.md).

Focused VM indicator early deployment (2026-09-30): machine revision `eb939a1`
was built and applied through normal whole-machine plan/apply using the
development VM's discovered SSH target. Only sys-gui changed; activation
restarted sys-gui and its running usr-home dependent. The quick suite passed
**663 tests, 58 skipped**. GNOME 50.4 reports the declaratively installed
indicator enabled and **ACTIVE**, the packaged metadata reader resolves Files
to `usr-home` with its blue label colour, and the operator tested it and reported
that it looks good. No live overlay or diagnostic configuration mutation was used.

Typed verification of transaction `1790778349293663479` reports
**reboot-required**: all seven running guest systems match their selections,
including sys-gui `cj7n3xrxkc7w86plv439rv1d5rq5karn`. Selected host is
`n2y35ad8hn3yz6j05rsx1475q5hvmnrs`; running host remains
`dyy6c32zx6fanzfrxyzgriw4rchihhwy`. Host verification after reboot and the
detailed GUI acceptance matrix remain pending. Evidence is under
`.airlock-logs/gnome-focused-vm/`; see the
[indicator plan](../completed/gnome-focused-vm-indicator.md).

Boot-menu acceptance (2026-09-30): the operator restarted the laptop and confirmed
the menu looks fine with the final `consoleMode = "1"` setting. Post-reboot typed
verification reports transaction `1790768363786110491` **complete**: running and
selected host both resolve to `dyy6c32zx6fanzfrxyzgriw4rchihhwy`, all six running
guest systems match their selections, and usr-browser remains stopped. The
[boot-menu plan](../completed/10-boot-menu-presentation.md) is complete, with the
physical manual older-generation/default-restoration trial explicitly untested.
Evidence: `.airlock-logs/boot-screen-consistency/verify-after-reboot.json` and
`acceptance.json`. No deployment or live configuration change was needed for
this documentation closeout.

Historical boot-screen size follow-up (2026-09-30): the operator confirmed bootstrap/startup
text is good and the 80x25 boot menu is too large. Machine revision `5bd3a92`
changes only the menu to `consoleMode = "1"` (UEFI 80x50 trial); startup font
configuration stays unchanged. The observed startup framebuffer is 3840x2400
with 240x75 console cells (inferred 16x32 glyphs). Firmware and Linux render
text separately; exact apparent size matching remains a physical comparison.

Normal plan/apply installed `console-mode 1` in the live ESP without guest
changes or restarts. Apply reused its prepared candidate with zero builds and
transfers. Transaction `1790768363786110491` is verified **reboot-required**:
six running guests match their selections, usr-browser is stopped, selected host
is `dyy6c32zx6fanzfrxyzgriw4rchihhwy`, and running host remains
`mq1z3vjikp3ynnc4ryrx64f3csplc26y`. The worker completed successfully. Quick tests
passed **629 tests, 58 skipped in 5.60 s**, with no Nix evaluations or automatic
tool setup; storage-tool-dependent checks skipped because those tools were not
on PATH. No new test was added for this configuration adjustment. No live
configuration mutation was used. Evidence: `.airlock-logs/boot-screen-consistency/`
and [completed boot-menu plan](../completed/10-boot-menu-presentation.md). Next boot must confirm the
menu/startup size comparison and the selected host generation.

Store preparation follow-up (2026-09-30): machine revision `3efe570` is committed,
pinned and applied through the normal controller route. Typed verification of
transaction `1790766764373029803` is **reboot-required**: all six running guests
match their selections, usr-browser is stopped, and the controller mounted the
incremental builder index (generation 17) with both inherited and appended host
files readable. Selected host is `fhrlj5cxg5lnc0xjw5rwv7bzn7zrbalh`; running host
remains `mq1z3vjikp3ynnc4ryrx64f3csplc26y`. No live diagnostic mutation was needed.
Host reboot verification remains pending.

The small follow-up reused 2,660 controller index objects and appended four.
Controller preparation took **17.04 s**, versus **149.68 s** on the initial full
migration; index construction fell from **129.37 s to 0.62 s**. Total preparation
was **94.21 s**, compared with the earlier small boot-menu update's **274.13 s**;
building took **53.39 s** separately. Cache states differ, so these are observed
runs rather than a controlled benchmark. The host still rebuilt its full index
(**35.35 s**). Apply reused the prepared candidate with zero builds/transfers.

`make test-quick` now excludes Nix evaluations/builds and automatic tool setup:
**665 passed, 22 skipped in 6.00 s**, with evaluators blocked and installed EROFS
tools on PATH. Nix checks are opt-in via `make test-nix`, not a routine deployment
gate. Before this workflow change, the full suite passed **742 tests, 25 skipped**
in **220.92 s**. Evidence: `.airlock-logs/store-preparation/` and
[performance notes](../todo/08-deployment-metadata-validation-reuse.md).

Boot-menu cleanup follow-up (2026-09-30): machine revision `e196ef8` passed
**733 quick tests, 25 skipped** and normal plan/apply without guest restarts.
Thirteen eligible release records were pruned through the deployed coordinator;
all legacy normal menu entries are gone, while recovery is preserved. Installed
normal order is **13, 12**, with generation 13 first/default and `console-mode 0`
(80x25) configured for larger text. Transaction `1790762690675027745` is verified
**reboot-required**: selected host `mq1z3vjikp3ynnc4ryrx64f3csplc26y`, running
host `0hhav9125qp9smac62ys6zmb3qhzp0c1`, and all six running guests match their
selections. Generation 12 remains protected as the running/rollback host.
The operator declined source-provenance backfill. Physical text-size/default
confirmation and generation 13 host verification remain pending. Evidence:
`.airlock-logs/boot-menu-cleanup/` and [completed boot-menu plan](../completed/10-boot-menu-presentation.md).

Boot-menu deployment (2026-09-30): machine revision `8048650` passed **733 quick
tests, 25 skipped** and normal plan/apply through sys-dev and sys-ctrl.
Transaction `1790759936024504141` is **reboot-required**: six running guests match
their selections, with usr-browser stopped. The selected host is
`0hhav9125qp9smac62ys6zmb3qhzp0c1`; the running host is still the previously
verified `1r34d0azzpnc70kw5qmppwm3g30hlfcf`. Bootctl confirms generation 12 is first
and default, titled `Airlock (2026-09-30_09:08-8048650)`, with numeric normal
history and recovery below it. Physical menu/default-boot and manual older-boot
acceptance remain pending. Existing entries have legacy-generation labels;
source metadata is available for new images and releases. Evidence:
`.airlock-logs/boot-menu-presentation/` and
[completed boot-menu plan](../completed/10-boot-menu-presentation.md).

Current checkpoint (2026-09-30): personal application VMs explicitly select
`sys-gui` as their GUI VM. Machine revision `caff35e` and parent pin `6368ef6`
passed **722 quick tests, 25 skipped**. Normal plan/apply through sys-dev and
sys-ctrl changed the two personal `guivm` preferences; after reboot, typed
verification marked transaction `1790728702090998936` **complete**. The running
host matches selected system `1r34d0azzpnc70kw5qmppwm3g30hlfcf`, and all seven
guest systems match their selections. The operator confirmed GNOME reached
login and subsequently confirmed the desktop looked good. After reboot, the
`usr-home` GUI agent and graphical session were active; GNOME's Xwayland client
list included Files from `usr-home` and Firefox from `usr-browser`, with their
Qubes VM identity properties. Evidence: `.airlock-logs/personal-guivm-fix/`,
including `verify-after-reboot.json`, `live-final-checks.json` and the plan/apply
records. See the
[GNOME notes](../sys-gui-gnome.md#personal-vm-gui-assignment-fix-2026-09-30).

Previous checkpoint (2026-09-29): **TODOs 05–07 complete**.
Final fixes `4845da0` and `b1cf5db` passed **720 quick tests, 25 skipped** and normal
apply/reboot. **Airlock (Generation 10)** booted the selected host; typed verify
marked transaction `1790721069384601443` complete. Both normal builders are
healthy, saved guest/builder/host markers match, temporary recovery setup is
absent, and dom0 has no failed units. Host pool generation 9 uses its own database.
Earlier root update/rollback, offline controller-disabled boot, isolation,
recovery, GC/compaction and measured planning results remain valid within their
recorded scope. No background test remains running; no USB rebuild was needed.
Evidence: `.airlock-logs/boot-verification-fix/final-apply/boot10-verify.json` and
r7 `boot10-final-checks.results.json`; completed plans are under `docs/completed/`.

## Connected personal desktop and management VM handoff

The default personal VM is connected `usr-home`, which owns Firefox, files and
terminal applications. Its uplink is `sys-firewall` through `sys-net`. Firefox
starts fresh; the retired browser's profile is not imported. The privileged lab
management VM is `sys-agent`; its tools, policies and builder identity use that
name. This does not change the Qubes trust or networking architecture.

The transition release retires `usr-browser` and `sys-dev` recoverably and maps
`airlock.machineDeployment.privateVolumeCopies.sys-agent = "sys-dev"`. Qubes
copies the independent private volume while both VMs are stopped, after setup
and before startup. Root-owned receipts preserve source/target UUIDs and volume
identities, prevent overwriting preexisting VMs, and prevent completed copies
from overwriting later writes during retries. The old VM's data remains in trash.
Clear the retirement and copy declarations in the following release.

Before applying to the existing lab, copy its root-owned `/var/lib/tailscale`
state to `/rw/config/tailscale` through its authorized root qrexec session.
This is enrollment-data preparation, not a live configuration overlay. Sys-agent
binds that private directory declaratively and sets its Tailscale hostname.
A cloned builder's former overlay is archived beside `/rw/airlock-builder`
before a new base is mounted; existing files and development caches are kept.
Host publication seeds the missing agent builder from the controller's frozen
view. The existing controller view receives an additive update.

For this first submission, the installed clients still have their previous names.
Use the ordinary plan/apply targets with these explicit transport overrides:

```bash
AIRLOCK_DEPLOY_SOURCE_SCHEMA=airlock.sys-dev-machine-source.v1 \
AIRLOCK_DEPLOY_SOURCE_COMMAND=airlock-sys-dev-machine-source \
AIRLOCK_DEPLOY_REMOTE_CLIENT=airlock-sys-dev-machine-deploy \
make machine-plan DEPLOY_TARGET=user@TAILSCALE_IP
```

Use the same overrides for apply. They select the installed transport only; the
built release uses the committed new names. No aliases are installed in new VMs.
Expect SSH to disconnect while sys-dev stops. The detached deployment continues.
The operator must reboot into the selected host before sys-agent's renamed
management policies can be used and whole-machine verification can complete.

## For the next agent

### Deployment diagnostics

New apply/resume/rollback transactions capture evidence under
`/var/lib/airlock/machine/transactions/<id>/diagnostics/` in dom0. `events.jsonl`
records phases, exact command arguments, elapsed time, failures/timeouts, and
observed Qubes/input service-state changes. Numbered `.stdout` and `.stderr`
files remain readable while commands run and are fsynced approximately every
half-second while waiting. PCI assignments are queried from Qubes before VM
stop/start, including assignments discovered automatically at setup time.

A background observer samples the dom0 journal and Xen/guest/device-model
console tails throughout deployment, independently of sys-agent or SSH. Journal
collection stops after reaching 32 MiB (the final batch may exceed this); console
snapshots keep the last 64 KiB per file. Observer commands have three-second
timeouts and failures are diagnostic only. Sampling runs about every two seconds
plus collection time. These are bounded best-effort observations, not a guarantee
against lost evidence on hardware failure. Existing console files are also
fsynced when sampled. The collector stops when the deployment command exits.

Storage-helper stderr additionally records the beginning/end/failure of each
`qubesd` stop/start and socket existence afterward. Socket presence and systemd
active states do **not** establish RPC or physical-input readiness. This logging
does not retry starts, reset hardware, change policy, or repair services.

Start with `git status`, this guide, and the last deployment journal. Keep four
claims separate: source committed, release built, guests verified, host verified.
Add pytest regression coverage before declaring an incident fixed. The primary
feedback loop is `make test-quick`; focused tests are
`uv run pytest airlock-machine/tests/test_machine_deploy.py tests/test_machine_deploy_client.py`.

Implementation entry points:

- `airlock-machine/lifecycle/misc/modules/machine-release.nix`: standalone release construction.
- `lifecycle/image/profiles/usb-prepared-store.nix`: prepared USB topology adapter.
- `lifecycle/misc/tools/scripts/machine-deploy.py`: build/provenance/SSH transport.
- `airlock-machine/lifecycle/runtime/tools/machine-deploy.py`: dom0 command-line entry point.
- `airlock-machine/lifecycle/runtime/tools/airlock_machine_deploy/`: planning, journalled apply/recovery, retirement, and release pruning; see its `README.md` for module and locking boundaries.
- `airlock-machine/lifecycle/runtime/tools/sys-agent-machine-deploy.py`: direct sys-agent build/transport client.
- `airlock-machine/lifecycle/misc/modules/machine-deployment.nix`: image/boot integration.
- `airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py`: staged generation use.

The physical acceptance run has demonstrated an ordinary VM config change,
unchanged providers staying up, a GUI/provider restart surviving SSH loss,
reboot preserving selection, and rollback restoring the previous guest system
while preserving home data. Treat this as a physically exercised development
updater, not a production-safe updater: failure injection, power-loss recovery,
release pruning, schema migrations, and restricted production authority remain
outside the demonstrated boundary.

### USB boot diagnosis, 2026-09-24

Offline inspection of boot `bc87160c-094d-4bd4-9f73-d482571da2a6` found:

- GNOME rejected the setup wizard's legacy `X-GNOME-Autostart-Phase` entry.
- sys-firewall uplink setup could not find `nft`. It also ran on downstream
  `vif*` interfaces, conflicting with Xen's hotplug address configuration.
  Tailscale received its enrollment request but DNS and control-server requests
  timed out through this broken network path.
- A desktop enrollment request started sys-dev during boot autostart; the
  coordinator then failed because the VM was already running.

The machine source fixes remove the wizard's legacy autostart phase, supply
nftables to uplink setup, exclude downstream Xen interfaces from both udev
startup and reconciliation, and tolerate concurrent VM starts while retaining
running-generation verification. Validation is limited to the quick test suite;
image rebuilding and physical boot verification are left to the operator.
The USB was inspected without configuration edits. The operator granted read
ACLs on its logs and Qubes/Airlock state directories for offline inspection.

### Desktop readiness and requested shutdown, 2026-09-24

A later USB boot marked sys-dev failed one second before its qrexec agent became
ready. GNOME's power-off request reached dom0, but the leftover startup marker
forced recovery on the next boot. The new source waits up to 180 seconds for each
started guest to reach Running and answer qrexec with the expected generation.
Start commands remain separately bounded; a wrong generation fails immediately.

After guarded hardware and GUI startup succeeds, background-service startup no
longer controls the forced-recovery marker. Failures remain recorded as degraded
and appear in GNOME as a notification with Retry and Dismiss. The fixed
`airlock.BootStatus` and `airlock.BootRetry` RPCs are limited to sys-gui; Retry can
only restart the declared boot-autostart unit after a degraded desktop startup.
It cannot run arbitrary commands or bypass the earlier hardware guards.

System shutdown records `shutdown_requested_at` separately from startup status.
A cancelled startup is not subsequently reported as an unexpected interruption;
known critical hardware/GUI failures still require recovery. Power loss before
the desktop boundary retains guarded recovery. Shutdown records express a
request, not proof that every VM and filesystem finished shutting down.

These changes require a newly built image and physical validation. No image was
rebuilt or live system modified during this change; validation uses quick tests.


### Prepared deployment reuse

A plan now records a root-owned prepared candidate token. Apply from the same
committed revision reuses that candidate and pinned build, with no package
inventory or payload transfer. Dom0 checks the retained views and current plan;
the worker also checks the recorded machine state under the deployment lock.
A stale candidate is rejected: run `make machine-plan DEPLOY_TARGET=...` again
and review its effects before applying. A direct apply without a saved plan still
prepares the release normally.

Unchanged guest generations keep their original admission provenance, bound to
the exact immutable identities in the new release. New package contents remain
validated at admission; existing objects use dom0's verified admission records.
The update that installs this implementation may still use the previous client
and receiver until its new host and guests are running together. Do not treat
that installation's timing as a measurement of the optimized workflow.
