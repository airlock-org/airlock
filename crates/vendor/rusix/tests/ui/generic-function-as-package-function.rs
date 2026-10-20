use rusix::interop::{raw::NixValue, Nixpkgs};

fn main() {
    let pkgs = Nixpkgs::new();
    let function = pkgs.pkgs_function("callPackage");
    pkgs.call_package(&function, NixValue::record([] as [(&str, NixValue); 0]));
}
