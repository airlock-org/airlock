# Installed-machine operations

Open usr-assistant: Console and work from `~/airlockos-machine`. The checkout and
private application data survive system updates. Local changes do not require pushing to an upstream
remote.

## Plan and apply

Before editing system configuration:

```sh
airlock machine status
airlock machine verify
```

Compare the recorded deployed source with the checkout, including committed and
uncommitted changes. Stop for older undeployed work, an incomplete or failed
deployment, a required reboot, or an unknown baseline. Follow the checkout's
`AGENTS.md` for resolving and rechecking that state.

A fresh USB reports `phase: image-baseline` and `selection: null`. Its source is
in `baseline.revision` and `baseline.lock_hash`; successful `verify` checks that
image and the running VM generations before the first deployment. An
uninitialized baseline means startup must finish before verification can pass.

After making and committing the intended changes:

```sh
airlock machine plan
```

Airlock Control requests source approval before evaluation and building.
An approved reply means work can continue; it does not establish success. Follow
the returned request ID and wait for the completed plan:

```sh
airlock machine review-status --request-id REQUEST_ID
```

Review affected VMs, restarts, and any required dom0 reboot before activation:

```sh
airlock machine apply
airlock machine status
airlock machine verify
```

An unchanged approval can cover plan and apply for the same source and baseline.
For changes limited to an existing VM, use `airlock machine guest-plan --target VM`
and `airlock machine guest-apply --target VM`. Topology, new VMs, routing,
preferences, and dom0 policy use the whole-machine route.

## Recovery

Activation runs in a detached dom0 worker. A usr-assistant restart closes its
applications and remote connections; reconnect and inspect status afterwards.
If a dom0 reboot is required, verify again after the machine restarts.

Inspect status before `airlock machine resume` or `airlock machine rollback`.
These typed recovery interfaces remain available when sys-ctrl is stopped.
Rollback restores system software and configuration, leaving private application
data at its current state. A returned request or completed build is not proof
of deployment; verify the worker outcome and running generations.

The installed Nix command selects usr-assistant's pool-backed builder. Do not run
external-development GC targets against it or collect the shared pool or dom0
store. State and logs are under `~/.local/state/airlock`.
