use rusix::IntoConfig;

#[derive(IntoConfig)]
#[rusix(prefix = "services.example")]
struct Example {
    enable: bool,
}

fn main() {}
