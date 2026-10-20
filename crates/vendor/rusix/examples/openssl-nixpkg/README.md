# OpenSSL authored in Rusix

This example defines OpenSSL build recipes in Rust for three releases from the
pinned nixpkgs checkout. Start with [model.rs](model.rs) to choose a release, then
[main.rs](main.rs) to see how that choice becomes generated Nix.

The authoring code uses an ordinary Rust enum:

```rust
use rusix::{interop::Nixpkgs, nix_record};

let release = model::Release::Preview;
let factory = lowering::factory(release);
let openssl = Nixpkgs::new().call_package(&factory, nix_record! {});
```

A package factory is a Nix function that accepts dependencies and feature choices
and returns a build recipe. `call_package` describes a Nix `callPackage` call:
it fills unspecified arguments from the package set. The empty argument record
above keeps the factory's Nix defaults. Rust constructs these expressions without
evaluating Nix or building OpenSSL.

From the repository root:

```bash
cargo run --locked -p rusix --example openssl-nixpkg
```

The executable prints `family` as a function returning all three releases and
`openssl` as the selected package expression.
`Preview` means OpenSSL 3.3.2 at this pin; `Lts` means 3.0.15 and `Legacy` means
1.1.1w. These labels describe the pinned checkout's policy.
To select another pinned release, change the enum variant in `main.rs`.

| File | Purpose |
| --- | --- |
| [model.rs](model.rs) | Release choices, fixed versions and source archive checksums |
| [inputs.rs](inputs.rs) | References to dependencies and feature arguments supplied later in Nix |
| [lowering.rs](lowering.rs) | Shared build recipe and release-specific patches and metadata |
| [scripts.rs](scripts.rs) | Shell text for build phases; Rust does not execute these commands |
| [main.rs](main.rs) | Selects a release and prints the generated Nix |
| [mod.rs](mod.rs) | Exposes the factories for other Rust examples |

## Recipe and compatibility details

The semantic reference is `pkgs/development/libraries/openssl/default.nix`
(337 lines, 20 public arguments, eight lazy defaults) at nixpkgs
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`.
The complete argument view supplies `args::argument_names()` to both factories,
including unused upstream parameters. Naming annotations cover exceptions such
as `enableSSL2`; ordinary names use the default lowerCamelCase mapping.
Fixed opaque records use `nix_record!`, with iterators retained for release families
and dynamic target tables.
Boolean conjunction uses `Expr<bool>::and`; typed list concatenation uses
`NixList::concat` and dynamic lists use `NixValue::concat_lists`. Version comparisons use the supplied
`NixLibrary::version_at_least` and `version_older`. Text replacement uses
`NixValue::replace_text`; native attribute checks and fallbacks use `has_attr`
and `attr_or`, including dynamically selected configuration targets.
These shared helpers preserve lazy evaluation
and Rust call locations without local copies of their implementations.

All package policy is Rust authored. The backend remains the caller's nixpkgs
library, fetchurl, stdenv.mkDerivation, outputs, wrappers, setup hooks and tests.
Unused upstream arguments remain in the public interface. Shared recipe parameters
stay lexically captured: overriding `version` through `overrideAttrs` does not
change the upstream source URL or changelog. The recursive pkg-config test does
observe `finalAttrs.finalPackage`.

OpenSSL bootstraps fetchurl. Patches therefore use checked-in files from the pin;
introducing fetchpatch here would create a bootstrap dependency cycle. No source
or patch is downloaded and no package is built by this experiment.

## Verification

Run `cargo test --locked -p rusix --test openssl`. The suite compares exact
`.drv` identities and recipe bytes, output paths, source recipes, patch bytes and
store paths, phase text and string contexts, flags, metadata, family membership,
argument defaults and recursive pkg-config tests. It covers every release,
individual feature switches, configuration files, static builds, native Linux
and Darwin variants, cross Linux/musl/MinGW/FreeBSD/RISC-V/MIPS, attribute overrides,
excluded dependency laziness and Rust provenance for a failing fetcher call.
Artifacts are saved under `target/openssl-equivalence/`.

Patch source paths refer to either the checked checkout or its session symlink;
comparisons inspect their actual bytes and Nix store paths rather than temporary
filesystem spelling. Metadata's source `position` is excluded because the package
is now authored in Rust. Neither exclusion changes or normalizes derivation recipes.

The independent factory returns `PackageFunction<Package>` and the family factory
returns `PackageFunction<NixAttrs<Package>>`. The supplied `Stdenv`, `NixLibrary`,
fetcher callable, package dependencies, phase text and dependency/flag lists retain
their Rust interfaces. No package-family schema was added to core.
See [typed package authoring](../../docs/typed-package-values.md).
