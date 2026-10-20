use rusix::{Config, interop::Nixpkgs, nixos::NixosModule};

fn main() {
    let module = Nixpkgs::new().module("misc/label.nix");
    NixosModule::new(Config::new()).system_packages(vec![module]);
}
