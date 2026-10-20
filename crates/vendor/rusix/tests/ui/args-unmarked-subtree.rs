
#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        subtree: Subtree,
    }

    struct Subtree {
        port: i64,
    }
}

fn main() {
    let _ = args::from_value(rusix::interop::raw::NixValue::null())
        .subtree
        .as_value();
}
