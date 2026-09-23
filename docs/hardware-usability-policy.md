# Hardware usability and isolation

Status: agreed design policy; implementation and hardware verification remain.
Recorded 2026-09-23.

## Principle

Working local input and connectivity are acceptance conditions for Airlock's
hardware security configuration. Start from a working configuration, introduce
isolation in recoverable steps, and retain changes only after verification.
A security configuration that leaves the user unable to operate the machine
is unsuitable for that machine. Recovery must not require reinstalling.

The first-run wizard is the primary place for users to test proposed changes
on their actual hardware. Detection, successful builds, and running service VMs
are not proof that typing, pointing, or networking works.

## Required behavior

- Establish and record a working baseline before changing ownership or routing.
  Include dependencies for input, system storage, and existing connectivity.
- Explain the proposed change and recovery behavior before starting a trial.
- Require live user testing and explicit confirmation before retaining a trial.
  No input or no confirmation by a fixed deadline triggers automatic rollback.
- Run rollback supervision outside the GUI and affected service VM. Loss of
  their input, display, or execution must not disable the timeout.
- Preserve a complete known-good configuration and recover it predictably.
  An older boot entry alone is insufficient if the user cannot select it.
- Describe reduced isolation accurately. A usable fallback must not silently
  become a claim of full isolation, or permission to trust every new USB device.

Automatic recovery has limits: software cannot guarantee recovery from a
wedged host or failed hardware. Persist trial state so the next boot can recover
without requiring the broken input path. Provide and test a manual recovery
route for cases requiring a restart or power cycle; do not promise seamless
recovery where hardware evidence does not support it.

## Hardware constraints

USB controller ownership applies to all devices on that controller. Multiple
suitable controllers may allow separation of input and general USB. A machine
with one controller may not support that separation. Accept this limitation
and preserve usability with an explicit description of the resulting exposure.

No universal dom0-versus-sys-usb ownership policy is agreed here. Keep essential
devices working while evaluating the supported arrangements. Do not transfer a
controller required for the running system's storage, or disrupt the user's
only USB network connection as an incidental input-setup action.

Trusting the base machine and explicitly selected existing devices is a working
assumption, not proof that installation-time devices are safe. New-device
admission remains a separate decision. A temporary inspection VM cannot certify
that a device is safe for later unrestricted use in dom0.

## Decisions and open questions

Agreed: usability is required, isolation is incremental, trials require live
confirmation and timed rollback, hardware limits are acceptable, and protection
must be described honestly.

Open: hardware-dependent ownership selection, exact timeout durations,
controller handoff and reboot mechanics, early-boot USB keyboard handling,
and USB Wi-Fi routing. Resolve these through focused implementation plans and
hardware testing rather than assuming them solved by this policy.

## Implementation plans

- [First-run setup wizard](todo/first-run-setup-wizard.md): interaction, lifecycle,
  timed trials, and confirmation.
- [USB keyboard and pointing-device setup](todo/usb-input-setup.md): initial
  hardware use case, input preservation, and verification.

USB Wi-Fi routing needs a separate plan. The input milestone must preserve
existing connectivity but does not promise support for new Wi-Fi adapters.
