# Development access

`sys-agent` provides remote development access and has an explicitly enabled
Qubes RPC shell into dom0. This is a privileged development configuration,
not a hardened profile.

```text
development computer --Tailnet SSH--> sys-agent --airlock-dom0-shell--> dom0
                                           |
usr-home -------------------------------- sys-firewall
                                           |
                                         sys-net
                                           |
                                       physical NIC
```

The normal `Airlock` boot entry uses the internal `usb-lab` profile and starts
sys-gui, sys-agent, sys-ctrl, sys-net, sys-firewall, and connected usr-home.
Sys-net owns physical network devices. Sys-agent, sys-ctrl, and usr-home route
through sys-firewall. The retired sys-dev/usr-browser topology is historical;
[deployment notes](history/machine-deployment-lab-notes-2026-10-09.md) retain the
migration and acceptance evidence. Dom0's direct NetworkManager, Tailscale, and
SSH services are disabled in this normal profile.

## First boot

From the physical dom0 console, authenticate the new Tailnet node:

```sh
qvm-run --pass-io --user root sys-agent 'tailscale up --ssh'
```

Open the URL printed by Tailscale and approve `sys-agent`. From the development
machine, use Tailnet SSH to enter that VM, then open the intentionally enabled
dom0 session:

```sh
tailscale ssh user@sys-agent
airlock-dom0-shell
```

The dom0 shell runs as the ordinary `airlock` account; `sudo` remains
password-protected. Treat the entire profile, especially `sys-agent` and this
RPC permission, as development-only.

To remove remote control later, select a profile without `sys-agent`, or disable
the profile's `airlock.devDom0Terminal` host capability. The new networking
topology retains the Tailnet/development workflow in `sys-agent`.

## Physical update and rollback result (2026-09-25)

The corrected `usb-lab` release at
`/nix/store/8g8fncff0wl21p3dr7lf033wfd024pqb-airlock-machine-release`
was applied, rolled back to the booted image baseline, and reapplied on the
USB laptop. Each selection was followed by a normal reboot and
`make machine-verify DEPLOY_TARGET=sys-agent`; all three checks reported
`phase=complete` with matching host and running guest generations. Tailscale
SSH and HTTPS through `sys-firewall` and `sys-net` returned after each reboot.
The NIC assignments remained with `sys-net`; dom0 had no default route and
`usr-home` remained offline. The [network isolation trial](completed/usb-lab-network-isolation.md)
records transaction IDs and diagnostics; the [follow-up TODO](todo/usb-lab-network-followups.md)
tracks the remaining proxy, Wi-Fi, and test checks.
