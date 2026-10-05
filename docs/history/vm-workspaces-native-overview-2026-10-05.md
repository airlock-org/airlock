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

The first worker failed at 09:51:03 UTC while stopping sys-agent:
`qvm-shutdown --wait --timeout 120 sys-agent` exited 1. Guest journal evidence
shows systemd reached poweroff and stopped journald, but the VM did not finish
shutdown within the coordinator deadline. The exact late shutdown cause remains
undetermined. No force kill or runtime workaround was applied.

After the operator rebooted, SSH returned. The actual dom0 boot ID was
`70051877-e13b-4425-8a76-78c76ada9091`. Status showed the previous successful
transaction and sys-gui generation `5qvhnb2fja19hlk49r0527366rv8kmw5`, so the new
display had not been activated. Sys-agent had no failed services or pending jobs.
The previous transient worker was no longer loaded after reboot; its default
`Result=success` property did not establish success. Its retained journal
established the failure above.

A fresh controller guest plan and apply reused the committed source. Retry worker
`airlock-machine-deploy-d1447d837aa0` targets candidate
`/var/lib/airlock/machine/candidates/4a722fab38c1cc5b93b59e6e7493da4541a849b618b2e751abed9e1d5b5b04e7`.
The operator confirmed GNOME restarted to its login screen during this retry.
The retained worker journal establishes failure at 10:45:23 UTC during verification:
`qrexec-client -d sys-agent 'user:readlink -f /run/current-system'` exited 125.
This is a qrexec communication failure, not evidence of an incorrect generation.
The successful transaction pointer was not advanced.

The restarted sys-agent booted at about 10:44:56 UTC. Its Xen console shows
Tailscale, qrexec and the GUI agent starting, and reaching multi-user startup at
10:45:02, then no further console output. Unlike healthy cold boots, it never
printed the serial login prompt. Later sys-gui calls timed out waiting for its
GUI channel, and the Tailscale enrollment launcher remained waiting on qrexec
until reboot. Host shutdown also failed to stop sys-agent normally. This supports
a persistent guest/startup failure affecting both qrexec and remote access;
the initial verification timeout alone does not explain the prolonged outage.

The failed guest boot's retained journal contains only early boot entries.
Its abandoned journal file was also inspected and passed `journalctl --verify`;
it did not recover later service diagnostics. No relevant host OOM, disk I/O
failure or storage exhaustion was found. The console's poweroff-time i8042 and
EHCI messages, and the busy `/lib/modules` unmount seen in other guests too,
do not establish the cause of the first failed shutdown.

A configuration lead is that sys-agent still uses HVM from its former sys-dev
role owning the physical network card. Physical networking moved to sys-net in
`dc2a9b4`, but HVM was retained. Sys-agent now has no assigned PCI devices;
usr-home and sys-ctrl use PVH. Qubes starts an additional emulated GUI path for
HVM guests unless configured otherwise. In the failed hot restart, the
sys-agent device-model log shows that GUI connecting and then EOF at 10:45:02,
coincident with the guest console stopping. Healthy cold-boot logs do not show
that sequence. This is a hypothesis about the HVM/emulated GUI startup path,
not proof of the precise kernel or device-model fault. Generic device-removal
messages also occur in healthy boots and are not crash evidence.

The deployment verifier has a separate readiness weakness: `verify()` makes
one qrexec attempt, whose internal connection timeout is shorter than the outer
180-second process deadline. The boot path already uses a bounded retry loop
in `wait_boot_guest()`. A prospective change should use bounded readiness retry
for deployment too, while retaining exact generation verification. The next
controlled startup test should also consider changing sys-agent to PVH through
the declared Nix preferences now that it has no hardware assignment. Neither
change was made during this investigation.

After the second operator reboot, dom0's actual boot ID was
`eba7098a-1080-48eb-86d8-f4e53a658e97`. Tailscale restored its persisted connection
without fresh enrollment. `make machine-verify DEPLOY_TARGET=user@100.104.92.101`
passed for the host and all six running guests against the previous selected
release, transaction `1790962552320744861`, revision `714b6c3`. Sys-gui is still
`5qvhnb2fja19hlk49r0527366rv8kmw5`; the native Activities change has **not** been
successfully deployed or manually verified. The status record's boot ID belongs
to its original transaction, not this reboot.

This postmortem used read-only live inspection. No services or VMs were restarted,
and no diagnostic configuration overlays or runtime workarounds were applied.
Captured logs are in `postmortem.json`; the recovered baseline verification is
in `postmortem-baseline-verify.log` in the evidence directory below.

Evidence: `.airlock-logs/vm-workspaces-native-overview/` and the build record under
`.airlock-logs/machine-deploy/52f1cfcdfb8eebd83fe1c8a31daf7cd8dfef1217/`.
