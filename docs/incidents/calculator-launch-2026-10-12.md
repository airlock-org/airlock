# Calculator launch failures, 2026-10-12

The operator reproduced a refused launch during boot and a separate delayed
window after closing and reopening Calculator with the desktop unlocked.
No tests were run. No runtime configuration overlays or process-memory writes
were used. The diagnostic launch used a collected transient user unit.

## Boot-time refusal

At 08:22:12 UTC, the existing launcher recorded Request refused at exactly the
same time dom0 stopped qubesd. The policy daemon could not obtain system info
while qubesd was unavailable. Qubesd returned at 08:22:15. The GUI watcher
reconnected and attached usr-home at 08:23:35. Usr-home's agent and Xorg were
running by inspection; the selected khaki host had booted successfully.
Whole-machine verification reports complete. No GPU fence/flip timeout matches
were observed in this boot.

The launcher previously exited on its first failed feature lookup/readiness
request, converting a transient control-service outage into a permanent user
error. Component cda910c retries only feature lookup and session readiness
within a shared 90-second budget. StartApp remains a single submission, avoiding
duplicate application launches. Shell syntax parsing succeeded; no automated
checks were executed. The diagnostic launch attempted while GNOME was locked
was correctly refused by workspace preparation and is a separate condition.

## Custom-label window identity

Khaki is Qubes label index 9. The focused-window identity reader accepted only
indices 1 through 8 despite first verifying the supplying client's executable
against the installed GUI daemon. A live read of Calculator XID 0x1600089
returned unknown. Its X11 properties identify Calculator in usr-home, with a
viewable window on workspace 0. The workspace extension hides windows pending
identity resolution and retries unknown identities, then falls back to an
isolated neutral workspace. Thus custom-label windows enter the wrong handling
path; process existence and even X11 viewability do not establish physical
visibility. Both manual launches eventually appeared and the operator confirmed
Calculator input worked. Exact compositor timing was not traced; GPU-renderer
probe warnings alone do not establish the cause of the delay.

Component 293bea5 accepts positive custom label indices within the one-byte
property range (1 through 255), retaining executable provenance and all property
validation. Component 8619ca9 updates invalid-label coverage and adds custom-label
cases proving untrusted executable provenance remains rejected. Coverage was
written but not run, respecting the operator instruction.

Both fixes are committed and pinned. Live deployment and physical acceptance
of the new generation are pending. An initial readiness-only plan is being
prepared and must be superseded with a plan for the latest combined revision
before activation. Evidence and controller logs are under
.airlock-logs/calculator-launch-2026-10-12/.
