# Image Layer

Builds bootable Nixome OS artifacts.

Image profiles live in `profiles/`. They should describe what goes into a
bootable test or VM image, not the ongoing runtime configuration edited after
the system is running.

Image-only helper packages or scripts belong in `tools/`. Cross-stage helpers
belong in `../misc/tools/`.
