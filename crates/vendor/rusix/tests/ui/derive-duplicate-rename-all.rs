use rusix::IntoRusixValue;

#[derive(IntoRusixValue)]
#[rusix(rename_all = "PascalCase", rename_all = "lowerCamelCase")]
struct Settings {
    exec_start: String,
}

fn main() {}
