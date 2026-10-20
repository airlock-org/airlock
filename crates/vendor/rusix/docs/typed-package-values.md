# Typed package authoring

Rusix package interfaces represent deferred Nix expressions. Constructing a
package handle describes a computation; Nix evaluates it when its value is needed.
The Rust interface remains available through calls, bindings and overrides.

Import `rusix::prelude::*` for the common authoring interfaces. Dynamic Nix
operations require explicit imports from `rusix::interop::raw`.

## Interfaces

| Interface | Represents |
| --- | --- |
| `Package` | An instantiated package authored in Rust or Nix |
| `PackageFunction<R>` | A factory with named arguments and result interface `R` |
| `NixCallable<R, A>` | A function with result interface `R` and parameter interface `A` |
| `NixAttrs<T>`, `NixList<T>` | Deferred collections with typed members |
| `Expr<bool>`, `Expr<String>`, `Expr<i64>` | Deferred scalar values |
| `NixNullable<T>` | A value that may evaluate to null |
| `NixPath` | A Nix path, retaining its dependency context |
| `Stdenv`, `Platform`, `FinalAttrs` | Views of builder, platform and final derivation attributes |
| `Overridable<T>` | An external interface with argument-override support |

These interfaces express expectations about external Nix values. They do not
prove that a package has a particular output or that a function implements its
expected contract. Nix checks those properties during evaluation.

## Calls and argument records

`Nixpkgs::call_package` describes nixpkgs `callPackage`: the package set supplies
arguments unless the caller overrides them. `try_call_package` accepts a Rust
record implementing `IntoRusixValue` and reports structural conversion errors.
It does not evaluate the package. The same conversion boundary is available
through `try_call` and `try_mk_derivation`.

`PackageFunction<R>` preserves its result interface. `NixCallable<R, A>` also
preserves the parameter interface; its default `A = NixValue` allows dynamic
inputs. Structured `try_call` is available on that dynamic input interface.
For an external `interop::NixFunction`, import `raw::NixFunctionExt` to attach
an expected contract with `signature`.

`#[rusix::args]` declares a finite view of a Nix argument record. Accessors describe
later Nix lookups. The argument root supports whole-record access and lazy binding;
a nested view needs `#[rusix(value)]` for those operations. `as_attrs()` exposes
the complete record, including undeclared fields, for shallow attribute-set union.
Option roots restrict whole-root access so dependencies on final NixOS values
remain explicit.

## Sharing and overrides

`NixExpression::bind` shares an expression through a lazy Nix binding. Both the
callback parameter and its result keep their Rust interfaces. For example, with
a `PackageFunction<Package>` factory and two argument sets:

```rust
let family = factory.bind(|factory| {
    NixAttrs::new([
        ("default", pkgs.call_package(&factory, default_arguments)),
        ("variant", pkgs.call_package(&factory, variant_arguments)),
    ])
});
```

The result is `NixAttrs<Package>`. The factory expression appears once, while each
package remains deferred. `choose`, `asserted` and `require` also preserve the
interface. `try_bind_record` converts a Rust record before constructing its
binding callback.

`Package` supports `NixOverridable::override_arguments` and `override_attrs`.
These call the package's real nixpkgs methods. External callables need an explicit
`Overridable<T>` interface for argument overrides; plain `NixCallable` does not
provide that capability.

See [composed packages](../examples/composed-packages/README.md) for shared
dependencies and [MariaDB](../examples/mariadb-nixpkg/README.md) for a release
family that retains ordinary `callPackage` overrides.

## Collections, text and null

Typed collection operations retain their member interfaces. `NixList::concat`
uses native Nix concatenation; `concat_with` uses the supplied library's
`concatLists`. Library operations such as `optional` and `when` honor that
library's implementations. This distinction matters when callers override helpers.

Rust `BTreeMap<K, V>` values convert to structural records when keys convert to
strings and values implement `IntoRusixValue`. Keys retain their literal spelling,
including dots and hyphens; ordered iteration keeps generated output deterministic.
Use `#[rusix(rename_all = "preserve")]` on derived records to retain Rust field
names, including underscores. Explicit field renames still take precedence.

`nix_text!` returns `Expr<String>`. Interpolation uses `ToNixText` and preserves
Nix string dependency context. Text joining uses `builtins.concatStringsSep` and
requires no nixpkgs input. Packages remain `Package` values until explicit
text coercion. Attribute sets and callables have no implicit text conversion.
`Nixpkgs::source_path` returns `NixPath`, which also retains context when coerced.
`NixPath::join_relative` appends validated relative components using native Nix
path addition, without inspecting the filesystem during Rust construction.

`NixNullable<T>` represents a deferred null-or-value choice. Its `map` and
`unwrap_or` leave excluded branches lazy. Rust `Option` instead controls
construction and, with `#[rusix(omit_none)]`, omission of record fields.

`NixAttrs::has` tests presence even when a value is null. `get_or` supplies a
fallback only for missing attributes; `get_optional` returns a nullable interface
that represents absence as null.

## Dynamic interop and custom interfaces

Use `interop::raw::NixValue` when an external interface cannot be described by
the typed handles. `raw::expect` attaches an expected interface to an expression
without evaluating or validating its Nix type. Import `raw::NixRepresentation`
for access through `as_expression` or construction through `from_expression`.
These operations retain the original expression's diagnostic provenance.

Custom symbolic interfaces implement `NixRepresentation`, `Clone` and
`IntoRusixValue`; they then receive `NixExpression` automatically. External record
views can be reused through `#[rusix(expression)]` fields. Their declared types
must implement `NixExpression`; the macro does not inspect the external schema.

Built-in handles support direct configuration placement through the sealed
`ConfigValue` trait. User-defined records use `IntoRusixValue` for nesting and
conversion at receiving boundaries. Factories can be passed directly to records
and `callPackage`.

Compiler extensions use `rusix::ir` and `rusix::compiler::ast`. These APIs and raw
representation access are excluded from the authoring prelude.

For executable examples, start with [curl](../examples/curl-nixpkg/README.md) or
[Git](../examples/git-nixpkg/README.md). The
[development guide](development.md#verification) covers verification, and
[error reporting](error-reporting.md) explains how Nix failures connect to Rust.
