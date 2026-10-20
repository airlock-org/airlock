//! Print Nix for the package set assembled by composition::packages.
pub mod composition;

use rusix::{
    IntoConfig,
    interop::{NixAttrs, Package},
};

/// Exports packages whose selected dependencies are wired together in Rust.
#[derive(IntoConfig)]
struct Output {
    /// Connected package set: OpenSSL feeds curl and Git; curl feeds MariaDB.
    /// Each package remains a deferred recipe that Nix evaluates when demanded.
    packages: NixAttrs<Package>,
}

fn main() {
    // Connect package recipes: our OpenSSL feeds curl and Git, and curl feeds MariaDB.
    // Rust records these dependencies; Nix resolves their recipes when needed.
    let generated = rusix::compile(Output {
        packages: composition::packages(),
    })
    .unwrap();

    // The output is Nix source for the connected package set, not built packages.
    println!("{}", generated.source);
}
