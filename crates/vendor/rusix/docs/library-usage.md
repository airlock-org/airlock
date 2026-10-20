# Using Rusix from Rust

Add one dependency to your own library or executable:

```toml
[dependencies]
rusix = "0.1"
```

Configuration macros and derives are re-exported by `rusix`; Cargo brings in
`rusix-derive` automatically. You can also rename the dependency in Cargo.toml.

## Evaluation

Evaluation runs offline in a disposable Nix store. It does not build packages or
activate a system.

```rust
use rusix::{Expr, IntoConfig, NixSession, compile};

#[derive(IntoConfig)]
struct Output {
    /// Integer computation deferred until Nix evaluates this field.
    answer: Expr<i64>,
}

let generated = compile(Output { answer: Expr::int(84).divide(Expr::int(2)) })
    .expect("valid Rust configuration");
let session = NixSession::new().expect("temporary evaluation workspace");
let result = session.evaluate(&generated).expect("successful Nix evaluation");
assert_eq!(result.value["answer"], 42);
```

Compilation requires no Nix process. The default `evaluation` feature provides
`NixSession`; evaluation is supported on Linux with Nix 2.34.8. It requires `nix`
and `nix-instantiate` on PATH. Rust 1.88 or newer is required.

For authoring and compilation alone, including on Windows:

```toml
[dependencies]
rusix = { version = "0.1", default-features = false }
```

## nixpkgs and NixOS

For expressions using nixpkgs or NixOS, supply an
existing clean local checkout matching `rusix::nixos::NIXPKGS_REVISION`:

```rust
let session = rusix::NixSession::with_nixpkgs("/path/to/nixpkgs")
    .expect("clean pinned nixpkgs checkout");
```

Rusix verifies the checkout without fetching anything. Keep it unchanged for the
lifetime of the session. Repository development uses the pinned `vendor/nixpkgs`
submodule automatically; registry consumers supply their own checkout.

## Evaluation by an existing Nix project

A project can save `compile_module`'s `NixosArtifact` alongside its generated
module and import that module into its own NixOS configuration. It keeps control
of nixpkgs, overlays and evaluation. Supply module packages through
`Nixpkgs::from_module()` so they resolve from that configuration's `pkgs`.

After an external evaluation fails, call `artifact.diagnose` with the failure
kind, raw stderr, saved module filename and the project's nixpkgs source path.
This recovers Rust locations without launching Nix or selecting a package pin;
it is available with default features disabled. Preserve the exact artifact and
source together. Unknown diagnostic formats retain the original Nix error and
available source locations rather than claiming complete attribution.

## Public APIs

Start with `rusix::prelude` for authoring, `rusix::interop` for existing Nix values,
and `rusix::nixos` for system configuration. `rusix::ir` and
`rusix::compiler::ast` support explicit inspection and compiler extensions.

## Compatibility

Rusix is experimental: minor releases before 1.0 may change APIs. Saved diagnostics
and source maps should be read with the same Rusix version that produced them.

See the [examples](../examples/README.md) for complete definitions and
[development guide](development.md) for repository setup and fixture tools.
