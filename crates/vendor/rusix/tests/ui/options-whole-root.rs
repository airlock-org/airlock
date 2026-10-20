
#[rusix::options]
mod options {
    #[rusix(root)]
    struct Root {
        port: i64,
    }
}

fn main() {
    let _ = options::root().as_value();
}
