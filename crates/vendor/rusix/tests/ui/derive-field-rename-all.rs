use rusix::IntoConfig;

#[derive(IntoConfig)]
struct Config {
    #[rusix(rename_all = "PascalCase")]
    listen_port: u16,
}

fn main() {}
