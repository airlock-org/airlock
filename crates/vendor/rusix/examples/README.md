# Examples

These examples define configuration and packages in Rust, then print the generated
Nix. The smaller examples introduce individual concepts; the larger ones implement
existing nixpkgs packages and a NixOS module.

Start with [enum-option.rs](enum-option.rs) for a small configuration model, or
[nix-interop.rs](nix-interop.rs) to work with existing Nix definitions.

## Running an example

From the repository root:

```bash
cargo run --locked -p rusix --example enum-option
```

Replace `enum-option` with another example's Cargo name. The command prints Nix
source. Integration tests evaluate the generated definitions and check their
behavior.

## Configuration models

These examples introduce Rust models and their conversion to Nix. Each is a
single Rust file.

| Example | Demonstrates |
| --- | --- |
| [Enum option](enum-option.rs) | Map a Rust enum to a Nix option value. |
| [Typed submodule](typed-submodule.rs) | Reuse a typed record within a configuration tree. |
| [Invalid states](invalid-states.rs) | Require TLS credentials through enum variants. |
| [Function contracts](function-contracts.rs) | Build configuration through a function with typed inputs. |
| [Exhaustive match](exhaustive-match.rs) | Derive several settings from exhaustive enum matches. |
| [Typed values](typed-values.rs) | Distinguish values with the same representation using newtypes. |
| [Layered validation](layered-validation.rs) | Separate Rust model constraints from Rusix and NixOS validation. |

The `demo` settings in the small models illustrate configuration structure.
Their Rust types are defined in the example files. These models constrain how
configuration is assembled; they do not validate hostnames or check that TLS
files exist.

Independent module contributions retain NixOS merging. Rusix checks structural
conversion within each contribution, including duplicate flattened fields.

## Nix interoperability

These examples use existing Nix definitions or refer to values resolved during
NixOS evaluation.

| Example | Demonstrates |
| --- | --- |
| [Nix interop](nix-interop.rs) | Combine a Rust model with existing Nix packages and functions. |
| [Symbolic option](symbolic-option.rs) | Make a service command follow the final NixOS option value. |
| [Package overlay](overlay/README.md) | Customize a nixpkgs package with an overlay written in Rust. |

## Package and module implementations

These examples implement existing nixpkgs definitions. Each has its own README
explaining the source layout and how to run it.

| Example | Demonstrates |
| --- | --- |
| [curl](curl-nixpkg/README.md) | Define a curl package with configurable features and TLS support. |
| [Git](git-nixpkg/README.md) | Define the Git package and its feature arguments. |
| [OpenSSL](openssl-nixpkg/README.md) | Share a package recipe across several OpenSSL releases. |
| [MariaDB](mariadb-nixpkg/README.md) | Define client and server packages across several MariaDB releases. |
| [Composed packages](composed-packages/README.md) | Connect Rust package definitions while retaining nixpkgs dependencies. |
| [PostgreSQL module](postgresql-nixos-module/README.md) | Define a NixOS module with its public option schema. |

The composed package example connects OpenSSL to curl and Git, then curl to
MariaDB. The remaining dependencies come from nixpkgs.

## Testing

Integration tests import the example code and evaluate its generated Nix. Package
and module tests also compare it with the corresponding pinned nixpkgs definitions.

See the [verification instructions](../docs/development.md#verification) for the
required tooling and commands. [Model tests](../crates/rusix/tests/typed_examples.rs)
check the configuration examples; [symbolic option tests](../crates/rusix/tests/symbolic_options.rs)
check that generated expressions follow final NixOS values. The package and module
READMEs link their compatibility tests.

## Further reading

- [Typed package authoring](../docs/typed-package-values.md) describes deferred package interfaces.
- [Developing Rusix](../docs/development.md) covers repository setup and verification.
