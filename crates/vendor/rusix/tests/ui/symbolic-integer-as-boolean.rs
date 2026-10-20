use rusix::nixos::{NixosModule, OptionRef};

fn main() {
    let port = OptionRef::<i64>::new("services.example.port");
    NixosModule::empty().assertion("enabled", port.into_expr(), "must be enabled");
}
