//! Emit the complete family and the default package without building or fetching.
mod inputs;

mod lowering;

mod model;

mod scripts;

use rusix::{
    IntoConfig,
    interop::{NixAttrs, Nixpkgs, Package, PackageFunction},
    nix_record,
};

/// Exports a reusable release family and one selected package as named Nix attributes.
#[derive(IntoConfig)]
struct Output {
    /// Nix function returning all three releases with the caller's dependencies.
    family: PackageFunction<NixAttrs<Package>>,
    /// Selected OpenSSL package instantiated through nixpkgs' callPackage.
    openssl: Package,
}

fn main() {
    // Describe the Nix recipe function for our chosen OpenSSL release.
    // Rust constructs a function expression; Nix will apply it later.
    let factory = lowering::factory(model::Release::Preview);

    // With no explicit arguments, callPackage takes matching dependencies from
    // nixpkgs and leaves the function's own defaults in effect for other inputs.
    let openssl = Nixpkgs::new().call_package(&factory, nix_record! {});

    // Export a reusable function for the release family alongside our selection.
    // Generating this Nix source neither fetches sources nor builds OpenSSL.
    let generated = rusix::compile(Output {
        family: lowering::family_factory(),
        openssl,
    })
    .unwrap();
    println!("{}", generated.source);
}
