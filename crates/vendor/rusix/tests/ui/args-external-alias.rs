
type External = String;

#[rusix::args]
mod args {
    use super::External;

    #[rusix(root)]
    struct Root {
        external: External,
    }
}

fn main() {}
