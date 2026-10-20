//! Prints the complete pinned curl factory and a Rust-authored package choice.
mod inputs;

mod lowering;

pub mod model;

use rusix::{
    IntoConfig,
    interop::{Nixpkgs, Package, PackageFunction},
};

/// Exports the reusable curl recipe and a package with the Rust model's feature choices.
#[derive(IntoConfig)]
struct Output {
    /// Reusable Nix function accepting curl's dependencies and feature arguments.
    factory: PackageFunction<Package>,
    /// Selected curl package instantiated with this example's feature choices.
    curl: Package,
}

fn main() {
    // Describe a reusable Nix function for curl, including its dependency inputs.
    // Constructing the function in Rust does not run its Nix body.
    let factory: PackageFunction<Package> = lowering::factory();

    // callPackage supplies matching dependencies from nixpkgs by parameter name.
    // The Rust model supplies feature choices for this particular curl recipe.
    let curl = Nixpkgs::new()
        .try_call_package(&factory, model::model().arguments())
        .expect("fixed authoring arguments");

    // Export the reusable function and the selected recipe as Nix attributes.
    // This prints source; evaluating or building curl happens separately.
    let artifact =
        rusix::compile(Output { factory, curl }).expect("the example has valid structural values");
    println!("{}", artifact.source);
}
