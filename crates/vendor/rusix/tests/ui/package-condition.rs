use rusix::{Expr, interop::{Nixpkgs, Package}};

fn dependency(_: Package) {}

fn main() {
    let _ = Nixpkgs::new();
    dependency(Expr::boolean(true));
}
