# Repository-first whole-machine deployment

Failed boot investigated and corrected release prepared (2026-10-08): the
saved initrd evidence for boot `56146084-0929-4b18-b0f2-33ff303900e9`
confirms `host generation has no machine boot binding`. Menu generation 34
(`6c04c37`, host `c47s40197s7qpbap1qdfqm5fn11mqriy`) uses host-store generation
35, whose `machine.json` is absent. Both capture transactions timed out at
GUI reconnection before host binding/selection. The recovered menu generation
33 remains running and the persistent default; its selected deployment reports
complete. No live configuration mutation was used.

Normal `make machine-plan DEPLOY_TARGET=user@100.104.92.101` prepared corrected
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
A normal host reboot was requested for whole-machine verification. Keep generation
34 until the replacement has booted and verified. Guarded `make gc-auto`
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
`100.104.92.101`, its builder and Tailscale services are active, and no guest
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
confirmed the desktop and application. [Plan 12](completed/12-live-vm-additions.md)
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
rollback/reapply remain pending. See [TODO 12](completed/12-live-vm-additions.md#no-reboot-addition-verified-2026-10-01)
and `.airlock-logs/live-vm-trial/boot-hang-20261001/usr-notes-live-acceptance.json`.

Recovery baseline boot verified (2026-10-01): the operator restarted normally
and confirmed the desktop looks good. Dom0 now runs selected host
`4v44y6607wp1c087dgkl8id5pj8cxnrw`, boot ID
`b1866698-e8d6-42a9-b6a2-e27d064e387b`. Typed verification of transaction
`1790847116434715227` reports **complete**; all six running guests match their
selections and usr-browser is halted. The generic candidate-preparation and
admission RPCs are present. The separate usr-notes addition is now being
prepared; no further host reboot has occurred. See [TODO 12](completed/12-live-vm-additions.md#baseline-boot-verified-2026-10-01)
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
[TODO 12](completed/12-live-vm-additions.md#live-baseline-installation-attempt-2026-09-30)
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
[indicator plan](completed/gnome-focused-vm-indicator.md).

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
see the [indicator plan](completed/gnome-focused-vm-indicator.md).

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
[indicator plan](completed/gnome-focused-vm-indicator.md).

Boot-menu acceptance (2026-09-30): the operator restarted the laptop and confirmed
the menu looks fine with the final `consoleMode = "1"` setting. Post-reboot typed
verification reports transaction `1790768363786110491` **complete**: running and
selected host both resolve to `dyy6c32zx6fanzfrxyzgriw4rchihhwy`, all six running
guest systems match their selections, and usr-browser remains stopped. The
[boot-menu plan](completed/10-boot-menu-presentation.md) is complete, with the
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
and [completed boot-menu plan](completed/10-boot-menu-presentation.md). Next boot must confirm the
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
[performance notes](todo/08-deployment-metadata-validation-reuse.md).

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
`.airlock-logs/boot-menu-cleanup/` and [completed boot-menu plan](completed/10-boot-menu-presentation.md).

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
[completed boot-menu plan](completed/10-boot-menu-presentation.md).

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
[GNOME notes](sys-gui-gnome.md#personal-vm-gui-assignment-fix-2026-09-30).

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

## The development contract

The committed `airlock-machine` repository and the parent repository's pinned `airlock-machine/`
submodule pointer are the source of truth. A running machine is a
deployment target, not a second configuration repository. Implement fixes here,
test and commit them, then deploy those artifacts. Do not copy hand-edited GUI
scripts into `/run`, edit guest store paths, or leave systemd overrides as the
only implementation of a fix. Temporary diagnostic changes must be documented
and removed before claiming that a Nix generation reproduces the result.

The coordinator covers the whole declared machine, not one application/VM.
It uses the existing Xen/Qubes lifecycle, setup-config converger, policy, and
immutable closure-store engine. It does not replace those mechanisms.

For `usb-lab`, host-side plan/apply commands push committed source to sys-agent
and use the typed controller route. The explicitly selected development-host
transport (`DEPLOY_LEGACY_DOM0=1`) uses sys-agent's existing **development-only
full dom0 administrative access**, with a password-protected sudo session. It adds no
dom0 SSH listener or new production qrexec capability. This is not yet the
restricted config-authority/approval workflow described in `config-system.md`.

## Prerequisites and current validation boundary

- A bootstrapped machine started from the `Airlock` boot-menu entry, with
  sys-agent reachable over SSH/Tailscale and access to the dom0 development
  password. The internal deployment profile remains named `usb-lab`; it is not
  a boot-menu label. Discover the development VM's current SSH/Tailscale
  hostname or address with `tailscale status` and confirm it with `ssh`.
  Enrollment can change that hostname; do not infer it from the Qubes name,
  which remains `sys-agent`. Set `DEPLOY_TARGET` to the confirmed SSH target.
- Keep a physical dom0 console available for the first trials. Restarting
  sys-agent removes the remote connection; failures may require local recovery.
- Clean, committed `airlock-machine` and parent `airlock` repositories, an initialized
  `airlock-machine/` submodule at the committed pointer, and the guarded Airlock Nix store
  under `/run/media/user/data/airlock`.
- Free space in sys-ctrl's writable build store and dom0's shared pool and
  ordinary store. Normal updates transfer missing guest objects to the pool
  and missing host dependencies to the ordinary store. Space checks never
  resize partitions, run GC or delete retained generations to make an update fit.

At implementation time the live development VM's 31-GiB root had only
about 1.4 GiB free. SSH and the password-protected dom0 control transport were
tested, but a full live apply/reboot/rollback cannot be validated on that disk
without provisioning more space. `make usb-dev-provision USB_SERIAL=...` now
expands a freshly flashed root filesystem to the USB capacity before
personalization; see `usb-dev.md`. This erases the old installation as before,
so back up wanted runtime data first. No larger seed image is necessary.
The release build and pytest fault tests are
separate evidence, not a claim of a completed physical deployment. Do not erase
this distinction when handing off to another agent.

Physical follow-up, 2026-09-16: the expanded USB successfully applied release
revision `f8e15558ccc625c84035851727420e7d9b00f98c` and rebooted into its selected
host generation. `airlock-machine-autostart` started `usr-home` automatically;
`machine-verify` returned `complete`, with matching host and running guest
generations and no failed dom0 units. The browser stayed stopped as configured.
The earlier image-to-release apply also survived restarting sys-agent/sys-gui.

Rollback follow-up, 2026-09-16: revision `814b3a4de5d744a5f6d8f83c317166728a097bf3`
removed the temporary `usr-home` generation marker and switched `usr-home` from
`hbnf7k25...` to `n911xmhv...`. Rolling back restored both the prior release and
`hbnf7k25...`, including its marker, while exact sentinels in the persistent
homes of `usr-home` and `sys-agent` survived. Reapplying `814b3a4...` removed the
marker again; after reboot, `machine-verify` returned `complete`, both sentinels
still matched, and dom0 had no failed units. The temporary sentinels were then
removed. This validates one-generation whole-machine rollback and persistent
home preservation; it does not claim application-level data migration rollback.

GNOME's shutdown action was observed stopping only sys-gui, and input forwarding
did not recover after that VM stopped; these are separate unresolved desktop
issues.

## Normal loop

When committing inside `airlock-machine/`, make the commit available in the
submodule source before running parent flake checks or building. Nix fetches
the pinned revision from the URL in `.gitmodules`; a commit that exists only
in the initialized submodule is insufficient. For this checkout's local sibling
source, run from the parent repository:

```bash
git -C ../airlock-machine fetch --no-tags "$PWD/airlock-machine" HEAD:refs/airlock/parent-build-source
git add airlock-machine
git commit -m 'Update machine component'
```

This fetch preserves the sibling repository's working tree and current branch.

```bash
# Make the change in this repository, then:
make test-quick
git add <the relevant files>
git commit -m 'Describe the change'

make machine-build
# Set this once to the current SSH target discovered above:
export DEPLOY_TARGET='<current-development-VM-SSH-target>'
ssh "$DEPLOY_TARGET" hostname
make machine-plan
make machine-apply
make machine-status
make machine-verify
```

The same whole-machine release can be built and submitted directly from the
clean committed checkout included in sys-agent, without a parent `airlock`
checkout or development-host build artifacts:

```bash
airlock-sys-agent-machine-deploy build
airlock-sys-agent-machine-deploy plan
airlock-sys-agent-machine-deploy apply
airlock-sys-agent-machine-deploy status
airlock-sys-agent-machine-deploy verify
```

The sys-agent client sends a versioned source bundle to `sys-ctrl`. The
controller verifies the Git revision and lock hash and builds from the locked
source, fetching missing inputs or substitutes through sys-firewall. The live-VM
addition branch first imports missing host/transport dependencies through the
installed `airlock.Ctrl.MachinePrepare` interface into dom0's private host-store
upper. Dom0 validates the candidate and issues an inactive admission token.
The controller sends guest manifests through candidate-bound pool admission,
which requests only missing package objects and immutable root artifacts.
The TODO 07 implementation then publishes
verified host objects into the same pool and prepares an approved sys-ctrl base
for its next stopped setup. Dom0 binds independently checked admission receipts
to the release and invokes its journalled coordinator. `plan` never activates;
`apply` starts a detached worker so restarting sys-ctrl or sys-agent cannot cancel
activation. Normal status/verify/resume/rollback and retention commands use a
typed dom0 interface, including when sys-ctrl is unavailable. The explicitly
selected `--legacy-dom0` development route remains separate.

The controller records source archives and requests in
`~/.local/share/airlock/controller/candidates/<revision>/`; its observation timer
records actual dom0 outcomes. Submission is not success. The final report
includes package and sparse-root payload counts, root logical/allocated bytes,
host NAR/compressed bytes, build time and transfer/admission/validation timings.
The unique root inventory avoids counting a shared immutable base once per VM.
Unchanged roots seeded by image creation are verified and reused without another
Nix-store import. Root artifacts live in dom0’s pool and are pinned by guest
generations; Qubes imports them into separate per-VM writable root disks.
Private disks remain per VM. Host kernels stay available through the host closure.
Stopped sys-ctrl does not own the serving guest pool, and dom0 retains the host
and controller boot artifacts independently.

For a committed single-guest update, the controller route is:

```bash
# Run inside sys-agent from its clean airlock-machine checkout.
airlock-sys-agent-machine-deploy guest-plan --target usr-home
airlock-sys-agent-machine-deploy guest-apply --target usr-home
```

The client sends a versioned source archive over qrexec. `sys-ctrl` verifies
the revision and lock hash, imports the checkout, builds from locked inputs
in its pool-backed private store, and sends the resulting guest closure
to dom0's shared pool. Dom0 accepts admission only from `sys-ctrl`, validates
the inactive generation receipt, and runs its existing journalled guest
plan/apply coordinator. The controller cannot submit an arbitrary release path
or shell command to this interface. Guest activation also uses a detached dom0 worker, so updating sys-agent or
sys-ctrl can disconnect the submitting client without cancelling activation.
Inspect dom0's machine deployment status to establish the outcome. The newest whole-machine integration has not been tested or built in this
pass; the sys-ctrl TODO remains open for automated and physical validation.

For a guest closure already built and admitted by sys-agent, the development
coordinator can derive a one-guest candidate without building or transferring a
second whole-machine release:

```bash
make machine-guest-plan ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
make machine-guest-apply ADMISSION_RECEIPT=/home/user/.local/share/airlock/machine-build/admissions/<receipt>.json
```

The receipt path is inside sys-agent. Dom0 validates it against the retained pool
generation and keeps the resulting candidate and exact generation binding in
the machine journal. Apply, verify, reboot handling, resume and rollback then
use the same coordinator flow as an ordinary whole-machine release.

For a coordinator fix on an already flashed image, add `UPDATE_COORDINATOR=1`
to `machine-guest-plan` and `machine-guest-apply`. This builds the standalone
coordinator from the committed local submodule and transfers only missing Nix
paths through the existing development transport. The candidate records that
coordinator, and apply roots it against GC. This avoids rebuilding/flashing the
USB for a coordinator-only fix; it does not update other host components.

`machine-build` needs no remote machine. All commands default to the internal
`usb-lab` deployment profile; this is not a boot-menu entry.
`DEPLOY_PROFILE=usb-gui-lab` builds that complete profile instead, but its usual
topology has no sys-agent bridge; use the dom0 coordinator directly if artifacts
are provisioned by another authorized transport. A deployment cannot silently
change the active boot profile.

`plan` builds/imports missing immutable artifacts and displays the complete
change/restart plan; it does **not** activate a generation or restart VMs.
`apply` submits the requested release and launches a detached systemd worker in dom0.
Closing the terminal or losing SSH does not terminate that worker. Reconnect
and inspect status; do not interpret a disconnected SSH session as failure.

Guest changes are applied live by restarting affected VMs. Dom0 uses
`switch-to-configuration boot`, not `switch`: physical GPU, Xen, initrd, host
services and host policy changes take effect on reboot. `reboot-required` means
the guests were verified but the selected host generation is not running yet.
After reboot, run `machine-verify` again. Only `complete` means both host and
guest generation checks passed. Window behavior still needs its own functional
test; generation equality alone is not a GUI acceptance test.

The boot menu lists normal Airlock generations from newest installed to oldest,
then recovery entries. New images and releases use titles such as
`Airlock (2026-09-30_14:30-a1b2c3d)` and
`Airlock (recovery, 2026-09-30_14:30-a1b2c3d)`, using the source commit's UTC
timestamp and short SHA. Entries created before source metadata was embedded
are explicitly labelled `legacy generation N` during verified migration.

The newest normal generation is always the persistent default. Manually choosing
an older generation boots it once; the following boot defaults to the newest
again. Whole-machine rollback and recovery selection do not establish an older
persistent default: use the physical menu to boot that older host. Boot artifacts
and writable host state remain specific to the manually chosen generation.

Recovery does not start VMs, hand off the GPU or bring up networking automatically.
The repository-owned post-install normalizer applies presentation after image
builds and deployments. Recovery is pinned as a separate Nix system profile, so
it remains available when the original image generation falls outside history.
Retained entry migrations verify existing checksums before changing presentation,
with a journalled checksum transition; Xen, kernel and initrd checksums stay fixed.

Normal USB boots are guarded by a persistent boot-attempt supervisor. A VM is
not ready merely because Xen created it: network providers, sys-gui, and every
declared autostart guest must answer qrexec and report the exact
`/run/current-system` selected by the topology. Startup stages have hard
timeouts and failures capture the journal, Xen state, Qubes state, memory, and
guest console logs under `/var/log/airlock/boot-attempts/<boot-id>/`. The
machine-readable attempt state is under
`/var/lib/airlock/boot-attempts/<boot-id>/`. A detected failure replaces the
bootstrap display with an actionable tty1 diagnostic login instead of leaving
a stale completion message onscreen.

If a hard reset interrupts a supervised boot, the next boot preserves the old
logs before Xen or Qubes can reopen them. It then deliberately enters a safe
local recovery boot: PCI-backed VMs, GPU handoff, networking, and guest
autostart remain disabled, and tty1 explains where the evidence lives. Inspect
that evidence and reboot once more for a normal retry. This recovery boot is
distinct from the boot-menu `Airlock (recovery)` generation.

The 2026-09-22 physical trial exposed three integration defects before any guest
started: the readiness package omitted its supervisor dependency, concurrent
failure handlers collided on the state file, and logind's `autovt@tty1` login
competed with the diagnostic console. The local fixes declare the dependency,
serialize state/event/marker updates with unique temporary files, and mask both
standard tty1 login names during supervised boots. Diagnostic login is also
requested if capture fails. Regression checks run the built readiness package
with an empty search path, launch concurrent failure handlers, and evaluate both
tty1 masks. A fresh physical boot and failure-console trial remain required.

The `airlock-machine` revision and lock-file hash are recorded in the release. The
development-side receipt also records the parent revision. Local build receipts
and GC roots are under `.airlock-logs/machine-deploy/<airlock-dev-commit>/`.
Runtime journals are in `/var/lib/airlock/machine/transactions/<id>/`, including
`status.json`, `staged.json`, and setup's JSON/command logs. Use
`journalctl -u 'airlock-machine-deploy-*'` in dom0 for worker output.

## What is built and applied

The flake output is:

```text
./airlock-machine#airlock-machine-release-usb-lab
```

It contains `release.json` (host system, exact setup/coordinator binaries,
profile, commit and lock hash) and `topology.json` (all declared VMs, policies,
preferences, devices and exact VM systems/root/store sources).

The release is built directly from the pinned `airlock-machine` checkout. The USB
image calls the same OS construction function, then substitutes prepared-store
descriptors only for the topology embedded in the image. Release construction
canonicalizes those entries to ordinary closure manifests for transport and
admission. Both paths name the same VM system paths and cannot drift into
independently maintained host or guest configurations. A standalone release
does not evaluate or build the prepared store or disk image.

The coordinator:

1. Captures the booted image/current topology as the first rollback baseline.
2. Validates the whole topology and calculates dependency-ordered restarts.
3. Pins host artifacts and checks the controller-admitted guest generations without selecting them.
4. Records the desired release before stopping affected VMs, dependants first.
5. Runs the release's exact setup-config binary over the complete topology,
   reusing its staged admissions; private/home volumes are not replaced.
6. Selects the host boot generation, starts VMs in dependency order, and checks
   selected systems and running guests' `/run/current-system` over qrexec.

Unchanged VMs remain running. A first deployment can affect every VM because
the image's prepared descriptor sources are replaced by closure-manifest sources.
Already running VMs and declared autostart VMs are brought back, with their
dependencies. Shutdown timeouts fail the transaction; the tool does not kill VMs.

### Live VM addition implementation

[TODO 12](completed/12-live-vm-additions.md) implements a released baseline that can
prepare and activate a new ordinary VM without a preparatory host deployment
or reboot. Its host generation is selected for the next boot while Qubes creates
the VM and its persistent private disk on the running host. Physical acceptance
is pending; the existing live machine has not received this branch.

Candidate admission is restricted to sys-ctrl and binds exact guest identities
to the imported candidate. It does not activate anything. The normal whole-machine
apply still binds independently checked pool receipts, journals activation and
uses the existing selected-release boot and recoverable rollback paths. An
admission token becomes stale when boot, selection or deployment state changes;
prepare the release again rather than bypassing the check.

Graphical readiness uses the Qubes GUI-VM tag. The new sys-gui interface
`airlock-sync-qube-appmenus VM` explicitly starts a guest, waits for its session
and uses upstream Qubes tools to install its declared menus live. `menu-items`
contains the selected desktop filenames; generated launchers use fixed StartApp
RPCs and the readiness barrier. Routine menu additions need no sys-gui restart.
User review/approval and the smaller declaration interface remain separate work.

Deployment admission uses `--verify-mode incremental`: complete canonical object
records in dom0's managed append-only pool are reused without rehashing their
source trees or stored payloads. Both metadata records must agree on identity,
tree hash and extent; backing extents must exist and fit their segment. Missing
records fall back to admission. New paths undergo nonrecursive Nix content
verification, tree hashing and archive admission before generation publication.
The generation manifest records new/reused object counts. Prepared image objects
are trusted under the same build-validated image assumption used at bootstrap.

This assumes the pool and its admission metadata remain dom0-owned and payloads
are not modified after admission. Incremental deployment does not detect silent
bit rot in previously admitted bytes. Explicit `admit --verify-mode nix-store`
retains the full source/closure verification path and payload checks; it can be
used for an integrity investigation rather than on every deployment. Transport
checks, generation verification and fail-closed publication remain in place.

Sys-dev's `airlock-pool-send` now uses `airlock.Pool.AdmitIncremental+TARGET`.
It sends a closure inventory with source tree hashes first. Dom0 checks its
existing pool records (including the prepared USB seed), requests missing
objects, then verifies only the transferred payloads before publishing a retained,
inactive generation. Reused objects are neither packaged nor uploaded; sys-agent
still reads their local contents to compute the inventory hashes. The response
records `objects_transferred`, `payload_bytes_transferred`, and reuse counts.
Admission, GC, compaction, and recovery share a lock to protect negotiated reuse.
Both sys-agent and dom0 need the updated image; the original full-artifact service
remains available, but the new sender does not silently fall back to a full upload.
Fresh-image physical validation passed on 2026-09-18: a committed usr-home
change built in sys-agent admitted 4 new objects (434,176 bytes) and reused 978.
Generation 2 was retained but inactive; generation 1 and the running guest
stayed unchanged. See the [completed build/admission trial](completed/01-sys-dev-machine-build.md).

Measured on the physical USB laptop on 2026-09-16: incremental re-admission of
the already deployed sys-gui closure took 2.13 seconds, reusing all 1,578 objects
(9,488,805,888 logical store bytes) with zero admissions. This was an inactive,
unretained benchmark generation (sys-gui generation 8), not a VM switch. It tests
the fully reused case; a changed closure may still need a new EROFS index and VM
restart, so this is not a two-second end-to-end deployment claim.

The selection survives a normal reboot. Boot setup consumes the selected
release, not the original image topology. Old images receive a repository-owned
compatibility drop-in for setup-config; new images include the coordinator and
profile marker. `/var/lib/airlock/machine/controller` points to a pinned Nix
binary, not a manually maintained live script.

`airlock-machine-autostart.service` starts declared autostart VMs and their
dependencies after topology setup, network-provider startup, and the guarded
physical-GPU startup. This replaces the boot responsibility of the deliberately
masked USB `qubes-vm@` service, without bypassing GPU checks. It consumes the
selected release (or image topology before the first deployment), skips running
VMs, and fails closed on interrupted deployments or a profile mismatch. It does
not resurrect every VM that happened to be open before shutdown.

Reflashing can erase deployment journals and machine data. It does **not** erase
the committed implementation or configuration in this local repo. Building the
next image from that commit reproduces its declared configuration. Runtime user
data, Tailscale enrollment and credentials need separate backups.

## Independent recovery acceptance

Select the labelled Airlock recovery entry manually from the boot menu and log in as `user`
with the provisioned local password. This entry deliberately has no networking,
Qubes guests or main pool-store assembly, so remote Tailscale access is unavailable.
For the initial healthy-pool recovery smoke test, inspect:

```bash
cat /etc/airlock/pool-recovery
findmnt /nix/store
```

The store should be a read-only bind of `/var/lib/airlock/recovery-store`, not
the normal host overlay. Then reselect the currently retained normal host:

```bash
sudo airlock-host-store recover "$(readlink -f /nix/var/nix/profiles/system)"
```

This validates the retained pool binding and installed EFI artifacts before
selecting the entry; it does not reboot. Capture the command result at the
console, then manually boot the newest normal Airlock entry and verify the machine again.
This smoke test alone does not establish recovery from a missing/corrupt view;
that fault-injection trial remains separate. Do not use the recovery environment
to run normal whole-machine apply or garbage-collect its shared Nix database.

## Failures, recovery, and limits

This is a journalled convergent deployment, **not an atomic multi-VM database
transaction**. A failure can leave a mixed running state. The journal records
the failed phase and selected/previous releases; it never reports that as success.
Boot reconciliation refuses an interrupted transaction instead of silently
adopting the image's original topology. Don't run legacy apply tools concurrently.

```bash
make machine-status
make machine-resume
# Or explicitly select the previous whole-machine release:
make machine-rollback
```

If sys-agent is down, use the physical dom0 console:

```bash
sudo /var/lib/airlock/machine/controller status
sudo /var/lib/airlock/machine/controller resume
# Or:
sudo /var/lib/airlock/machine/controller rollback
```

Do this from the matching lab profile. Text recovery can inspect files and boot
entries, but is deliberately not an alternative configuration apply target.
Rollback reselects systems, not user-data snapshots; application data migrations
are not undone. A VM introduced by the rolled-back release is stopped,
quarantined in recoverable trash with its private data, and reported as
`retained_undeclared`.

Automatic deployment currently supports Nix-closure/shared-closure VMs and
recoverable domain retirement. It still rejects class/template/storage-layout
changes, arbitrary physical device reassignment, and deletion of
preference/feature/service keys without explicit reset values. A rollback that
removes newly introduced settings can likewise require explicit reset values.
Unsupported changes fail preflight rather than being silently ignored.

The USB lab `sys-agent`/`sys-net` automatic network PCI handoff is a scoped
exception: the release coordinator reports `pci_handoff`, checks current
network-controller assignments before stopping VMs, and releases the former
owner after the dependent VMs stop. Setup assigns the new owner. The same
sequence supports rollback; confirm it on hardware before relying on remote
access through sys-agent.

To retire a domain, remove it from the topology and list its name in
`airlock.machineDeployment.retireDomains` in the same committed release. The
list must exactly match domains removed relative to the selected release; clear
it again in later releases. Preflight rejects removal while surviving domains,
machine defaults, or topology-managed qrexec rules still refer to the domain.
Later releases are held to the same rule while the name remains in trash;
reintroducing the VM through a validated restore is the exception.
Apply stops the domain, disables
autostart, applies the `airlock-retired` tag, pins its selected pool generation,
and records its UUID, private storage identity, source release, and expiry under
`/var/lib/airlock/machine/trash/`. Early qrexec policy denies ordinary calls
from or to tagged domains. The domain remains registered under its original name so Qubes
and the shared pool retain their existing identity bindings.

Whole-machine rollback, or a later committed release that reintroduces the same
domain and private storage identity, restores it and removes the retirement tag.
Boot enforces the stopped, tagged state and refuses to continue if a trashed VM
was found running. Inspect trash from a dom0 recovery shell with:

```bash
sudo /var/lib/airlock/machine/controller trash-list
```

Trash is retained for at least
`airlock.machineDeployment.trashRetentionDays` (30 days by default). Expiry only
makes it eligible for permanent deletion. Purge also requires every retained
release and bootable host profile referencing that VM to have been safely
pruned. Use the deployed coordinator to inspect all recorded releases, their
roots, removable Nix boot generations, and protection reasons:

```bash
airlock-sys-agent-machine-deploy release-list
airlock-sys-agent-machine-deploy release-prune-plan --release /nix/store/RELEASE
airlock-sys-agent-machine-deploy release-retain --release /nix/store/RELEASE --retain true
airlock-sys-agent-machine-deploy release-prune --release /nix/store/RELEASE \
  --confirm /nix/store/RELEASE
```

Replace `RELEASE` with the exact release path shown by `release-list`.
The selected release, its immediate rollback choice, the running/booted host,
recovery profiles, unfinished transactions, and unexpired trash requirements
are protected. Additional rollback choices can be protected with
`release-retain`; `--retain false` removes only that explicit protection.
To move the immediate rollback choice beyond a retirement, deploy another
committed release that still omits the VM and clears `retireDomains`. A
confirmation cannot override any protection. Legacy failed transaction records
without a recorded recovery remain protected and require investigation.
Boot into the selected retirement generation before testing purge: a running
host whose embedded topology still references the VM also blocks deletion.
Recovery images remain protected; use a disposable VM added after image boot
for the first purge trial so the image's recovery topology does not declare it.

To empty a specific trash record before its deadline, inspect the effective
prune plan and prune each release that references the VM:

```bash
airlock-sys-agent-machine-deploy release-prune-plan --release /nix/store/RELEASE \
  --allow-unexpired-trash TRANSACTION-VM
airlock-sys-agent-machine-deploy release-prune --release /nix/store/RELEASE \
  --confirm /nix/store/RELEASE --allow-unexpired-trash TRANSACTION-VM
```

The override waives only `unexpired trash TRANSACTION-VM` for that release.
It cannot bypass a selected or running host, rollback/recovery reference,
unfinished transaction or purge, or explicit retain. Each additional unexpired
record needs its own `--allow-unexpired-trash` argument. Pruning the source
release gives up restoration from that release; its exact roots and boot
generations are still journalled before removal. Repeat an interrupted prune
with the same override.

Pruning journals the exact roots and profile generation numbers before deleting
them, refreshes the boot menu, and preserves deployment audit files. It does
not run Nix or pool GC. Repeat the same command after an interruption; deployment
and purge refuse to proceed while pruning is unfinished. Inspect
`release-prune-plan` for the durable status and the reported systemd unit journal
for errors. Pruned releases cannot be applied again through the coordinator;
reintroducing a VM requires a new committed release. Once references have been
pruned, old software may be collected and that rollback choice is lost.

After expiry and pruning, purge one exact trash record:

```bash
sudo /var/lib/airlock/machine/controller trash-purge TRANSACTION-VM \
  --confirm TRANSACTION-VM
```

The sys-agent equivalent is `airlock-sys-agent-machine-deploy trash-purge
--identity TRANSACTION-VM --confirm TRANSACTION-VM`. Prune and purge use detached
dom0 workers and survive SSH disconnection; `trash-list` reports purge status.
Before expiry, add `--before-expiry` to that exact purge command. This waives
only the deadline; the retained-release, profile, VM identity, ownership, and
stopped-state checks still apply. The trash audit record notes early purging.

Purge checks the UUID, actual volume identities, sharing/source relationships,
retirement tag, stopped state, and recorded pool generation before removing
the Qubes domain. Trash created before volume snapshots were introduced must
be restored and retired again before purge. Do not edit its metadata to bypass
the check. The private volumes are permanently removed through Qubes, while the
pool generation becomes eligible for ordinary GC. Shared objects remain subject
to other generations' and builder views' retention. Durable pool retirement
receipts allow retries even if GC runs before the coordinator records completion.
The audit record remains with status `purged`; repeating a finished purge is a
no-op. No expiration or purge automatically deletes unrelated pool objects.

There is no automatic old-release pruning. Retained releases cost disk space.
Only use the guarded Airlock GC targets on the development host; never run GC
on its host `/nix/store`. Do not manually delete runtime closure-store segments.

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
