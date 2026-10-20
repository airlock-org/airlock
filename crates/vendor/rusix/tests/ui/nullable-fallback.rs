use rusix::{Expr, interop::NixNullable};

fn main() {
    let value = NixNullable::<Expr<String>>::null();
    let _ = value.unwrap_or(Expr::int(3));
}
