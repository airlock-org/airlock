# General USB lab

`usb-lab` is the development topology for a physical USB boot. It is not a
hardened profile: `sys-dev` is deliberately privileged and has a
development-only Qubes RPC shell into dom0.

```
development machine --Tailnet SSH--> sys-dev --airlock-dom0-shell--> dom0
                                      |
                                 sys-firewall <-- usr-browser
                                      |
                                   sys-net
                                      |
                                 physical NIC

usr-home stays offline
```

The user-facing `Airlock` boot entry uses this topology and starts `sys-dev`,
`sys-net`, `sys-firewall`,
the offline `sys-ctrl` bootstrap, physical-GPU `sys-gui`, offline `usr-home`,
and `usr-browser`. `sys-ctrl` is not yet a deployment authority. `usb-lab` is only
the internal profile name. The checked-in profile assigns physical network
devices to `sys-net`, with `sys-dev` and `usr-browser` routed through
`sys-firewall`. See the [migration checklist](todo/usb-lab-network-isolation.md)
for deployment and physical acceptance. A machine that has not applied the new
release still runs the previous `sys-dev` provider topology. Dom0's direct
NetworkManager, Tailscale, and SSH services are disabled in this profile.

## First boot

From the physical dom0 console, authenticate the new Tailnet node:

```sh
qvm-run --pass-io --user root sys-dev 'tailscale up --ssh'
```

Open the URL printed by Tailscale and approve `sys-dev`. From the development
machine, use Tailnet SSH to enter that VM, then open the intentionally enabled
dom0 session:

```sh
tailscale ssh user@sys-dev
airlock-dom0-shell
```

The dom0 shell runs as the ordinary `airlock` account; `sudo` remains
password-protected. Treat the entire profile, especially `sys-dev` and this
RPC permission, as development-only.

To remove remote control later, select a profile without `sys-dev`, or disable
the profile's `airlock.devDom0Terminal` host capability. The new networking
topology retains the Tailnet/development workflow in `sys-dev`.
