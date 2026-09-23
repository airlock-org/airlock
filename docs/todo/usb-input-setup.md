# Preserve and verify input during USB setup

Status: planned; no implementation. Created 2026-09-23.

Follow the [hardware usability policy](../hardware-usability-policy.md).
Use the [first-run wizard](first-run-setup-wizard.md) to establish a working
keyboard and pointing-device baseline, trial a proposed USB configuration, and
automatically restore that baseline if the user cannot confirm success.

## Scope

- Ask the user to type and operate their mouse, touchpad, or pointing stick.
  Identify physical sources in the domain handling the devices; sys-gui's
  virtual input devices alone do not identify the physical USB controller.
- Trace USB input to its parent PCI controller. Include working non-USB input
  paths and identify devices sharing controllers.
- Preserve at least one confirmed keyboard path and one confirmed pointing
  path, including forwarding to sys-gui. Reject plans that remove the last
  required path without providing a supported replacement for the trial.
- Store desired ownership and input forwarding declaratively in the machine's
  Nix configuration. Keep observed hardware evidence and trial outcome separate
  from the desired configuration.
- Evaluate a supported ownership arrangement for this machine. Do not mandate
  that dom0 or sys-usb own every controller; single-controller machines may need
  a usable fallback with reduced isolation.
- Exclude controllers needed for running system/installation storage. Detect
  USB dependencies for existing networking and defer reassignment of those
  controllers until a supported networking transition exists. If dependencies
  cannot be established, leave ownership unchanged and report why.

## Trial and recovery

Use the wizard's bounded trial protocol. Save the working baseline and arm the
independent rollback supervisor before changing ownership or forwarding.
Require fresh keyboard and pointer exercises, followed by explicit confirmation
within the deadline. Device presence, background input events, or a running
sys-usb must not automatically confirm the trial or extend its deadline.

Verify login and normal desktop input, reconnect behavior, and persistence
across reboot. Early-boot input, including disk unlocking where applicable,
needs separate verification; desktop success does not establish it.

Rollback must restore controller reservations, Qubes assignments, forwarding,
and the relevant boot configuration together. Switching only a dom0 NixOS
generation is insufficient unless it demonstrably restores that entire path.
Use reboot-based transitions where required, with persistent unconfirmed trial
state and a boot recovery mechanism. Do not attempt an unverified live handoff
of the only input controller. A failed trial returns to the known-good baseline
and offers a clear explanation without requiring reinstalling or networking.

## Acceptance criteria

- [ ] User exercises correctly identify USB and non-USB input paths and shared
  controllers on supported hardware.
- [ ] A proposed configuration preserves keyboard and pointing-device operation
  through login, desktop use, reconnect, and reboot.
- [ ] No input, partial input (keyboard-only or pointer-only), no confirmation,
  GUI failure, and sys-usb failure cause timed rollback without user input.
- [ ] Interrupted/rebooted trials recover the baseline without a reboot loop;
  restoration of actual ownership and forwarding is verified.
- [ ] System storage and existing USB connectivity are not disrupted.
- [ ] A single-controller machine can retain a working baseline with clearly
  described isolation limits when the proposed arrangement fails.

Use focused pytest checks for planning and trial state, then physical tests for
controller transitions and recovery. Implement deployable behavior in
`airlock-machine`; keep parent-repository integration in `airlock`.

USB Wi-Fi routing, networking setup, advanced routing, and stronger input
isolation are separate work. Do not add them to this milestone.
