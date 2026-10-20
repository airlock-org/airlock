use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
#[rusix(rename_all = "PascalCase")]
struct Port(u16);

fn main() {}
