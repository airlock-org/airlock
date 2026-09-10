# Image Layer

Builds bootable Nixome OS artifacts.

Image profiles live in `profiles/`. They should describe what goes into a
bootable test or VM image, not the ongoing runtime configuration edited after
the system is running.

## Image Mode

Nixome VM image outputs are split into explicit development and release flavors.
Development mode is a build-time image profile flag, not a runtime switch.

Development images enable dom0 conveniences for nested work: the
`/etc/nixome/dev-mode` marker, extra dom0 tools, test/observation commands,
and a loopback HTTP(S) proxy that forwards through sys-net. Release images
leave those dom0 conveniences disabled. The sys-net topology is the same in
both modes.

Build with Make:

```bash
make vm-gui-config-image
make vm-gui-config-image VM_IMAGE_MODE=release
```

Build explicit flake outputs:

```bash
nix build .#nixome-vm-gui-config-dev-image
nix build .#nixome-vm-gui-config-release-image
```

The historic unsuffixed image outputs remain development-mode aliases for now.

Image-only helper packages or scripts belong in `tools/`. Cross-stage helpers
belong in `../misc/tools/`.
