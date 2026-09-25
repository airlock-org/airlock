# USB lab network isolation

Status: fresh USB image booted with the target topology; physical acceptance is
in progress. The sys-firewall fail-closed check passed, but restart did not
restore downstream traffic. A guest hotplug fix is built locally and awaits a
new image and physical retest. Migration and rollback remain unverified.
Reviewed against the checked-in lab topology on 2026-09-25.

Follow the [hardware usability policy](../hardware-usability-policy.md). This
plan describes an operator-assisted lab migration with a physical recovery
console, not an unattended first-run transition. Before exposing this change
through the [setup wizard](first-run-setup-wizard.md), add its bounded trial,
live connectivity confirmation, and automatic rollback independently of the
network connection being changed. USB Wi-Fi routing is separate work; do not
assume PCI NIC discovery covers it or disrupt an existing USB connection.

## Goal and scope

Transition the existing `usb-lab` profile to dedicated `sys-net` and
`sys-firewall` domains. Keep working on this profile throughout; do not add an
experimental profile or a new boot entry. Preserve the current sys-dev
Tailscale development workflow and dom0 control access.

The previous deployed baseline gave sys-dev the physical NICs and routed
usr-browser through it. The fresh USB image now boots with sys-net and
sys-firewall using Qubes networking machinery. This is not evidence that the
in-place migration or rollback works. usr-home remains offline.

Target traffic paths:

```text
sys-dev     -> sys-firewall -> sys-net -> physical network
usr-browser -> sys-firewall -> sys-net -> physical network
usr-home    -> no NetVM
```

sys-net owns the physical NICs and network management. sys-firewall applies
guest firewall policy and provides the updates proxy where needed. sys-dev
keeps Tailscale, SSH, development tools, and its existing development-only dom0
access. Removing that privilege, introducing sys-ctrl, and USB isolation are
separate work.

## Configuration work

The checked-in configuration now includes both providers, routes sys-dev and
usr-browser through sys-firewall, and clears sys-dev's provider preferences,
features, and services explicitly. The deployable release's coordinator plans a
`pci_handoff`, checks ownership before stopping VMs, and unassigns the old
provider's network controllers after dependent VMs stop. Qubes setup then
assigns them to the new provider. The same operation runs in reverse for
rollback. The prepared USB guest-store entries follow the same machine guest
list. The release build passed on 2026-09-24; fresh USB image and physical
validation remain.

- [ ] Include sys-net and sys-firewall in the existing USB lab machine topology
  and prepared image closures. Keep deployable releases and fresh USB images
  consistent.
- [ ] Move physical network PCI ownership and automatic network-device
  discovery from sys-dev to sys-net. Ensure the same NIC cannot be claimed by
  both domains. Preserve required device options and account for reset behavior.
- [x] Set sys-net's NetVM to none, sys-firewall's to sys-net, and sys-dev's and
  usr-browser's to sys-firewall. Set the profile's default NetVM to sys-firewall
  and retain usr-home's explicit offline configuration.
- [x] Remove sys-dev's network-provider role and associated forwarding,
  firewall-provider, and physical-interface management services. Keep the Qubes
  networking services it needs as a client.
- [ ] Preserve Tailscale identity and persistent state in sys-dev. Move or
  reprovision physical connection settings needed by sys-net without embedding
  credentials in Nix closures or source control.
- [ ] Audit updates-proxy consumers and qrexec policy; configure only the access
  required by the USB lab. Keep dom0's direct networking disabled.
- [ ] Make startup readiness follow sys-net, sys-firewall, then networked
  clients. Include both providers in boot supervision, generation verification,
  and deployment restart/dependency handling.

Machine configuration and component tests belong in `airlock-machine`; image
seeding, deployment orchestration, and integration checks belong in `airlock`.
Follow the [repository workflow](../repositories.md) and
[whole-machine deployment procedure](../machine-deployment.md).

## Migration and recovery

- [ ] Inspect the actual deployed topology, PCI assignments, connection state,
  and available storage before preparing the migration. Record the starting
  release and a usable rollback generation.
- [ ] Build and stage the required closures while sys-dev still has working
  networking. Review a concrete deployment plan showing device transfers,
  NetVM changes, service changes, and required stops/restarts.
- [ ] Inspect `pci_handoff` in the machine plan and confirm it names
  `sys-dev` as `from` and `sys-net` as `to`. Preflight must find a network
  controller assigned to one of those domains before stopping VMs. Record the
  actual `qvm-device pci list --assignments dom0` output at the physical console.
- [ ] Specify the apply order and how deployment continues after sys-dev loses
  its connection. Confirm the worker survives the transport loss and that
  resume/reboot handling cannot leave NIC ownership ambiguous.
- [ ] Have a physical dom0 console available for the NIC handoff. Document exact
  recovery and rollback commands before applying, including restoration of
  sys-dev's former PCI ownership and network-provider settings.
- [ ] Apply through the existing committed whole-machine deployment path,
  reconnect through sys-dev, and verify selected and running generations.

At the physical dom0 console, inspect `sudo /var/lib/airlock/machine/controller
status` and the transaction's `setup/setup-config.json`. If the handoff was
interrupted, run `sudo /var/lib/airlock/machine/controller resume` to finish the
selected release, or `sudo /var/lib/airlock/machine/controller rollback` to
reselect the previous whole-machine release. The rollback plan must show
`pci_handoff` from `sys-net` to `sys-dev`; its coordinator releases the NICs
from sys-net, then the previous setup re-enables sys-dev's provider settings
and auto-assigns the network NICs. Check the resulting assignments and
`machine-verify` before restoring remote development access. Do not hand-edit
Qubes assignments as the final fix.

## Acceptance

Local checks on 2026-09-24: `make test-quick` passed (519 passed, 23 skipped)
and `make machine-build` produced release
`/nix/store/4wgc0jsq5r1ijfr57lk8b4s9ldqy83ph-airlock-machine-release`
from machine revision `dc2a9b4` and parent revision `6b7deb4`. These are build
results, not a live deployment. At that time `sys-dev-2` was absent from the
Tailnet and `sys-dev` was reachable; the dom0 development transport required
the operator's sudo password. No live machine plan or apply ran.
Fresh USB image checks on 2026-09-25 via Tailscale SSH to `sys-dev-2` and the
dom0 development shell: `sys-net` exclusively has the Realtek wired and Qualcomm
Wi-Fi controllers assigned and attached; sys-dev has neither. sys-net's
NetworkManager reports `ens6` connected via wired DHCP, with a default route
through `192.168.1.1` and working DNS. sys-firewall routes through sys-net;
sys-dev and usr-browser route through sys-firewall; usr-home has no NetVM and
could not resolve an external host. Dom0 has no active NetworkManager,
tailscaled, or sshd service and no default route. Both sys-dev and usr-browser
received HTTP 200 from `https://example.com`. A temporary Qubes firewall drop
rule blocked usr-browser's pinned HTTPS destination; removing the rule restored
HTTP 200 and the original accept policy.

After a physical reboot, Tailscale SSH and the dom0 development shell returned.
sys-net and sys-firewall were running, their NetVM chain and PCI assignments
were intact, sys-net regained wired DHCP and DNS, and sys-dev and a newly
started usr-browser again received HTTP 200. The boot setup, provider startup,
and autostart units became active in that order and remained active.
`airlock-machine-deploy status` reported
`phase=image-baseline` with no selected whole-machine release, as expected for
a fresh image; whole-machine generation agreement has not been exercised.

A detached test attempted to stop the providers and completed, but it cut off
remote access. Tailscale access had not returned more than three minutes after
the unit finished; an operator reboot restored it. Its detailed log was stored
in `/run`, which the reboot erased, so the individual stop, connectivity, and
restart results are unverified. The previous boot's qubesd journal shows
sys-firewall starting at 01:14:49 UTC and sys-net starting at 01:15:05 UTC
during the test. This is a recovery issue to investigate. The
installed updates-proxy policy permits template VMs to target sys-net, but this
image has no template VMs; no actual updates-proxy request was tested. The
in-place PCI handoff, whole-machine apply, and rollback were not exercised on
this fresh flash.

Provider recovery follow-up on 2026-09-25: a persistent dom0 JSONL probe
confirmed `sys-firewall` halted while both usr-browser and sys-dev failed a
pinned HTTPS request, with no alternate route. `qvm-start sys-firewall` then
succeeded, but both downstream requests still failed for 70 seconds. A separate
recovery timer found both providers running, restarted sys-dev's tailscaled,
and still could not connect from usr-browser. The logs were copied from the USB
to `.airlock-logs/network-provider/`; the sys-net shutdown was not attempted
after this failure. The pinned upstream Qubes agent disables Xen interface
renaming and explicitly restarts the uplink service on udev add; the old Airlock
configuration applied that naming rule only to sys-net and used `SYSTEMD_WANTS`
for all guest frontends. Revision `96f908c` applies the upstream hotplug pattern
to all guests. Focused configuration checks passed (2 tests), and a committed
USB-lab release built from machine revision `96f908c` and parent revision
`a7bed89` at `/nix/store/gzkl20hy69pgi4m8pamsbmsx37vxxys5-airlock-machine-release`.
The full quick suite had 558 passed, 23 skipped, and two stale assertions
unrelated to the hotplug change; both assertions passed after correction in
focused reruns. Review superseded that candidate with machine revision `eca7b84`: udev now
queues service jobs with `--no-block`, and the uplink service follows upstream
Qubes by leaving interface lifecycle handling to udev rather than binding to a
device unit. An isolated NixOS VM demonstrated that the earlier dependency
could fail startup before systemd processed the device event; adding
`--no-block` alone still failed interface recreation. The corrected service
passed three interface removal/recreation cycles, each with a new service
invocation and the expected interface address. A control with the previous
synchronous command and device dependency reproduced a dependency failure.
This refines the review's predicted deadlock: the tested systemd version
rejected the dependency immediately instead of waiting indefinitely.

Run the behavioral regression with
`AIRLOCK_RUN_UPLINK_VM=1 uv run pytest -q airlock-machine/tests/test_network_uplink_vm.py`.
Its build log and result path are under `$AIRLOCK_LOG_DIR/uplink-hotplug/`;
the result contains the guest journal and JUnit report. The test uses real
udev/systemd events, dummy interfaces, and a substitute address-setup command;
it does not verify Xen reconnection, QubesDB, provider forwarding, or Tailscale.
The existing USB can boot as a deployment target. Deploy the corrected
whole-machine release before repeating both provider restart tests, collecting
persistent guest interface, route, uplink-unit, and provider diagnostics as well
as traffic results. Hardware recovery remains unverified.

- [ ] Add focused pytest coverage for topology, exclusive PCI ownership,
  startup dependencies, and migration/rollback behavior, including interruption
  during the handoff. Run the required repository checks and build the release.
- [x] On hardware, confirm sys-net exclusively owns the intended NICs and
  establishes the required wired connection with working DNS.
- [x] Confirm usr-browser and sys-dev reach the network through sys-firewall;
  exercise an explicit Qubes deny rule and verify enforcement before restoring
  the intended policy.
- [ ] Confirm usr-home stays offline and stopping either provider prevents
  downstream connectivity without a direct-network fallback.
- [ ] Verify Tailscale SSH into sys-dev, the existing dom0 development shell,
  and the build/deployment workflow. Check any configured updates-proxy path.
- [ ] Cold boot and verify provider ordering, connection persistence, remote
  access, and whole-machine generation agreement.
- [ ] Exercise whole-machine rollback on hardware and verify that the previous
  topology restores NIC ownership and remote access, then reapply and verify
  the new topology.
- [ ] Update the USB lab runbook and topology diagram. Record implementation
  checks, deployed revision, hardware results, and remaining issues separately;
  a passing build alone does not complete this transition.
