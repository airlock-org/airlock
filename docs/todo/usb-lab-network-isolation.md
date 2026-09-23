# USB lab network isolation

Status: planned; implementation, deployment, and physical acceptance pending.

## Goal and scope

Transition the existing `usb-lab` profile to dedicated `sys-net` and
`sys-firewall` domains. Keep working on this profile throughout; do not add an
experimental profile or a new boot entry. Preserve the current sys-dev
Tailscale development workflow and dom0 control access.

Today, sys-dev owns the physical NICs and provides networking to usr-browser.
usr-home stays offline. Definitions for sys-net and sys-firewall already exist
in airlock-machine's fuller bootstrap topology, but the USB lab does not use
them. Reuse those definitions and the existing Qubes networking machinery.

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

- [ ] Include sys-net and sys-firewall in the existing USB lab machine topology
  and prepared image closures. Keep deployable releases and fresh USB images
  consistent.
- [ ] Move physical network PCI ownership and automatic network-device
  discovery from sys-dev to sys-net. Ensure the same NIC cannot be claimed by
  both domains. Preserve required device options and account for reset behavior.
- [ ] Set sys-net's NetVM to none, sys-firewall's to sys-net, and sys-dev's and
  usr-browser's to sys-firewall. Set the profile's default NetVM to sys-firewall
  and retain usr-home's explicit offline configuration.
- [ ] Remove sys-dev's network-provider role and associated forwarding,
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
- [ ] Specify the apply order and how deployment continues after sys-dev loses
  its connection. Confirm the worker survives the transport loss and that
  resume/reboot handling cannot leave NIC ownership ambiguous.
- [ ] Have a physical dom0 console available for the NIC handoff. Document exact
  recovery and rollback commands before applying, including restoration of
  sys-dev's former PCI ownership and network-provider settings.
- [ ] Apply through the existing committed whole-machine deployment path,
  reconnect through sys-dev, and verify selected and running generations.

## Acceptance

- [ ] Add focused pytest coverage for topology, exclusive PCI ownership,
  startup dependencies, and migration/rollback behavior, including interruption
  during the handoff. Run the required repository checks and build the release.
- [ ] On hardware, confirm sys-net exclusively owns the intended NICs and
  establishes the required wired/Wi-Fi connections with working DNS.
- [ ] Confirm usr-browser and sys-dev reach the network through sys-firewall;
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
