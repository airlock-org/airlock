use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
enum Mode {
    #[rusix(flatten)]
    Server,
}

fn main() {}
