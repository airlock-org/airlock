# Lifecycle Layers

This directory contains development-owned lifecycle support. Deployable
bootstrap, runtime, modules, and libraries live under `../airlock-machine/lifecycle`.

- `image/`: builds the bootable artifact.
- `misc/tools/scripts/`: development-host and lab orchestration.

The important config handoff is:

```text
image bundles bootstrap/initial-runtime-config
sys-config-vault creates the first current baseline from it
sys-config-staging imports current config from sys-config-vault for editing
```

Runtime commands, schemas, builder libraries, and NixOS modules belong in
`airlock-machine`.
