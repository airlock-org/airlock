use rusix::prelude::*;

fn main() {
    let _ = Nixpkgs::new().value("openssl");
}
