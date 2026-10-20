use rusix::prelude::*;

fn main() {
    let _ = Expr::int(1).into_node(todo!());
}
