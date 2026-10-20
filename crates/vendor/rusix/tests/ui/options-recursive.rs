
#[rusix::options]
mod options {
    #[rusix(root)]
    struct Root {
        nested: Nested,
    }

    struct Nested {
        next: Nested,
    }
}

fn main() {}
