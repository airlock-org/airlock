# Lifecycle Layers

This directory contains the Nixome lifecycle layers.

- `image/`: builds the bootable artifact.
- `bootstrap/`: first-boot wiring that makes the image manageable.
- `runtime/`: code used after bootstrap to operate the running system.
- `misc/`: shared libraries, modules, and cross-stage tooling.

The important config handoff is:

```text
image bundles bootstrap/initial-runtime-config
sys-config-vault creates the first current baseline from it
sys-config-staging imports current config from sys-config-vault for editing
```

`runtime/tools/` contains runtime-only operational commands. If a tool is used
by more than one lifecycle layer, it belongs under `misc/tools/`. Shared Nix
schemas, builder libraries, and modules belong under `misc/lib/` and
`misc/modules/`.
