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

## Future pages and separate work

Trusted-input identification and USB controller planning motivate this setup
stage: a future page could ask the user to use their keyboard and pointing
device, then present an ownership proposal. Track that work in a separate TODO
covering discovery, controller suitability, input preservation, declarative
assignments, verification, and rollback. None of it is required to complete
this wizard milestone.

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

Use focused pytest coverage for state transitions and startup behavior, plus a
GUI smoke test for the actual launch, finish, reopen, and reboot flow. Report
automated checks and observed GUI behavior separately.

## Implementation location

Deployable wizard packaging, desktop integration, and persistent state support
belong in `airlock-machine`. The parent `airlock` repository owns this plan and
image/lab integration. Follow the
[repository boundaries](../repositories.md) when implementing and validating.
