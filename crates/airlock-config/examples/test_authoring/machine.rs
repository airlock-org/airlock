//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Describe the machine evaluation boundary using caller-supplied source trees.
pub(super) fn expression() -> NixValue {
    NixValue::function_attrs(
        [
            "nixpkgsSource",
            "qubesSource",
            "libvirtSource",
            "machine",
            "apps",
            "generated",
            "variant",
            "conflict",
            "desktopConflict",
            "scale",
            "captureTarget",
            "desktopSession",
            "gnomeEnabled",
            "captureSource",
            "captureDestination",
            "senderContract",
            "receiverContract",
            "projection",
            "wholeMachine",
            "revision",
        ],
        machine_expression_13,
    )
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_body(cfg: NixValue, lib: NixValue) -> NixValue {
    {
        let receiver = lib
            .clone()
            .select_segments(["mapAttrs"])
            .call(NixValue::function(|_| {
                NixValue::function(|entry| {
                    field(["mode"], entry.clone().select_segments(["mode"])).merge_attrs(
                        NixValue::if_else(
                            !entry
                                .clone()
                                .select_segments(["text"])
                                .equals(NixValue::null()),
                            field(["text"], entry.clone().select_segments(["text"])),
                            NixValue::record([
                                (
                                    "source",
                                    NixValue::builtin("toString")
                                        .call(entry.clone().select_segments(["source"])),
                                ),
                                (
                                    "script",
                                    entry
                                        .clone()
                                        .attr_or(
                                            "source",
                                            NixValue::record(Vec::<(&str, NixValue)>::new()),
                                        )
                                        .attr_or("text", NixValue::null()),
                                ),
                            ]),
                        ),
                    )
                })
            }));

        let argument_1 = lib
            .clone()
            .select_segments(["filterAttrs"])
            .call(NixValue::function(|name| {
                NixValue::function(|_| {
                    NixValue::builtin("elem")
                        .call(name.clone())
                        .call(NixValue::list([
                            NixValue::from("airlock/capture-sender.json"),
                            NixValue::from("airlock/capture-receiver.json"),
                            NixValue::from("xdg/autostart/airlock-capture-retry.desktop"),
                            NixValue::from("qubes-rpc/airlock.CaptureImport"),
                            NixValue::from("qubes-rpc/airlock.CaptureOpen"),
                            NixValue::from("qubes/rpc-config/airlock.CaptureImport"),
                            NixValue::from("qubes/rpc-config/airlock.CaptureOpen"),
                        ]))
                })
            }))
            .call(cfg.clone().select_segments(["environment", "etc"]));

        receiver.call(argument_1)
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_2(cfg: NixValue, entry: NixValue) -> NixValue {
    {
        let field_scale = cfg
            .clone()
            .attr_or("airlock", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or("appGui", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or("scale", NixValue::null());

        let field_packages = NixValue::builtin("map")
            .call(NixValue::function(|package| {
                NixValue::record([
                    ("name", package.clone().select_segments(["name"])),
                    ("out", NixValue::builtin("toString").call(package.clone())),
                ])
            }))
            .call(
                cfg.clone()
                    .select_segments(["environment", "systemPackages"]),
            );

        let field_system = NixValue::record([
            (
                "drv",
                entry.clone().select_segments([
                    "configuration",
                    "config",
                    "system",
                    "build",
                    "toplevel",
                    "drvPath",
                ]),
            ),
            (
                "out",
                NixValue::builtin("toString").call(entry.clone().select_segments([
                    "configuration",
                    "config",
                    "system",
                    "build",
                    "toplevel",
                ])),
            ),
        ]);

        let field_store = NixValue::record([
            (
                "drv",
                entry.clone().select_segments(["storeArtifact", "drvPath"]),
            ),
            (
                "out",
                NixValue::builtin("toString")
                    .call(entry.clone().select_segments(["storeArtifact"])),
            ),
        ]);

        NixValue::record([
            (
                "hostName",
                cfg.clone().select_segments(["networking", "hostName"]),
            ),
            ("scale", field_scale),
            ("packages", field_packages),
            ("system", field_system),
            (
                "root",
                NixValue::record([
                    (
                        "drv",
                        entry.clone().select_segments(["rootImage", "drvPath"]),
                    ),
                    (
                        "out",
                        NixValue::builtin("toString")
                            .call(entry.clone().select_segments(["rootImage"])),
                    ),
                ]),
            ),
            ("store", field_store),
        ])
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_3(descriptor: NixValue, cfg: NixValue) -> NixValue {
    {
        let field_display_mode = cfg
            .clone()
            .attr_or("airlock", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or(
                "sysGuiGuest",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("displayMode", NixValue::null());

        let field_physical_desktop_on_boot = cfg
            .clone()
            .attr_or("airlock", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or(
                "sysGuiGuest",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("physicalDesktopOnBoot", NixValue::null());

        NixValue::record([
            ("label", descriptor.clone().select_segments(["label"])),
            ("netvm", descriptor.clone().select_segments(["netvm"])),
            ("prefs", descriptor.clone().select_segments(["prefs"])),
            ("features", descriptor.clone().select_segments(["features"])),
            ("services", descriptor.clone().select_segments(["services"])),
            ("devices", descriptor.clone().select_segments(["devices"])),
            (
                "state",
                descriptor.clone().select_segments(["backend", "state"]),
            ),
            (
                "hostName",
                cfg.clone().select_segments(["networking", "hostName"]),
            ),
            ("displayMode", field_display_mode),
            ("physicalDesktopOnBoot", field_physical_desktop_on_boot),
            (
                "password",
                cfg.clone()
                    .select_segments(["users", "users", "user", "initialPassword"]),
            ),
            (
                "mutableUsers",
                cfg.clone().select_segments(["users", "mutableUsers"]),
            ),
            (
                "kernelModules",
                cfg.clone().select_segments(["boot", "kernelModules"]),
            ),
            (
                "videoDrivers",
                cfg.clone()
                    .select_segments(["services", "xserver", "videoDrivers"]),
            ),
        ])
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_4(
    failures: NixValue,
    etc: NixValue,
    gui: NixValue,
    home: NixValue,
    lib: NixValue,
) -> NixValue {
    {
        let argument_2 = {
            let field_desktop = NixValue::record([
                (
                    "hasDesktopOptions",
                    gui.clone()
                        .select_segments(["options", "airlock"])
                        .has_attr("sysGuiDesktop"),
                ),
                (
                    "session",
                    gui.clone().select_segments([
                        "config",
                        "services",
                        "displayManager",
                        "defaultSession",
                    ]),
                ),
                (
                    "defaultSession",
                    gui.clone().select_segments([
                        "config",
                        "services",
                        "displayManager",
                        "defaultSession",
                    ]),
                ),
                (
                    "xfce",
                    gui.clone()
                        .select_segments(["config", "airlock", "sysGuiGuest", "enableXfce"]),
                ),
                (
                    "gnome",
                    gui.clone().select_segments([
                        "config",
                        "services",
                        "desktopManager",
                        "gnome",
                        "enable",
                    ]),
                ),
            ]);

            let field_gnome = NixValue::record([
                (
                    "drv",
                    gui.clone()
                        .select_segments(["pkgs", "gnome-shell", "drvPath"]),
                ),
                (
                    "patches",
                    NixValue::builtin("map")
                        .call(NixValue::builtin("toString"))
                        .call(
                            gui.clone()
                                .select_segments(["pkgs", "gnome-shell", "patches"]),
                        ),
                ),
                (
                    "postPatch",
                    gui.clone()
                        .select_segments(["pkgs", "gnome-shell", "postPatch"]),
                ),
            ]);

            let field_packages = NixValue::record([
                (
                    "sender",
                    NixValue::builtin("map")
                        .call(NixValue::builtin("toString"))
                        .call(gui.clone().select_segments([
                            "config",
                            "environment",
                            "systemPackages",
                        ])),
                ),
                (
                    "receiver",
                    NixValue::builtin("map")
                        .call(NixValue::builtin("toString"))
                        .call(home.clone().select_segments([
                            "config",
                            "environment",
                            "systemPackages",
                        ])),
                ),
            ]);

            NixValue::record([
                (
                    "sender",
                    etc.clone().call(gui.clone().select_segments(["config"])),
                ),
                (
                    "receiver",
                    etc.clone().call(home.clone().select_segments(["config"])),
                ),
                (
                    "hasSenderOptions",
                    gui.clone()
                        .select_segments(["options", "airlock"])
                        .has_attr("captureTransfer"),
                ),
                (
                    "hasReceiverOptions",
                    home.clone()
                        .select_segments(["options", "airlock"])
                        .has_attr("captureTransfer"),
                ),
                ("desktop", field_desktop),
                ("gnome", field_gnome),
                ("packages", field_packages),
            ])
        };

        NixValue::assert(
            lib.clone()
                .select_segments(["assertMsg"])
                .call(failures.clone().equals(NixValue::list([])))
                .call(
                    lib.clone()
                        .select_segments(["concatMapStringsSep"])
                        .call(NixValue::from(
                            r#"
"#,
                        ))
                        .call(NixValue::function(|a| {
                            a.clone().select_segments(["message"])
                        }))
                        .call(failures.clone()),
                ),
            argument_2,
        )
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_5(
    gui: NixValue,
    arguments_1: NixValue,
    bindings_2: NixValue,
    lib: NixValue,
) -> NixValue {
    NixValue::builtin("getAttr")
        .call(arguments_1.clone().select("captureDestination").clone())
        .call(bindings_2.clone().select("entries").clone())
        .select_segments(["configuration"])
        .bind(|home| {
            NixValue::function(|cfg| machine_expression_body(cfg, lib.clone())).bind(|etc| {
                NixValue::builtin("filter")
                    .call(NixValue::function(|a| {
                        !a.clone().select_segments(["assertion"])
                    }))
                    .call(NixValue::concat_lists([
                        gui.clone().select_segments(["config", "assertions"]),
                        home.clone().select_segments(["config", "assertions"]),
                    ]))
                    .bind(|failures| {
                        machine_expression_4(
                            failures,
                            etc.clone(),
                            gui.clone(),
                            home.clone(),
                            lib.clone(),
                        )
                    })
            })
        })
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_6(bindings_2: NixValue, captures: Bindings) -> NixValue {
    let apps_source = captures.get("apps_source");
    let arguments = captures.get("arguments");
    let arguments_1 = captures.get("arguments_1");
    let lib = captures.get("lib");
    let materialize = captures.get("materialize");
    let nixpkgs = captures.get("nixpkgs");
    let original = captures.get("original");

    {
        let field_entries = {
            let when_true = host::expression().call(NixValue::record([
                ("lib", lib.clone()),
                ("nixpkgs", nixpkgs.clone()),
                (
                    "nixpkgsSource",
                    arguments_1.clone().select("nixpkgsSource").clone(),
                ),
                (
                    "qubesSource",
                    arguments_1.clone().select("qubesSource").clone(),
                ),
                (
                    "libvirtSource",
                    arguments_1.clone().select("libvirtSource").clone(),
                ),
                ("machine", arguments_1.clone().select("machine").clone()),
                ("generated", arguments_1.clone().select("generated").clone()),
                ("variant", arguments_1.clone().select("variant").clone()),
                ("revision", arguments_1.clone().select("revision").clone()),
                ("entries", bindings_2.clone().select("entries").clone()),
                ("apps", apps_source.clone()),
                ("vmEntriesOnly", NixValue::from(true)),
            ]));

            let when_false = {
                let when_true = NixValue::record([
                    (
                        "sys-net",
                        original
                            .clone()
                            .select_segments(["sys-net"])
                            .call(NixValue::from("x86_64-linux")),
                    ),
                    (
                        "sys-firewall",
                        original
                            .clone()
                            .select_segments(["sys-firewall"])
                            .call(NixValue::from("x86_64-linux")),
                    ),
                    (
                        "usr-home",
                        original
                            .clone()
                            .select_segments(["usr-home"])
                            .call(NixValue::from("x86_64-linux")),
                    ),
                ])
                .merge_attrs(
                    lib.clone()
                        .select_segments(["optionalAttrs"])
                        .call(arguments_1.clone().select("wholeMachine").clone())
                        .call(NixValue::record([
                            (
                                "sys-gui",
                                original
                                    .clone()
                                    .select_segments(["sys-gui"])
                                    .call(NixValue::from("x86_64-linux")),
                            ),
                            (
                                "sys-ctrl",
                                original
                                    .clone()
                                    .select_segments(["sys-ctrl"])
                                    .call(NixValue::from("x86_64-linux")),
                            ),
                            (
                                "usr-assistant",
                                original
                                    .clone()
                                    .select_segments(["usr-assistant"])
                                    .call(NixValue::from("x86_64-linux")),
                            ),
                            (
                                "usr-cmd",
                                original
                                    .clone()
                                    .select_segments(["usr-cmd"])
                                    .call(NixValue::from("x86_64-linux")),
                            ),
                        ])),
                );

                let when_false = lib
                    .clone()
                    .select_segments(["mapAttrs"])
                    .call(NixValue::function(|_| {
                        NixValue::function(|factory| {
                            factory.clone().call(NixValue::from("x86_64-linux"))
                        })
                    }))
                    .call(
                        NixValue::builtin("import")
                            .call(add(
                                arguments_1.clone().select("generated").clone(),
                                NixValue::concat_text([
                                    NixValue::from("/"),
                                    arguments_1.clone().select("variant").clone().to_text(),
                                    NixValue::from("/default.nix"),
                                ]),
                            ))
                            .select_segments(["builders"])
                            .call(arguments.clone().merge_attrs(NixValue::record([
                                (
                                    "airlockSupport",
                                    add(
                                        source_path("/"),
                                        arguments_1.clone().select("machine").clone(),
                                    ),
                                ),
                                ("airlockApps", apps_source.clone()),
                                (
                                    "modulesDirectory",
                                    add(
                                        add(
                                            source_path("/"),
                                            arguments_1.clone().select("generated").clone(),
                                        ),
                                        NixValue::concat_text([
                                            NixValue::from("/"),
                                            arguments_1.clone().select("variant").clone().to_text(),
                                        ]),
                                    ),
                                ),
                            ]))),
                    );

                NixValue::if_else(
                    arguments_1
                        .clone()
                        .select("variant")
                        .clone()
                        .equals(NixValue::from("original")),
                    when_true,
                    when_false,
                )
            };

            NixValue::if_else(
                arguments_1
                    .clone()
                    .select("variant")
                    .clone()
                    .equals(NixValue::from("usb-lab-reference")),
                when_true,
                when_false,
            )
        };

        let field_topology = NixValue::builtin("import")
            .call(add(
                arguments_1.clone().select("machine").clone(),
                NixValue::from("/generated/schemas.nix"),
            ))
            .select_segments(["topology"])
            .call(lib.clone())
            .select_segments(["mkTopology"])
            .call(field(
                ["domains"],
                lib.clone()
                    .select_segments(["mapAttrs"])
                    .call(NixValue::function(|_| materialize.clone()))
                    .call(bindings_2.clone().select("entries").clone()),
            ));

        NixValue::record([("entries", field_entries), ("topology", field_topology)])
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_7(
    args: NixValue,
    arguments_1: NixValue,
    lib: NixValue,
    real: NixValue,
) -> NixValue {
    let option = |name| arguments_1.clone().select(name);
    let force = |value: NixValue| lib.clone().select("mkForce").call(value);
    let optional = |condition: NixValue, value: NixValue| {
        lib.clone().select("optional").call(condition).call(value)
    };
    let named = |name: &str| args.clone().select("name").equals(name);
    let supplied = |name| !option(name).equals(NixValue::null());
    let mut modules = vec![args.clone().select("modules")];

    modules.push(optional(
        supplied("scale"),
        field(["airlock", "appGui", "scale"], option("scale")),
    ));
    modules.push(optional(
        supplied("captureTarget").and(named("sys-gui")),
        field(
            ["airlock", "captureTransfer", "senderConfig", "target"],
            force(option("captureTarget")),
        ),
    ));

    for (setting, target, filename) in [
        (
            "senderContract",
            "captureSource",
            "airlock/capture-sender.json",
        ),
        (
            "receiverContract",
            "captureDestination",
            "airlock/capture-receiver.json",
        ),
    ] {
        let selected = args.clone().select("name").equals(option(target));
        modules.push(optional(
            supplied(setting).and(selected),
            field(
                ["environment", "etc", filename, "text"],
                force(option(setting)),
            ),
        ));
    }

    modules.push(optional(
        supplied("desktopSession").and(named("sys-gui")),
        field(
            ["services", "displayManager", "defaultSession"],
            lib.clone()
                .select("mkOverride")
                .call(40_i64)
                .call(option("desktopSession")),
        ),
    ));
    modules.push(optional(
        supplied("gnomeEnabled").and(named("sys-gui")),
        field(
            ["services", "desktopManager", "gnome", "enable"],
            force(option("gnomeEnabled")),
        ),
    ));
    modules.push(optional(
        option("desktopConflict").and(named("sys-gui")),
        field(
            ["services", "displayManager", "defaultSession"],
            force("conflicting-session".into()),
        ),
    ));
    modules.push(optional(
        option("conflict").and(named("usr-assistant")),
        field(
            ["environment", "sessionVariables", "AIRLOCK_WORKSPACE"],
            "conflicting definition",
        ),
    ));

    let contributions = field(["modules"], NixValue::concat_lists(modules));
    real.select("mkNixNativeDomain")
        .call(args.merge_attrs(contributions))
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_8(
    system: NixValue,
    apps_source: NixValue,
    arguments_1: NixValue,
    lib: NixValue,
    nixpkgs: NixValue,
) -> NixValue {
    NixValue::builtin("import")
        .call(add(
            apps_source.clone(),
            NixValue::from("/airlock-config/generated/vm-builders.nix"),
        ))
        .select_segments(["builders"])
        .call(NixValue::record([
            ("nixpkgs", nixpkgs.clone()),
            ("system", system.clone()),
            (
                "nixpkgsLibvirt105Source",
                arguments_1.clone().select("libvirtSource").clone(),
            ),
            (
                "nixosQubesSource",
                arguments_1.clone().select("qubesSource").clone(),
            ),
            (
                "airlockSupport",
                arguments_1.clone().select("machine").clone(),
            ),
            ("airlockAppsSource", apps_source.clone()),
        ]))
        .bind(|real| {
            real.clone().merge_attrs(field(
                ["mkNixNativeDomain"],
                NixValue::function(|args| {
                    machine_expression_7(args, arguments_1.clone(), lib.clone(), real.clone())
                }),
            ))
        })
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_9(
    bindings_2: NixValue,
    apps_source: NixValue,
    arguments_1: NixValue,
    inspect: NixValue,
    lib: NixValue,
    materialize: NixValue,
    nixpkgs: NixValue,
) -> NixValue {
    {
        let when_true = host::expression().call(NixValue::record([
            ("lib", lib.clone()),
            ("nixpkgs", nixpkgs.clone()),
            (
                "nixpkgsSource",
                arguments_1.clone().select("nixpkgsSource").clone(),
            ),
            (
                "qubesSource",
                arguments_1.clone().select("qubesSource").clone(),
            ),
            (
                "libvirtSource",
                arguments_1.clone().select("libvirtSource").clone(),
            ),
            ("machine", arguments_1.clone().select("machine").clone()),
            ("generated", arguments_1.clone().select("generated").clone()),
            ("variant", arguments_1.clone().select("variant").clone()),
            ("revision", arguments_1.clone().select("revision").clone()),
            ("entries", bindings_2.clone().select("entries").clone()),
            ("apps", apps_source.clone()),
            ("physicalOnly", NixValue::from(true)),
        ]));

        let when_false = {
            let when_true = host::expression().call(NixValue::record([
                ("lib", lib.clone()),
                ("nixpkgs", nixpkgs.clone()),
                (
                    "nixpkgsSource",
                    arguments_1.clone().select("nixpkgsSource").clone(),
                ),
                (
                    "qubesSource",
                    arguments_1.clone().select("qubesSource").clone(),
                ),
                (
                    "libvirtSource",
                    arguments_1.clone().select("libvirtSource").clone(),
                ),
                ("machine", arguments_1.clone().select("machine").clone()),
                ("generated", arguments_1.clone().select("generated").clone()),
                ("variant", arguments_1.clone().select("variant").clone()),
                ("revision", arguments_1.clone().select("revision").clone()),
                ("entries", bindings_2.clone().select("entries").clone()),
                ("apps", apps_source.clone()),
                ("desktopDetails", NixValue::from(true)),
            ]));

            let when_false = {
                let when_true = host::expression().call(NixValue::record([
                    ("lib", lib.clone()),
                    ("nixpkgs", nixpkgs.clone()),
                    (
                        "nixpkgsSource",
                        arguments_1.clone().select("nixpkgsSource").clone(),
                    ),
                    (
                        "qubesSource",
                        arguments_1.clone().select("qubesSource").clone(),
                    ),
                    (
                        "libvirtSource",
                        arguments_1.clone().select("libvirtSource").clone(),
                    ),
                    ("machine", arguments_1.clone().select("machine").clone()),
                    ("generated", arguments_1.clone().select("generated").clone()),
                    ("variant", arguments_1.clone().select("variant").clone()),
                    ("revision", arguments_1.clone().select("revision").clone()),
                    ("entries", bindings_2.clone().select("entries").clone()),
                    ("apps", apps_source.clone()),
                    ("desktopOnly", NixValue::from(true)),
                ]));

                let when_false = {
                    let when_true = host::expression().call(NixValue::record([
                        ("lib", lib.clone()),
                        ("nixpkgs", nixpkgs.clone()),
                        (
                            "nixpkgsSource",
                            arguments_1.clone().select("nixpkgsSource").clone(),
                        ),
                        (
                            "qubesSource",
                            arguments_1.clone().select("qubesSource").clone(),
                        ),
                        (
                            "libvirtSource",
                            arguments_1.clone().select("libvirtSource").clone(),
                        ),
                        ("machine", arguments_1.clone().select("machine").clone()),
                        ("generated", arguments_1.clone().select("generated").clone()),
                        ("variant", arguments_1.clone().select("variant").clone()),
                        ("revision", arguments_1.clone().select("revision").clone()),
                        ("entries", bindings_2.clone().select("entries").clone()),
                        ("apps", apps_source.clone()),
                        ("policyOnly", NixValue::from(true)),
                    ]));

                    let when_false = {
                        let when_true = host::expression()
                            .call(NixValue::record([
                                ("lib", lib.clone()),
                                ("nixpkgs", nixpkgs.clone()),
                                (
                                    "nixpkgsSource",
                                    arguments_1.clone().select("nixpkgsSource").clone(),
                                ),
                                (
                                    "qubesSource",
                                    arguments_1.clone().select("qubesSource").clone(),
                                ),
                                (
                                    "libvirtSource",
                                    arguments_1.clone().select("libvirtSource").clone(),
                                ),
                                ("machine", arguments_1.clone().select("machine").clone()),
                                ("generated", arguments_1.clone().select("generated").clone()),
                                ("variant", arguments_1.clone().select("variant").clone()),
                                ("revision", arguments_1.clone().select("revision").clone()),
                                ("entries", bindings_2.clone().select("entries").clone()),
                                ("apps", apps_source.clone()),
                            ]))
                            .merge_attrs(field(
                                ["vms"],
                                lib.clone()
                                    .select_segments(["mapAttrs"])
                                    .call(NixValue::function(|_| inspect.clone()))
                                    .call(bindings_2.clone().select("entries").clone()),
                            ));

                        let when_false = {
                            let when_true = bindings_2
                                .clone()
                                .select("entries")
                                .clone()
                                .select_segments(["sys-gui", "configuration", "config"])
                                .bind(|cfg| {
                                    NixValue::record([
                                        (
                                            "config",
                                            cfg.clone().select_segments([
                                                "environment",
                                                "etc",
                                                "airlock/capture-sender.json",
                                                "text",
                                            ]),
                                        ),
                                        (
                                            "retry",
                                            cfg.clone().select_segments([
                                                "environment",
                                                "etc",
                                                "xdg/autostart/airlock-capture-retry.desktop",
                                                "text",
                                            ]),
                                        ),
                                        (
                                            "session",
                                            cfg.clone().select_segments([
                                                "services",
                                                "displayManager",
                                                "defaultSession",
                                            ]),
                                        ),
                                    ])
                                });

                            let when_false = {
                                let when_true = NixValue::builtin("getAttr")
                                    .call(arguments_1.clone().select("captureSource").clone())
                                    .call(bindings_2.clone().select("entries").clone())
                                    .select_segments(["configuration"])
                                    .bind(|gui| {
                                        machine_expression_5(
                                            gui,
                                            arguments_1.clone(),
                                            bindings_2.clone(),
                                            lib.clone(),
                                        )
                                    });

                                let when_false = {
                                    let when_false = {
                                        let when_false = {
                                            let when_true = lib
                                                .clone()
                                                .select_segments(["mapAttrs"])
                                                .call(NixValue::function(|_| {
                                                    NixValue::function(|entry| {
                                                        entry
                                                            .clone()
                                                            .select_segments([
                                                                "configuration",
                                                                "config",
                                                            ])
                                                            .bind(|cfg| {
                                                                materialize
                                                                    .clone()
                                                                    .call(entry.clone())
                                                                    .bind(|descriptor| {
                                                                        machine_expression_3(
                                                                            descriptor,
                                                                            cfg.clone(),
                                                                        )
                                                                    })
                                                            })
                                                    })
                                                }))
                                                .call(bindings_2.clone().select("entries").clone());

                                            let when_false = NixValue::if_else(
                                                arguments_1
                                                    .clone()
                                                    .select("projection")
                                                    .clone()
                                                    .equals(NixValue::from("scale")),
                                                bindings_2
                                                    .clone()
                                                    .select("entries")
                                                    .clone()
                                                    .select_segments([
                                                        "usr-assistant",
                                                        "configuration",
                                                        "config",
                                                        "airlock",
                                                        "appGui",
                                                        "scale",
                                                    ]),
                                                NixValue::record([
                                                    (
                                                        "topology",
                                                        bindings_2
                                                            .clone()
                                                            .select("topology")
                                                            .clone(),
                                                    ),
                                                    (
                                                        "vms",
                                                        lib.clone()
                                                            .select_segments(["mapAttrs"])
                                                            .call(NixValue::function(|_| {
                                                                inspect.clone()
                                                            }))
                                                            .call(
                                                                bindings_2
                                                                    .clone()
                                                                    .select("entries")
                                                                    .clone(),
                                                            ),
                                                    ),
                                                ]),
                                            );

                                            NixValue::if_else(
                                                arguments_1
                                                    .clone()
                                                    .select("projection")
                                                    .clone()
                                                    .equals(NixValue::from("vm-settings")),
                                                when_true,
                                                when_false,
                                            )
                                        };

                                        NixValue::if_else(
                                            arguments_1
                                                .clone()
                                                .select("projection")
                                                .clone()
                                                .equals(NixValue::from("workspace")),
                                            bindings_2
                                                .clone()
                                                .select("entries")
                                                .clone()
                                                .select_segments([
                                                    "usr-assistant",
                                                    "configuration",
                                                    "config",
                                                    "environment",
                                                    "sessionVariables",
                                                    "AIRLOCK_WORKSPACE",
                                                ]),
                                            when_false,
                                        )
                                    };

                                    NixValue::if_else(
                                        arguments_1
                                            .clone()
                                            .select("projection")
                                            .clone()
                                            .equals(NixValue::from("desktop-session")),
                                        bindings_2
                                            .clone()
                                            .select("entries")
                                            .clone()
                                            .select_segments([
                                                "sys-gui",
                                                "configuration",
                                                "config",
                                                "services",
                                                "displayManager",
                                                "defaultSession",
                                            ]),
                                        when_false,
                                    )
                                };

                                NixValue::if_else(
                                    arguments_1
                                        .clone()
                                        .select("projection")
                                        .clone()
                                        .equals(NixValue::from("capture-implementation")),
                                    when_true,
                                    when_false,
                                )
                            };

                            NixValue::if_else(
                                arguments_1
                                    .clone()
                                    .select("projection")
                                    .clone()
                                    .equals(NixValue::from("capture")),
                                when_true,
                                when_false,
                            )
                        };

                        NixValue::if_else(
                            arguments_1.clone().select("wholeMachine").clone(),
                            when_true,
                            when_false,
                        )
                    };

                    NixValue::if_else(
                        arguments_1
                            .clone()
                            .select("projection")
                            .clone()
                            .equals(NixValue::from("capture-policy")),
                        when_true,
                        when_false,
                    )
                };

                NixValue::if_else(
                    arguments_1
                        .clone()
                        .select("projection")
                        .clone()
                        .equals(NixValue::from("desktop-consumer")),
                    when_true,
                    when_false,
                )
            };

            NixValue::if_else(
                arguments_1
                    .clone()
                    .select("projection")
                    .clone()
                    .equals(NixValue::from("desktop-implementation")),
                when_true,
                when_false,
            )
        };

        NixValue::if_else(
            arguments_1
                .clone()
                .select("projection")
                .clone()
                .equals(NixValue::from("physical-host")),
            when_true,
            when_false,
        )
    }
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_10(
    arguments: NixValue,
    apps_source: NixValue,
    arguments_1: NixValue,
    inspect: NixValue,
    lib: NixValue,
    materialize: NixValue,
    nixpkgs: NixValue,
) -> NixValue {
    NixValue::builtin("import")
        .call(add(
            arguments_1.clone().select("machine").clone(),
            NixValue::from("/generated/bootstrap/default.nix"),
        ))
        .select_segments(["builders"])
        .call(arguments.clone().merge_attrs(NixValue::record([
            (
                "airlockSupport",
                add(
                    source_path("/"),
                    arguments_1.clone().select("machine").clone(),
                ),
            ),
            (
                "modulesDirectory",
                add(
                    add(
                        source_path("/"),
                        arguments_1.clone().select("machine").clone(),
                    ),
                    NixValue::from("/generated/bootstrap"),
                ),
            ),
            ("airlockApps", apps_source.clone()),
        ])))
        .bind(|original| {
            recursive(|bindings_2| {
                machine_expression_6(
                    bindings_2,
                    Bindings::new([
                        ("apps_source", apps_source.clone()),
                        ("arguments", arguments.clone()),
                        ("arguments_1", arguments_1.clone()),
                        ("lib", lib.clone()),
                        ("materialize", materialize.clone()),
                        ("nixpkgs", nixpkgs.clone()),
                        ("original", original.clone()),
                    ]),
                )
            })
            .bind(|bindings_2| {
                machine_expression_9(
                    bindings_2,
                    apps_source.clone(),
                    arguments_1.clone(),
                    inspect.clone(),
                    lib.clone(),
                    materialize.clone(),
                    nixpkgs.clone(),
                )
            })
        })
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_11(
    apps_source: NixValue,
    arguments_1: NixValue,
    inspect: NixValue,
    lib: NixValue,
    nixpkgs: NixValue,
) -> NixValue {
    NixValue::function(|system| {
        machine_expression_8(
            system,
            apps_source.clone(),
            arguments_1.clone(),
            lib.clone(),
            nixpkgs.clone(),
        )
    })
    .bind(|mk_airlock_qube_builders| {
        NixValue::builtin("import")
            .call(add(
                apps_source.clone(),
                NixValue::from("/airlock-config/generated/displays.nix"),
            ))
            .select_segments(["materialize"])
            .bind(|materialize| {
                NixValue::record([
                    ("lib", lib.clone()),
                    ("mkAirlockQubeBuilders", mk_airlock_qube_builders.clone()),
                    ("airlockQubesVmKernelName", NixValue::from("6.12.64-1.fc41")),
                ])
                .bind(|arguments| {
                    machine_expression_10(
                        arguments,
                        apps_source.clone(),
                        arguments_1.clone(),
                        inspect.clone(),
                        lib.clone(),
                        materialize.clone(),
                        nixpkgs.clone(),
                    )
                })
            })
    })
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_12(
    nixpkgs: NixValue,
    arguments_1: NixValue,
    inspect: NixValue,
    lib: NixValue,
) -> NixValue {
    NixValue::builtin("toString")
        .call(NixValue::builtin("path").call({
            let field_filter = NixValue::function(|path| {
                NixValue::function(|r#type| {
                    lib.clone()
                        .select_segments(["cleanSourceFilter"])
                        .call(path.clone())
                        .call(r#type.clone())
                        .and(
                            !r#type.clone().equals(NixValue::from("directory")).and(
                                NixValue::builtin("elem")
                                    .call(NixValue::builtin("baseNameOf").call(path.clone()))
                                    .call(NixValue::list([
                                        NixValue::from("target"),
                                        NixValue::from("__pycache__"),
                                    ])),
                            ),
                        )
                })
            });

            NixValue::record([
                ("path", arguments_1.clone().select("apps").clone()),
                ("name", NixValue::from("airlock-apps")),
                ("filter", field_filter),
            ])
        }))
        .bind(|apps_source| {
            machine_expression_11(
                apps_source,
                arguments_1.clone(),
                inspect.clone(),
                lib.clone(),
                nixpkgs.clone(),
            )
        })
}

/// Build the machine expression contribution using the caller supplied deferred values.
fn machine_expression_13(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (
        vec![
            ("conflict", NixValue::from(false)),
            ("desktopConflict", NixValue::from(false)),
            ("scale", NixValue::null()),
            ("captureTarget", NixValue::null()),
            ("desktopSession", NixValue::null()),
            ("gnomeEnabled", NixValue::null()),
            ("captureSource", NixValue::from("sys-gui")),
            ("captureDestination", NixValue::from("usr-home")),
            ("senderContract", NixValue::null()),
            ("receiverContract", NixValue::null()),
            ("projection", NixValue::from("complete")),
            ("wholeMachine", NixValue::from(false)),
            ("revision", NixValue::null()),
        ],
        NixValue::builtin("import")
            .call(add(
                arguments_1.clone().select("nixpkgsSource").clone(),
                NixValue::from("/lib"),
            ))
            .select_segments(["extend"])
            .call(NixValue::function(|r#final| {
                NixValue::function(|_| {
                    field(
                        ["nixosSystem"],
                        NixValue::function(|args| {
                            NixValue::builtin("import")
                                .call(add(
                                    arguments_1.clone().select("nixpkgsSource").clone(),
                                    NixValue::from("/nixos/lib/eval-config.nix"),
                                ))
                                .call(args.clone().merge_attrs(field(["lib"], r#final.clone())))
                        }),
                    )
                })
            }))
            .bind(|lib| {
                NixValue::function(|entry| {
                    entry
                        .clone()
                        .select_segments(["configuration", "config"])
                        .bind(|cfg| machine_expression_2(cfg, entry.clone()))
                })
                .bind(|inspect| {
                    NixValue::record([
                        (
                            "outPath",
                            arguments_1.clone().select("nixpkgsSource").clone(),
                        ),
                        ("lib", lib.clone()),
                    ])
                    .bind(|nixpkgs| {
                        machine_expression_12(
                            nixpkgs,
                            arguments_1.clone(),
                            inspect.clone(),
                            lib.clone(),
                        )
                    })
                })
            }),
    )
}
