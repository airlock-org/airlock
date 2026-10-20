use rusix::prelude::*;

fn main() {
    let package: Package = Nixpkgs::new().get("openssl").into();
    let _ = package.as_expression();
}
