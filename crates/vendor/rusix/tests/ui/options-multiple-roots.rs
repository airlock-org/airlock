
#[rusix::options]
mod options {
    #[rusix(root)]
    struct First {
        port: i64,
    }

    #[rusix(root)]
    struct Second {
        enable: bool,
    }
}

fn main() {}
