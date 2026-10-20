use rusix::prelude::*;

fn main() {
    let _ = Nixpkgs::new().get("openssl").reference();
}
