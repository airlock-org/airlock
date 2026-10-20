use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
enum Mode {
    Server,
    Client(String),
}

fn main() {}
