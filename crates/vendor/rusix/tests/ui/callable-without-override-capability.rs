use rusix::{Expr, interop::{NixCallable, NixOverridable}};

fn expects_overrides<T: NixOverridable>(_: T) {}

fn main() {
    let callable = NixCallable::from_function(|value: Expr<String>| value);
    expects_overrides(callable);
}
