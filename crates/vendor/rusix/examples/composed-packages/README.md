# Composed packages

This example connects four Rust package definitions: OpenSSL supplies curl and
Git, then curl supplies MariaDB. The remaining dependencies come from nixpkgs.
Each definition can also be used independently with ordinary nixpkgs dependencies.

## Source layout

[composition.rs](composition.rs) defines the connections. It uses
`NixExpression::bind` to share deferred `Package` values without evaluating them.
Each package's `mod.rs` exposes its factory; composition supplies selected
arguments through nixpkgs `callPackage`.

```rust
let pkgs = Nixpkgs::new();
let openssl: Package = pkgs.call_package(
    &openssl::factory(openssl::model::Release::Preview), arguments(),
);
let curl: Package = pkgs.try_call_package(
    &curl::factory(), curl_arguments(&pkgs, openssl.clone()),
)?;
let git: Package = pkgs.try_call_package(
    &git::factory(), git::arguments().with_openssl(openssl),
)?;
let mariadb: Package = pkgs.call_package(
    &mariadb::factory(),
    NixAttrs::try_from_record(mariadb::model::Release::V1011.arguments())?
        .merge(NixAttrs::new([("curl", curl)])),
);
```

The full assembly returns `NixAttrs<Package>` and binds OpenSSL and curl once.
Rust argument records convert at the call boundary. See
[typed package authoring](../../docs/typed-package-values.md) for the interfaces.

## Running

From the repository root:

```bash
cargo run --locked -p rusix --example composed-packages
```

The command prints Nix with a `packages` field containing the connected package
expressions. It does not evaluate or build them.

## Compatibility

[Composition tests](../../crates/rusix/tests/composed.rs) compare the graph with
an equivalently connected graph from pinned nixpkgs. They compare exact derivation
recipes and every output path, without normalizing anything that affects a hash.
Tags added with `overrideAttrs` prove that dependents receive the supplied Rust
packages. Changing OpenSSL's `withZlib` argument changes the downstream recipes
and continues to match the upstream graph.

The default graph uses `pkgs.curl`'s feature choices because that is MariaDB's
upstream dependency. The standalone curl example defaults to `curlMinimal`.
Both come from the same Rust factory, but their derivation identities differ.

These are three explicit dependency substitutions. MariaDB's direct OpenSSL
input and Git's curl input still come from nixpkgs. Composition leaves the rest
of the package set unchanged.

The tests cover both directions of interoperability:

| Connections |
| --- |
| nixpkgs OpenSSL → Rust curl → Rust MariaDB |
| Rust OpenSSL → Rust curl → Rust MariaDB |
| Rust OpenSSL → Rust Git |
| nixpkgs curl → Rust MariaDB |
| Rust OpenSSL → Nix curl |
| Rust curl, using Rust OpenSSL → Nix MariaDB |
| Rust OpenSSL → Nix Git |

They also cross OpenSSL and MariaDB releases and check that argument overrides
remain effective. Excluded dependencies and unused graph branches stay lazy.
Generated source shares factories and dependencies rather than duplicating them.

Failures in supplied Rust dependencies retain their operation origins and the
original Nix trace. The tests include delayed stdenv validation, where dependency
metadata connects the error to the supplied Rust child. See
[error reporting](../../docs/error-reporting.md) for examples and mapping limits.

Evaluation runs offline in disposable stores. These comparisons establish
recipe compatibility; they do not build or run packages. The standalone graph
uses `Nixpkgs::new()`'s x86_64-linux scope. The individual package suites cover
caller-supplied native and cross-compilation scopes.
