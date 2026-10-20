
#[rusix::config]
mod config {
    #[rusix(root, omit_none, omit_none)]
    struct Root {
        value: Option<String>,
    }
}

fn main() {}
