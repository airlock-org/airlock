use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
enum Mode {
    Server,
    #[rusix(rename = "server")]
    Client,
}

fn main() {}
