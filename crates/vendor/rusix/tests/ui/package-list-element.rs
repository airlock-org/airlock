use rusix::{Expr, interop::{NixList, Package}};

fn main() {
    let _: NixList<Package> = NixList::new([Expr::boolean(true)]);
}
