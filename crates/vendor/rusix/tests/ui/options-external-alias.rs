
type External = String;

#[rusix::options]
mod options {
    use super::External;

    #[rusix(root)]
    struct Root {
        external: External,
    }
}

fn main() {}
