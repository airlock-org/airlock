# A normal nixpkgs overlay authored in Rust

An overlay customizes named packages while reusing the rest of nixpkgs' package
set. It is a Nix function `final: prev: { ... }`: `prev` provides the packages
before this overlay, and `final` refers to the set after all overlays. References
to `final` let packages depend on each other's final versions; Nix resolves only
the values an evaluation needs.

This example authors the same function in Rust/Rusix. It modifies ordinary
pinned nixpkgs `curl` with `prev.curl.overrideAttrs`, appending `--disable-dict`
to its existing configure flags to disable the DICT protocol. Curl's source,
dependencies and upstream package definition stay in nixpkgs. The remaining
package set stays ordinary nixpkgs.

Start with [authoring.rs](authoring.rs) to change the curl customization.
[inputs.rs](inputs.rs) declares the fields read from package sets and recipes;
[model.rs](model.rs) defines the ordinary Rust structs returned by the callbacks
and printed by [main.rs](main.rs).

```rust
let pkgs = authoring::package_set().view::<inputs::Packages>();
let curl = pkgs.curl().view::<inputs::PackageMetadata>();
let output = model::Output {
    overlay: authoring::overlay(),
    curl_derivation: curl.drv_path(),
};
```

Rust field names supply the Nix attribute names, so lookups and replacements use
accessors and struct fields. Names follow Rusix's usual lowerCamelCase mapping:
`configure_flags` becomes `configureFlags`, and `drv_path` becomes `drvPath`.
The executable compiles `output` directly and prints the overlay function
and an expression selecting the customized curl's build-recipe path. Nix applies
the overlay only when that lookup is evaluated; no package is built.

The equivalent handwritten overlay is:

```nix
final: prev: {
  curl = prev.curl.overrideAttrs (old: {
    configureFlags = old.configureFlags ++ [ "--disable-dict" ];
  });
}
```

[authoring.rs](authoring.rs) returns an `Overlay`, using
`Overlay::try_from_function(|final_pkgs: Packages, prev: Packages| ...)`.
The `Packages` view exposes `curl()` without reading the Nix package set in Rust.
The callback returns `Changes { curl }`; that struct names the package to replace.
Curl's `try_override_attrs` callback similarly receives a `BuildAttrs` view and
returns `ConfigureChanges`, preserving the previous configure flags before
appending the new flag. Packages, helpers and nested collections can all be
fields in an overlay's result struct.

Rust runs each callback once during construction and checks conversion of its
returned struct. Nix resolves the symbolic lookups and applies the generated
functions later. A declared view describes the fields this overlay needs; it
keeps the complete underlying package set, including undeclared packages. Nix
still checks whether selected fields exist and whether their values have the
expected types. Rust compilation catches misspelled Rust accessors and fields.

`Nixpkgs::new().with_overlay(overlay())` retains the overlay for application by
nixpkgs. Package lookups use that customized set, and further `with_overlay`
calls apply overlays in order. The method also accepts an `OverlayRef` from a
local Nix file. `Overlay` supports direct placement in `Config`, as shown above,
and can be passed in a Nix import's `overlays` list.

- Rusix package examples replace package definitions written in Nixlang.
- This overlay example replaces overlay/customization code written in Nixlang.

From the repository root:

```bash
cargo run --locked -p rusix --example overlay
cargo test --locked -p rusix --test overlay
```

The executable prints generated Nix without invoking Nix. Tests evaluate the
unchanged pin `8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296` for `x86_64-linux`,
offline in disposable stores; they never build packages. They compare complete
curl and downstream `curlpp` recipes and identities against the handwritten
overlay, check the exact configure-flags change, preserve `hello`'s identity,
and verify ordered `prev`, fixed-point `final`, and unused failing attributes.
A controlled error inside `overrideAttrs` maps to its Rust division operation;
upstream package internals retain their original Nix diagnostic frames.
Reviewable generated code, source maps, results and the failing Nix diagnostic
are saved under `target/overlay-equivalence/`.
