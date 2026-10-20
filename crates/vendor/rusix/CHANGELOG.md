# Changelog

## 0.1.1

- Share the repository introduction between GitHub and crates.io.
- Add a library usage guide for evaluator setup and feature choices.
- Keep README links usable from both sites.

The Rust API and evaluation behavior are unchanged.

## 0.1.0

First public release.

- Define configuration through Rust structs and conversion macros.
- Generate lazy Nix expressions for packages and NixOS modules.
- Reuse existing Nix definitions through deferred typed interfaces.
- Evaluate offline with the optional `evaluation` feature.
- Connect Nix diagnostics to Rust definitions and operation locations.

Rust 1.88 or newer is required. Evaluation supports Linux with Nix 2.34.8.
Authoring and compilation can be used on Windows without default features.
The API is experimental. Saved diagnostic artifacts are version-specific.
