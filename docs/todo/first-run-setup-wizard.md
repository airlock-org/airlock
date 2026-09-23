# First-run setup wizard

Status: planned; no implementation. Created 2026-09-23.

## Purpose

Introduce a small AirlockOS setup GUI after the first usable sys-gui desktop
starts. This establishes an installation stage for decisions that need user
interaction, with a persistent completion state and a way to reopen setup.

Bootstrap must provide working display and basic input before the wizard runs.
Keep **desktop ready for setup** distinct from **setup complete**: starting
sys-gui does not complete setup, and the wizard cannot be responsible for the
input needed to operate its own first screen.

Follow the [hardware usability policy](../hardware-usability-policy.md): users
must be able to test proposed hardware changes live, and failed or unconfirmed
trials must return automatically to a working configuration. This is a core
wizard requirement, implemented after the initial welcome/completion shell and
before enabling any disruptive hardware page.

## First implementation

Provide a welcome screen explaining first-run setup and an explicit **Finish
setup** action. This first milestone establishes the lifecycle without adding
hardware configuration or unrelated settings.

- Launch automatically in the first usable interactive sys-gui session while
  setup is incomplete. A running VM alone is not sufficient readiness.
- Show one wizard instance per session; repeated startup triggers must not
  create duplicate windows.
- Persist completion only after the user selects Finish setup and the state
  write succeeds. Report a write failure and keep setup incomplete.
- Closing the window leaves setup incomplete. Allow reopening from a desktop
  launcher, and offer setup again on the next session or boot. Do not repeatedly
  reopen a dismissed window within the same session.
- A crash, logout, or reboot before completion leaves setup available to resume.
- Once complete, normal login and reboot do not launch it automatically.
- Allow explicit reopening after completion without clearing completion merely
  because the user opened or closed the wizard.

The wizard should not block access to the usable desktop or recovery tools.
No installation partitioning, account setup, or full settings application is
required for this milestone.

## State and ownership

Treat completion as persistent machine setup state, not a temporary file in
the guest root filesystem or an incidental desktop preference. Specify its
authoritative owner and storage location during implementation, with one
source of truth. It must survive sys-gui replacement and normal OS updates.
Use an atomic state update and distinguish unreadable state from a fresh
installation; a read error must not silently become successful completion.

Fresh images start incomplete. Define how an existing installation with no
record enters setup, without modifying its configuration merely by showing the
wizard. Ordinary updates must not reset completion. Future required steps need
an explicit state migration policy; changing an application version alone must
not rerun setup.

Package and enable the wizard declaratively in Nix. Keep its presentation in
sys-gui and preserve existing Qubes authority boundaries. If completion storage
requires an inter-domain operation, use a narrow qrexec service with explicit
caller policy, not arbitrary commands or filesystem writes.

## Configuration authority

The first milestone records setup completion only; it does not require a
configuration deployment. Future pages that change machine configuration must
submit declarative candidates through Airlock's validation and apply machinery.
They must distinguish a saved choice from an applied and verified result, and
must not maintain a separate live configuration or bypass dom0 enforcement.

Follow the deployment architecture in place when those pages are implemented.
The [configuration system](../config-system.md),
[machine deployment workflow](../machine-deployment.md), and planned
[sys-ctrl migration](05-sys-ctrl.md) describe distinct paths and stages; do not
assume sys-ctrl already exists or make this minimal wizard depend on that
migration. Do not grant sys-gui general dom0 administrative access.

## Live trials and automatic rollback

Keep wizard completion separate from configuration trial state. Finish setup
must not confirm a pending hardware trial. Reopening an already completed
wizard must offer the same recovery protections for subsequent changes.

Before a disruptive change, show its purpose, the devices affected, the test
the user must perform, and the countdown/automatic recovery behavior. Save a
complete known-good baseline and durably record the candidate and trial identity.
Arm a bounded rollback supervisor before applying the change; if that cannot
be done, do not start the trial.

The intended sequence is:

```text
working baseline -> armed trial -> candidate applied -> live user tests
  -> explicit confirmation: retain verified candidate
  -> timeout, cancellation, or failure: restore baseline and verify recovery
```

The GUI presents the countdown, but the deadline and rollback belong to a
privileged supervisor outside sys-gui and the affected service VM. Use existing
deployment machinery with narrow, policy-controlled trial operations. The
supervisor must not depend on keyboard input, pointer input, networking, or a
responsive wizard to restore the baseline.

Require fresh exercises for every affected essential capability. For input,
require both keyboard and pointing-device interaction followed by an explicit
keep/confirm action. Raw activity, a detected device, or a running VM is not
confirmation. No input, only one working input type, or no final confirmation
by the fixed deadline triggers rollback. Background events must not extend the
deadline. Specify accessible instructions and a reasonable bounded test period
during implementation.

Only confirmation for the current trial, received before its deadline and
after required checks, may retain the candidate. Handle timeout/confirmation
races atomically; stale or duplicate messages cannot confirm a later trial.
Allow explicit cancellation to request immediate rollback.

For changes requiring reboot, persist the baseline, candidate, and unconfirmed
state before reboot. Define bounded boot/startup and interactive confirmation
windows so failure to reach the GUI also recovers. A reboot must not clear the
trial or reset its budget indefinitely. Provide boot fallback for a candidate
that never reaches the supervisor; do not rely solely on a GUI-session timer.

Restore the whole affected configuration, including device ownership and
forwarding where relevant, not just its source files. Verify restoration before
reporting rollback success. Prevent repeated failed trials or reboot loops,
retain failure details, and present the outcome when the desktop returns.
Document recovery for host hangs, power loss, or restoration failures; automatic
rollback cannot guarantee recovery from a wedged host without a restart.

Do not enable a disruptive page until its apply, confirmation, timeout, and
recovery paths are implemented and tested. The initial welcome shell can ship
without making hardware changes.

## Future pages and separate work

Trusted-input identification and USB controller planning motivate this setup
stage: a future page could ask the user to use their keyboard and pointing
device, then present an ownership proposal. The separate
[USB input TODO](usb-input-setup.md) covers discovery, controller suitability,
input preservation, declarative assignments, verification, and rollback. It is
the first intended consumer of live trials; it is not required for the initial
welcome/completion shell.

USB Wi-Fi, networking, advanced device routing, stronger input isolation, and
general hardware configuration are outside this document's implementation
scope. Leave room to add ordinary pages without building a plugin framework or
a general workflow engine now.

## Acceptance criteria

- [ ] A fresh boot reaches a usable sys-gui desktop and shows one welcome wizard.
- [ ] Keyboard and pointing-device interaction work before setup starts.
- [ ] Finish setup durably records completion; the next login and reboot do not
  automatically show the wizard.
- [ ] Closing, crashing, or rebooting before completion allows setup to resume
  without falsely recording success or repeatedly opening windows.
- [ ] The desktop launcher reopens setup before and after completion.
- [ ] State read/write failures are visible and do not report completion.
- [ ] Completion survives a sys-gui system replacement and an ordinary update;
  missing-state behavior on existing installations is documented and checked.
- [ ] No USB ownership, networking, or other machine configuration changes occur
  as a side effect of opening or finishing the initial wizard.
- [ ] Any cross-domain interface has a narrow operation and explicit caller
  policy; the GUI receives no general dom0 command authority.

Before shipping disruptive setup pages:

- [ ] A live trial exposes a countdown and requires capability tests and an
  explicit timely confirmation to retain changes.
- [ ] No input, partial input, no confirmation, GUI crash, and affected service
  failure trigger automatic rollback independently of the wizard.
- [ ] Failure to arm recovery prevents applying a candidate; stale confirmation
  and timeout races cannot retain an unconfirmed configuration.
- [ ] Reboot, power interruption, and failure to reach the GUI preserve recovery
  state and return to the baseline without an endless retry/reboot loop.
- [ ] Rollback restores and verifies the full affected configuration; failed
  restoration is reported accurately and has a documented recovery procedure.

Use focused pytest coverage for state transitions and startup behavior, plus a
GUI smoke test for the actual launch, finish, reopen, and reboot flow. Add
failure-injection checks for the trial protocol and physical verification of
hardware rollback before enabling hardware changes. Report automated checks
and observed GUI behavior separately.

## Implementation location

Deployable wizard packaging, desktop integration, and persistent state support
belong in `airlock-machine`. The parent `airlock` repository owns this plan and
image/lab integration. Follow the
[repository boundaries](../repositories.md) when implementing and validating.
