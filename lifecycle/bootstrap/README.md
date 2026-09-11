# Bootstrap Layer

Defines the first-boot bridge between an image and a manageable runtime.

This layer owns the generated config-domain defaults and the starter runtime
configuration bundled into the image.

On first boot, `sys-config-vault` turns `initial-runtime-config/` into the first
authoritative current config. `sys-config-staging` later imports that current config
from vault when it needs an editable workspace.

Bootstrap-only helper packages or scripts belong in `tools/`. Cross-stage
helpers belong in `../misc/tools/`.
