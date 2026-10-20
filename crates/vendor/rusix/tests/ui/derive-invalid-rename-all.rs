use rusix::IntoConfig;

#[derive(IntoConfig)]
#[rusix(rename_all = "kebab-case")]
struct Config {
    listen_port: u16,
}

fn main() {}
