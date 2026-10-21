# Development operations

Run from the parent airlock checkout. Both repositories must be committed, and
`airlock-machine` must match the parent pin. `DEPLOY_TARGET` is the confirmed
SSH target for the machine's usr-assistant VM. The workstation commands require
stable Rust; their launcher builds the local development tool before running it.
This tool is separate from the applications installed in VMs.

## Deployment

Before editing system configuration, inspect its recorded deployed source and
compare it with the checkout, including committed and uncommitted changes:

```sh
make machine-status DEPLOY_TARGET=user@TARGET
make machine-verify DEPLOY_TARGET=user@TARGET
```

Stop for older undeployed work, incomplete or failed deployments, a required
reboot, or an unknown baseline. Resolve that state before preparing another
change. Follow the full baseline and submission rules in the root [AGENTS.md](../AGENTS.md).

After making and committing the intended changes and updating the parent pin:

```sh
make machine-plan DEPLOY_TARGET=user@TARGET
```

Source approval occurs before evaluation and building. An approved or accepted
request is not a completed plan or deployment. Follow its returned request ID
inside usr-assistant with `airlock machine review-status --request-id REQUEST_ID`
and wait for the plan. Review its affected VMs, restarts, and reboot requirement
before applying:

```sh
make machine-apply DEPLOY_TARGET=user@TARGET
make machine-status DEPLOY_TARGET=user@TARGET
make machine-verify DEPLOY_TARGET=user@TARGET
```

Activation uses a detached dom0 worker. Restarting usr-assistant can disconnect
the client while deployment continues. Reconnect and check the worker outcome.
If dom0 requires a reboot, verify again after restarting the machine. A build,
approval, or accepted submission does not establish a verified deployment.

For recovery, inspect status before `make machine-resume` or
`make machine-rollback`, using the same `DEPLOY_TARGET`. Rollback changes system
software and configuration; it does not reverse private application data.

## Nested-VM checks

The workstation Rust tool owns image selection, writable overlays, baseline reuse,
SSH orchestration and test evidence. Shell entry points pass arguments to that
command. Warm tool sync is an explicit development override inside the disposable
guest; it is separate from the reviewed deployment path.

```sh
make test-warm
make test-slow
```

These explicitly build and run nested-VM checks. They require the external Nix
store and suitable virtualization support; use disposable test state. Test
selection and VM/storage options are defined in the root Makefile. These checks
do not verify physical hardware or establish Qubes OS security equivalence.

The ordinary `make test-quick` loop remains free of Nix builds and automatic
tool provisioning. Prepare native executables explicitly with `make native-integration-build` first.
