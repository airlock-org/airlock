use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
#[rusix(rename_all = "snake_case")]
enum Mode {
    ReadOnly,
}

fn main() {}
