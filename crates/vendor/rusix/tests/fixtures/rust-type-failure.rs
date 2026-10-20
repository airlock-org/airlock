use rusix::Expr;

fn main() {
    let _ = Expr::int(22).divide(Expr::boolean(true));
}
