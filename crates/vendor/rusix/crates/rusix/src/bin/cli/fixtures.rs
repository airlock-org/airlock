use rusix::{Config, Expr};

pub fn config(name: &str) -> Option<Config> {
    Some(match name {
        "good" => Config::new()
            .set_dynamic("services.openssh.enable", true)
            .set_dynamic("services.openssh.ports", vec![22])
            .set_dynamic("logging.level", "verbose"),
        "bad-port" => Config::new().set_dynamic(
            "services.openssh.ports",
            vec![Expr::int(70000).in_range(1, 65535, "SSH port must be in 1..=65535")],
        ),
        "nested" => Config::new().set_dynamic(
            "services.openssh.ports",
            vec![Expr::int(22), Expr::int(44).divide(Expr::int(0))],
        ),
        "conflict" => Config::new()
            .set_dynamic("service", true)
            .set_dynamic("service.enable", true),
        "selective" => Config::new()
            .set_dynamic("good", Expr::int(42))
            .set_dynamic("bad", Expr::int(44).divide(Expr::int(0))),
        _ => return None,
    })
}
