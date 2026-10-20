use crate::fixture_support::{ExistingModule, OpenSsh};
use rusix::{Config, Expr, IntoConfig, nixos::NixosModule};

pub fn module(name: &str) -> Option<NixosModule> {
    Some(match name {
        "good" => NixosModule::new(OpenSsh::new().enable(false).ports(vec![22]).into_config())
            .import(ExistingModule::OpenSsh),
        // Explicit low-level escape for a deliberately incompatible backend type.
        "type" => NixosModule::new(
            OpenSsh::new()
                .enable(false)
                .into_config()
                .set_dynamic("services.openssh.ports", vec!["twenty-two"]),
        )
        .import(ExistingModule::OpenSsh),
        "unknown" => NixosModule::new(
            OpenSsh::new()
                .enable(false)
                .into_config()
                .set_dynamic("services.openssh.rusixMissing", true),
        )
        .import(ExistingModule::OpenSsh),
        "assertion" => NixosModule::new(OpenSsh::new().enable(false).ports(vec![22]).into_config())
            .import(ExistingModule::OpenSsh)
            .assertion(
                "port-policy",
                Expr::boolean(false),
                "SSH port policy rejected",
            ),
        // label.nix normally relies on version.nix. Deliberately omit it.
        "external" => NixosModule::new(OpenSsh::new().enable(false).into_config())
            .import(ExistingModule::OpenSsh)
            .import(ExistingModule::NixosLabel),
        "lazy" => NixosModule::new(
            Config::new()
                .set_dynamic("services.openssh.enable", false)
                .set_dynamic(
                    "services.openssh.ports",
                    vec![Expr::int(22), Expr::int(44).divide(Expr::int(0))],
                ),
        )
        .import(ExistingModule::OpenSsh),
        _ => return None,
    })
}

pub fn selection(name: &str) -> &'static [&'static str] {
    match name {
        "external" => &["system", "nixos", "label"],
        "lazy" => &["services", "openssh", "enable"],
        _ => &["services", "openssh", "ports"],
    }
}
