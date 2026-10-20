use rusix::{
    IntoConfig,
    interop::{Nixpkgs, PackageRef},
};

#[derive(IntoConfig)]
struct Packages {
    packages: Vec<PackageRef>,
}

fn main() {
    let module = Nixpkgs::new().module("misc/label.nix");
    let _ = Packages {
        packages: vec![module],
    };
}
