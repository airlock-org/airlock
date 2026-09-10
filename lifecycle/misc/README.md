# Misc Lifecycle Support

Shared lifecycle support lives here.

- `lib/`: shared Nix schemas and builder libraries.
- `modules/`: shared NixOS modules consumed by image, bootstrap, and runtime configs.
- `tools/`: packages and scripts used by more than one lifecycle layer, plus local development and smoke-test helpers.

This is not a separate runtime layer. It is the common support area for code
that does not belong cleanly to only `image/`, `bootstrap/`, or `runtime/`.
