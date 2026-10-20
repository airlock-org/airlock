# PostgreSQL NixOS module

This example rewrites a complete upstream NixOS module in Rust/Rusix: both its
public option declarations and its configuration implementation. Ordinary NixOS
modules can configure it through the same `services.postgresql.*` interface.
PostgreSQL packages still come from nixpkgs through opaque Nix handles.

The compatibility reference is nixpkgs revision
`8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296`. This commit ID pins the exact nixpkgs
source tree containing the original
[postgresql.nix](https://github.com/NixOS/nixpkgs/blob/8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296/nixos/modules/services/databases/postgresql.nix),
NixOS library and package definitions used in comparisons. Tests use the pinned
`vendor/nixpkgs` submodule offline.
The minimal staged subset retains its SHA-256 checksums in
[nixpkgs-pin.json](../../crates/rusix/src/nixos/nixpkgs-pin.json).

| File | Purpose |
|---|---|
| [model.rs](model.rs) | User-defined Rust types and an example PostgreSQL configuration |
| [schema.rs](schema.rs) | Public NixOS option declarations, defaults, documentation and migrations |
| [options.rs](options.rs) | Finite symbolic dependencies on final merged NixOS values |
| [lowering.rs](lowering.rs) | PostgreSQL compatibility logic using those dependencies and existing Nix helpers |
| [main.rs](main.rs) | Composes the contributions and prints generated Nix |

## Authoring

Start with `model.rs`. Rust models express provisioning intent; symbolic option
references let dependent outputs follow ordinary downstream NixOS overrides.
NixOS remains responsible for checking and merging the generated module.

`Database::Owned` derives a database and its matching role from one name.
Unowned databases and independent roles are also supported. Role clauses distinguish
omission (`None`) from explicit null (`Some(Clause::Preserve)`). `Enable` grants
the role attribute; `Disable` revokes it. PostgreSQL settings remain an open map.

Option declarations and implementation defaults remain separate. NixOS validates
the final definitions against the declared schema. Symbolic references resolve
after merging, so ordinary Nix modules can override the port or package without
regenerating the Rust module. `Nixpkgs::from_module()` preserves the NixOS package
set and its overlays.

The generated module must replace the upstream PostgreSQL module in the import
list. Importing both would contribute two implementations of the same service.
The [comparison fixture](../../tests/fixtures/postgresql-equivalence.nix) shows
how the tests replace that import.

## Running

From the repository root:

```bash
cargo run --locked -p rusix --example postgresql-nixos-module
```

## Compatibility

[Implementation tests](../../crates/rusix/tests/postgresql.rs) and
[schema tests](../../crates/rusix/tests/postgresql_schema.rs) compare evaluated
results against the pinned upstream module in disposable isolated Nix stores.
The candidate imports no original PostgreSQL module.

Implementation comparisons cover generated configuration and service definitions.
Store paths and Nix string contexts are compared exactly; SQL ordering is preserved.
The harness sorts the set-like `pathsToLink` list and normalizes the recovery
symlink command's line continuation. A separate test checks that the original and
normalized commands retain identical arguments and Nix string context.

Schema comparisons cover declarations and migration options, including defaults
and behavior under ordinary Nix contributions. The
[downstream fixture](../../tests/fixtures/postgresql-downstream.nix) changes the
configuration through the same generated artifact.

Some aggregated NixOS assertions retain their Nix reason without a specific Rust
input location. Raw input strings retain upstream quoting behavior. These tests
cover module evaluation; they do not build or start PostgreSQL or run SQL.
