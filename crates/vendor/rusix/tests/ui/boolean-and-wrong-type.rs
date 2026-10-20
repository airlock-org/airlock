use rusix::Expr;

fn main() {
    let _ = Expr::boolean(true).and(Expr::int(42));
}
