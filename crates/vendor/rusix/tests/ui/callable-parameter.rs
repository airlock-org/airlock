use rusix::{Expr, interop::NixCallable};

fn main() {
    let function = NixCallable::from_function(|text: Expr<String>| text);
    let _ = function.call(Expr::int(3));
}
