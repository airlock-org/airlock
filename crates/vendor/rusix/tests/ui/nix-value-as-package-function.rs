use rusix::interop::{raw::NixValue, Nixpkgs};

fn main() {
    let pkgs = Nixpkgs::new();
    let function = NixValue::function_attrs(["lib"], |args| (Vec::<(&str, NixValue)>::new(), args));
    pkgs.call_package(&function, NixValue::record([] as [(&str, NixValue); 0]));
}
