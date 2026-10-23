//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test usr assistant workspace uses existing management
/// interfaces.
pub(super) fn assistant_workspace(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|f| {
            f.clone()
                .select_segments([
                    "nixosConfigurations",
                    "airlock-domain-usr-assistant",
                    "config",
                ])
                .bind(|c| {
                    f.clone()
                        .select_segments(["lib", "bootstrapEntries"])
                        .call(NixValue::from("x86_64-linux"))
                        .select_segments(["usr-assistant", "domain"])
                        .bind(|defaults| {
                            usr_assistant_workspace_assistant_workspace_body(defaults, c.clone())
                        })
                })
        })
}

/// Build the usr assistant workspace test usr assistant workspace uses existing management
/// interfaces contribution using the caller supplied deferred values.
fn usr_assistant_workspace_assistant_workspace_body(defaults: NixValue, c: NixValue) -> NixValue {
    {
        let field_workspace = NixValue::record([
            (
                "after",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-workspace",
                    "after",
                ]),
            ),
            (
                "before",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-workspace",
                    "before",
                ]),
            ),
            (
                "requires",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-workspace",
                    "requires",
                ]),
            ),
            (
                "serviceConfig",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-workspace",
                    "serviceConfig",
                ]),
            ),
        ]);

        let field_identity = NixValue::record([
            (
                "after",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-git-identity",
                    "after",
                ]),
            ),
            (
                "before",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-git-identity",
                    "before",
                ]),
            ),
            (
                "requires",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-git-identity",
                    "requires",
                ]),
            ),
            (
                "serviceConfig",
                c.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-usr-assistant-git-identity",
                    "serviceConfig",
                ]),
            ),
        ]);

        NixValue::record([
            ("prefs", defaults.clone().select_segments(["prefs"])),
            ("features", defaults.clone().select_segments(["features"])),
            ("netvm", defaults.clone().select_segments(["netvm"])),
            (
                "session",
                c.clone()
                    .select_segments(["environment", "sessionVariables"]),
            ),
            (
                "guiEnabled",
                c.clone()
                    .select_segments(["systemd", "services", "qubes-gui-agent", "enable"]),
            ),
            (
                "readyConfig",
                c.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes/rpc-config/airlock.AppSessionReady",
                    "text",
                ]),
            ),
            (
                "instructions",
                NixValue::builtin("readFile").call(c.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/usr-assistant/AGENTS.md",
                    "source",
                ])),
            ),
            (
                "intro",
                NixValue::builtin("readFile").call(c.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/usr-assistant/home-AGENTS.md",
                    "source",
                ])),
            ),
            (
                "operations",
                NixValue::builtin("readFile").call(c.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/usr-assistant/operations.md",
                    "source",
                ])),
            ),
            (
                "repoPath",
                c.clone()
                    .select_segments(["airlock", "qubeGuest", "configRepoPath"]),
            ),
            ("workspace", field_workspace),
            ("identity", field_identity),
        ])
    }
}
