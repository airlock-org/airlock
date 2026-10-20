use rusix::{Expr, interop::{NixCallable, raw::NixValue}};

fn main() {
    let function = NixCallable::from_function(|text: Expr<String>| text);
    let _ = function.try_call(NixValue::from(3_i64));
}
