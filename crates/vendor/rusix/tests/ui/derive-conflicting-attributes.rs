use rusix::IntoConfig;

#[derive(IntoConfig)]
struct Example {
    #[rusix(skip, rename = "enable")]
    enabled: bool,
}

fn main() {}
