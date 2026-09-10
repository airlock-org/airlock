# Lifecycle Layers

This directory contains the three lifecycle phases for Nixome.

- `image/`: builds the bootable artifact.
- `bootstrap/`: first-boot wiring that makes the image manageable.
- `runtime/`: code used after bootstrap to operate the running system.

The important config handoff is:

```text
image bundles bootstrap/initial-runtime-config
config-vault creates the first current baseline from it
config-staging imports current config from config-vault for editing
```

`runtime/tools/` contains the shared Nix libraries, schemas, modules, packages,
and scripts used by image, bootstrap, dom0, and the config VMs.
