use rusix::{ Expr};

#[rusix::options]
mod options {
    #[rusix(root)]
    struct Root {
        port: i64,
    }
}

fn needs_boolean(_: Expr<bool>) {}

fn main() {
    needs_boolean(options::root().port());
}
