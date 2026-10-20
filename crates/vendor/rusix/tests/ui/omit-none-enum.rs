use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
#[rusix(omit_none)]
enum Mode {
    Server,
}

fn main() {}
