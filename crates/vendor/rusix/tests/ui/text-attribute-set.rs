use rusix::{Expr, interop::NixAttrs, nix_text};

fn main() {
    let attrs = NixAttrs::new([("text", Expr::<String>::from("value"))]);
    let _ = nix_text!("{attrs}", attrs = attrs);
}
