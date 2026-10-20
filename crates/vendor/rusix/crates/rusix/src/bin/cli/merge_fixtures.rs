use crate::fixture_support::ExistingModule;
use rusix::{
    Config,
    nixos::{DefinitionPriority, NixosModule},
};

// Separate functions give each contribution a distinct Rust call site.
pub fn a() -> NixosModule {
    NixosModule::new(
        Config::new().set_dynamic("services.openssh.authorizedKeysCommandUser", "root"),
    )
}

pub fn b() -> NixosModule {
    NixosModule::new(
        Config::new().set_dynamic("services.openssh.authorizedKeysCommandUser", "nobody"),
    )
}

pub fn c() -> NixosModule {
    NixosModule::new(
        Config::new().set_dynamic("services.openssh.authorizedKeysCommandUser", "sshd"),
    )
}

pub fn ports_a() -> NixosModule {
    NixosModule::new(Config::new().set_dynamic("services.openssh.ports", vec![22]))
}

pub fn ports_b() -> NixosModule {
    NixosModule::new(Config::new().set_dynamic("services.openssh.ports", vec![2222]))
}

pub fn module(name: &str) -> Option<NixosModule> {
    let base = NixosModule::new(Config::new()).import(ExistingModule::OpenSsh);
    Some(match name {
        "merge-two" => base.module(a()).module(b()),
        "merge-three" => base.module(a()).module(b()).module(c()),
        "merge-ok" => base.module(ports_a()).module(ports_b()),
        "merge-priority" => base
            .module(a().priority(DefinitionPriority::Default))
            .module(b())
            .module(c().priority(DefinitionPriority::Force)),
        "merge-mixed" => {
            NixosModule::new(Config::new().set_dynamic("system.nixos.version", "24.11"))
                .import(ExistingModule::NixosLabel)
                .module(
                    NixosModule::new(
                        Config::new().set_dynamic("system.nixos.label", "rusix-label"),
                    )
                    .priority(DefinitionPriority::Default),
                )
        }
        // mergeEqualOption reports only the first conflicting pair. A separate
        // real type error exercises NixOS reporting all three invalid definitions.
        "merge-three-type" => base
            .module(NixosModule::new(
                Config::new().set_dynamic("services.openssh.ports", "invalid-a"),
            ))
            .module(NixosModule::new(
                Config::new().set_dynamic("services.openssh.ports", "invalid-b"),
            ))
            .module(NixosModule::new(
                Config::new().set_dynamic("services.openssh.ports", "invalid-c"),
            )),
        _ => return None,
    })
}

pub fn selection(name: &str) -> &'static [&'static str] {
    match name {
        "merge-mixed" => &["system", "nixos", "label"],
        "merge-ok" | "merge-three-type" => &["services", "openssh", "ports"],
        _ => &["services", "openssh", "authorizedKeysCommandUser"],
    }
}
