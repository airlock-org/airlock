# Git package

This example defines Git 2.47.0 in Rust using existing nixpkgs builders and
dependencies. It implements the
[upstream package](https://github.com/NixOS/nixpkgs/blob/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296/pkgs/applications/version-management/git/default.nix)
from nixpkgs revision `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`.
The pinned `vendor/nixpkgs` submodule supplies the comparison reference.

## Source layout

| File | Purpose |
| --- | --- |
| [model.rs](model.rs) | Rust feature choices, with Perl-dependent features grouped together |
| [inputs.rs](inputs.rs) | Deferred argument views and conversion from the Rust model |
| [lowering.rs](lowering.rs) | Package recipe and feature-dependent behavior |
| [main.rs](main.rs) | Exports the factory and selected package as generated Nix |

## Authoring

Start with `model.rs` to change Git's features. These choices exclude Perl and
its dependent helpers:

```rust
let choices = model::Git {
    perl: model::Perl::Disabled,
    manual: false,
    ..model::Git::defaults()
};
let factory = lowering::factory();
let git = rusix::interop::Nixpkgs::new()
    .try_call_package(&factory, inputs::arguments(choices))
    .expect("fixed authoring arguments");
```

`try_call_package` converts the Rust argument record. Nix later supplies the
remaining dependencies through `callPackage`. The result retains the `Package`
interface; the reusable factory is a `PackageFunction<Package>`.
See [typed package authoring](../../docs/typed-package-values.md).

## Running

From the repository root:

```bash
cargo run --locked -p rusix --example git-nixpkg
```

The command prints `factory` and `git` expressions. It does not evaluate or
build Git. Evaluation through Rusix stages the pinned source assets.

## Compatibility

The factory preserves upstream argument names and lazy defaults. Ordinary Nix
code can use `callPackage` with upstream's explicit framework and Perl-library
arguments. The resulting package supports `override` and `overrideAttrs`.
The Rust model groups related feature choices; external
Nix callers retain the upstream arguments and native feature assertions.

The recipe calls the existing `stdenv.mkDerivation`. Platform values and fetchers
come from nixpkgs, as do the remaining dependencies. Patches and `update.sh` remain
pinned source assets. The candidate does not import the original Git expression.
Its `finalAttrs` callback retains the final-package reference used by the upstream
install-check recipe.

[Git tests](../../crates/rusix/tests/git.rs) and the
[comparison fixture](../../tests/fixtures/git-equivalence.nix) instantiate both
factories in the same package scope. They compare exact derivation recipes and
output identities. Hash-relevant values are preserved; source asset paths are
compared through their Nix store paths. Additional comparisons check scripts and
their string contexts, metadata and the final-package install-check recipe.

The tests vary feature choices and caller-supplied platform scopes. They also
check overrides and lazy defaults, including failures from invalid inputs.
Both implementations reject the pinned nixpkgs's unsupported Solaris target;
a focused SunOS branch probe does not establish toolchain support.

Evaluation runs offline in disposable stores. These comparisons do not fetch
sources or build and execute Git.

Package assertions can map to Rust lowering locations. An upstream failure may
map only to the Rust call boundary; the original Nix trace is retained. See
[error reporting](../../docs/error-reporting.md) for examples and limits.
