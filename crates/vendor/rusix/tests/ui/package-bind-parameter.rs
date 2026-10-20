use rusix::{Expr, interop::{NixExpression, Nixpkgs, Package}};

fn main() {
    let package: Package = Nixpkgs::new().get("openssl").into();
    let _ = package.bind(|value: Expr<bool>| value);
}
