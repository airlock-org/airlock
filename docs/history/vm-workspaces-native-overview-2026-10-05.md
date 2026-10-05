# Native Activities interaction for VM workspaces, 2026-10-05

Component `5f4b991`, pinned by parent `52f1cfc`, replaces the separate VM workspace
toolbar with a passive display inside GNOME's existing Activities button. The
previous toolbar duplicated the native workspace indicators, highlighted each
VM label and dot independently, and switched workspaces rather than opening
GNOME's zoomed-out overview.

The existing Activities actor, click gesture, keyboard handling, scrolling,
drag behaviour, accessibility role and panel placement are retained. Only its
child is replaced with nonreactive VM labels and coloured workspace dots. This
provides one native hover/checked surface and preserves the original overview
open/close animation. There is no add-workspace menu. The native indicators are
kept detached and restored when the extension is disabled. The saved VM layout,
trusted identity reader, unrestricted window movement and lock-screen lifecycle
are unchanged. No guest agent or Qubes authority changes are involved.

Local validation passed **837 quick tests, 59 skipped**, lint and Python
compilation. Tests exercise passive descendants, retaining native input-handler
objects, opening/closing the overview without switching workspace, preventing a
second panel control, preserving display actors on unchanged inventory, and
restoring native indicators/accessibility on disable. GNOME 50.4's upstream
Activities implementation was inspected to confirm the integration point.

The whole-machine release built successfully:
`/nix/store/90qh7fd4whrpwf21w7alxihbfm24i69w-airlock-machine-release`.
The built extension package and file hashes were recorded. Guarded Airlock GC
skipped below its threshold. Source was pushed to the clean sys-agent checkout.

The controller guest plan targets only sys-gui, restarting sys-gui, sys-agent
and usr-home for activation, with no host reboot required.
Guest apply submitted detached worker `airlock-machine-deploy-809ed0fe3238`
for candidate
`/var/lib/airlock/machine/candidates/87250bff1d9298f6ce6304f6a208b46db51824770efb6e3a1982d237eee4b87f`.
The expected new sys-gui generation is
`/nix/store/pb270518irz1amq66kf07gm17mycg497-nixos-system-sys-gui-26.11`.

Live verification remains pending: after dependent VM shutdown, sys-agent went
offline in Tailscale and repeated bounded SSH probes timed out. Its return,
worker exit status, running generations and signed-in extension state have not
yet been observed. The operator was asked what the physical laptop displays.
No recovery mutation, host reboot or diagnostic configuration overlay was made.
A submitted apply is not recorded as a successful live deployment.

Evidence: `.airlock-logs/vm-workspaces-native-overview/` and the build record under
`.airlock-logs/machine-deploy/52f1cfcdfb8eebd83fe1c8a31daf7cd8dfef1217/`.
