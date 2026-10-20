# curl package

This example defines curl 8.11.0 in Rust using existing nixpkgs builders and
dependencies. It implements the
[upstream factory](https://github.com/NixOS/nixpkgs/blob/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296/pkgs/by-name/cu/curlMinimal/package.nix)
from nixpkgs revision `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`.
The pinned `vendor/nixpkgs` submodule supplies the comparison reference.

## Source layout

| File | Purpose |
| --- | --- |
| [model.rs](model.rs) | Rust choices for TLS and optional protocol support |
| [inputs.rs](inputs.rs) | Deferred views of the upstream factory's arguments |
| [lowering.rs](lowering.rs) | Package recipe and feature-dependent behavior |
| [main.rs](main.rs) | Exports the factory and selected package as generated Nix |

## Authoring

Start with `model.rs` to choose a TLS backend or change protocol support:

```rust
let choices = model::Curl {
    tls: Some(model::TlsBackend::OpenSsl),
    http3: true,
    websocket: false,
};
let factory = lowering::factory();
let curl = rusix::interop::Nixpkgs::new()
    .try_call_package(&factory, choices.arguments())
    .expect("fixed authoring arguments");
```

`try_call_package` converts the Rust argument record. Nix later supplies the
remaining dependencies through `callPackage`. The result retains the `Package`
interface; the reusable factory is a `PackageFunction<Package>`.
See [typed package authoring](../../docs/typed-package-values.md).

## Running

From the repository root:

```bash
cargo run --locked -p rusix --example curl-nixpkg
```

The command prints `factory` and `curl` expressions. It does not evaluate or
build curl. Evaluation through Rusix stages the pinned source tree.

## Compatibility

The factory preserves upstream argument names and lazy defaults. Ordinary Nix
code can use `callPackage` and the resulting package's `override` and
`overrideAttrs` methods. The Rust enum allows at most one TLS backend. External
Nix callers retain the four independent TLS flags, checked by a native assertion.

The recipe calls the existing `stdenv.mkDerivation` through a `finalAttrs`
callback. Version overrides affect the source URL and changelog; recursive checks
receive the final package. The candidate does not import the original curl
expression. The same factory supports both `curlMinimal` and `pkgs.curl` choices.

Curl participates in bootstrapping `fetchurl`. Source preparation therefore avoids
`fetchpatch`; it appears only in the existing lazy passthru test graph.
Caller-supplied library helpers remain effective where upstream calls them.
Native operations remain independent of unrelated library overrides.

[curl tests](../../crates/rusix/tests/curl.rs) compare both factories in the same
package scope. They compare exact derivation recipes and output identities,
without normalizing hash-relevant values. Additional comparisons check feature
behavior and shell text with its Nix string context. Recursive passthru recipe
comparisons cover both default and version-overridden curl.

The tests vary TLS and other features across native and cross-compilation scopes.
They also check overrides and lazy defaults, including rejection of conflicting
TLS flags. Both implementations reject the pinned nixpkgs's unsupported Solaris
target; focused platform branch probes do not establish toolchain support.

Evaluation runs offline in disposable stores. These comparisons do not fetch
sources or build curl, and they do not execute its passthru tests.

[Error reporting](../../docs/error-reporting.md) shows how evaluation failures
connect to Rust. [Runtime diagnostics](../../docs/runtime-diagnostics.md) explains
how opaque calls and generated source spans affect that mapping.
