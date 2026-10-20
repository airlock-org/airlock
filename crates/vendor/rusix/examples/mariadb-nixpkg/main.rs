//! Emit MariaDB's shared factory, family and default server/client result.
mod inputs;

mod lowering;

pub mod model;

mod scripts;

use rusix::{
    IntoConfig,
    interop::{NixAttrs, Nixpkgs, Package, PackageFunction},
};

/// Exports the reusable recipe, release family and selected MariaDB package.
#[derive(IntoConfig)]
struct Output {
    /// Reusable Nix function accepting a release, dependencies and feature arguments.
    factory: PackageFunction<Package>,
    /// All four pinned release packages, each exposing client and server members.
    family: NixAttrs<Package>,
    /// Selected MariaDB 10.11 package exposing its client and server members.
    mariadb: Package,
}

fn main() {
    // Describe one Nix function shared by the supported MariaDB releases.
    // Each application supplies the release, dependencies and feature choices.
    let factory = lowering::factory();

    // Select release 10.11 and let callPackage supply matching nixpkgs dependencies.
    // The result is a deferred recipe with both client and server members.
    let mariadb = Nixpkgs::new()
        .try_call_package(&factory, model::Release::V1011.arguments())
        .expect("fixed authoring arguments");

    // Export the function, release family and selected package as Nix attributes.
    // These are build descriptions; printing them does not build MariaDB.
    let generated = rusix::compile(Output {
        factory,
        family: lowering::family(),
        mariadb,
    })
    .unwrap();
    println!("{}", generated.source);
}
