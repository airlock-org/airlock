# USB lab network follow-up checks

Status: open. The wired `sys-dev → sys-firewall → sys-net` topology, provider
recovery, and whole-machine apply/rollback/reapply passed physical acceptance
on 2026-09-25. See the [completed network isolation trial](../completed/usb-lab-network-isolation.md)
for the deployed revision, transaction IDs, and evidence. These checks were
outside that trial's verified scope.

## Focused automated coverage

- [ ] Add pytest coverage for the declared NetVM topology, exclusive PCI NIC
  ownership, provider startup dependencies, and migration/rollback planning.
- [ ] Exercise interruption and resume or rollback during the PCI handoff in
  an appropriate test harness. Assert that the plan cannot leave ambiguous NIC
  ownership or give sys-dev a direct-network fallback.
- [ ] Run the required repository checks and retain machine-readable results
  under `.airlock-logs/`.

## Updates proxy

- [ ] Audit actual updates-proxy consumers and qrexec policy for the USB lab;
  grant only the required access while dom0 direct networking stays disabled.
- [ ] Make an end-to-end updates-proxy request from a configured consumer and
  confirm it traverses the intended provider. The accepted image had no
  template VM, so its installed policy alone did not prove this path works.

## Wi-Fi and persistence

- [ ] Connect the assigned Qualcomm PCI Wi-Fi controller in sys-net and verify
  downstream DNS and HTTPS, Tailscale SSH, and recovery after stopping and
  restarting each provider. The accepted trial used wired Ethernet only.
- [ ] Check exact private state preservation across a future apply/rollback
  with a nonsecret sentinel. The accepted trial showed retained Tailscale
  enrollment, but did not check an exact user-data sentinel.

USB Wi-Fi adapter routing is separate from the assigned PCI Wi-Fi controller.
The [setup wizard](first-run-setup-wizard.md) separately owns bounded trials,
live connectivity confirmation, and automatic rollback before exposing this
transition as an unattended user flow.
