# General USB lab

`usb-lab` is the development topology for a physical USB boot. It is not a
hardened profile: `sys-dev` is deliberately privileged and has a
development-only Qubes RPC shell into dom0.

```
development machine --Tailnet SSH--> sys-dev --airlock-dom0-shell--> dom0
                                      |
                                  physical NIC
                                      |
                           usr-browser; usr-home stays offline
```

The user-facing `Airlock` boot entry uses this topology and starts `sys-dev`,
physical-GPU `sys-gui`, offline `usr-home`, and `usr-browser`. `usb-lab` is only
the internal profile name. `sys-dev` owns the physical network devices until a
[planned change to this same profile](todo/usb-lab-network-isolation.md)
introduces `sys-net` and `sys-firewall`. Dom0's direct
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
the profile's `airlock.devDom0Terminal` host capability. The planned networking
stage keeps the Tailnet/development workflow but moves physical NIC ownership
to `sys-net`, with `sys-dev` routed through `sys-firewall`.
