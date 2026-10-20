
#[rusix::config]
mod config {
    #[rusix(root, root)]
    struct Machine {
        enable: bool,
    }
}

fn main() {}
