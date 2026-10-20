//! Authors the pinned Git package in Rust and prints its deferred build expression.
mod inputs;

mod lowering;

pub mod model;

pub use inputs::Arguments;

use rusix::interop::{Nixpkgs, Package as PackageValue, PackageFunction};

/// Defines the Nix output containing Git's reusable recipe function and selected package.
/// Each root field becomes a named attribute that a Nix caller can select.
#[rusix::config]
mod output {
    use super::{PackageFunction, PackageValue};

    /// Exports both the reusable native Nix factory and this example's chosen package.
    #[rusix(root)]
    pub(super) struct Package {
        /// Ordinary Nix callers may use callPackage to supply their own package scope.
        pub(super) factory: PackageFunction<PackageValue>,
        /// A real mkDerivation result; printing compiler output does not build Git.
        pub(super) git: PackageValue,
    }
}

fn main() {
    // Describe a reusable Nix function that takes Git's dependencies and features.
    // Rust constructs its body now; Nix applies the function when needed later.
    let factory: PackageFunction<PackageValue> = lowering::factory();

    // callPackage supplies matching dependencies from nixpkgs by parameter name.
    // Our Rust model supplies explicit choices; the result describes a Git build.
    let git = Nixpkgs::new()
        .try_call_package(&factory, inputs::arguments(model::model()))
        .expect("fixed authoring arguments");

    // Export both the function and the chosen package as Nix attributes.
    // Generating this source does not evaluate the recipe or build Git.
    let artifact = rusix::compile(output::Package { factory, git }).unwrap();
    println!("{}", artifact.source);
}
