use rusix::IntoConfig;

#[derive(IntoConfig)]
struct Root {
    #[rusix(omit_none)]
    value: i64,
}

fn main() {}
