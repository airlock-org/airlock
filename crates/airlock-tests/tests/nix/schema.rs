//! Closed schemas preserve runtime contracts and reject configuration that would widen authority.

use super::*;
use std::collections::BTreeSet;

/// Evaluate JSON-authored data through the real normalization library and retain rejected inputs.
fn schema(nix: &Nix, library: &str, config: Value, selector: &[&str]) -> Result<Reply> {
    nix.evaluate(
        &format!(
            "schema-{library}-{config_hash}",
            config_hash = selector.join("-")
        ),
        "schema/evaluate",
        json!({
            "machine": nix.prepared.join("airlock-machine"),
            "source": nix.nixpkgs()?,
            "library": library,
            "config": config,
            "selector": selector
        }),
    )
}

/// Run a complete schema example with a private source tree and admitted nixpkgs library.
fn scenario(nix: &Nix, name: &str, case: &str) -> Result<Reply> {
    let directory = nix
        .temporary
        .path()
        .join(format!("schema-fixture-{name}-{case}"));
    fs::create_dir_all(directory.join("airlock"))?;
    for name in ["template-source", "kernel-source", "system-source"] {
        fs::create_dir_all(directory.join(name))?;
    }
    for name in ["topology.nix", "system-schema.nix"] {
        fs::copy(
            nix.prepared.join("airlock-machine/generated/schemas.nix"),
            directory.join("airlock").join(name),
        )?;
    }

    nix.evaluate(
        &format!("schema-{name}-{case}"),
        "schema/scenario",
        json!({
            "source": nix.nixpkgs()?,
            "fixtureRoot": directory,
            "scenario": name,
            "case": case
        }),
    )
}

/// Authored option names normalize consistently; unknown fields cannot hide beside a known field.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn canonical_options_preserve_the_runtime_contract() -> Result<()> {
    let nix = context()?;
    let cases = [
        (
            "system",
            "dom0.profile",
            "activationCommand",
            "activation_command",
            json!("bin/custom-activate"),
        ),
        (
            "system",
            "dom0.profile",
            "activationAction",
            "activation_action",
            json!("none"),
        ),
        (
            "system",
            "dom0.nixos.system",
            "switchAction",
            "switch_action",
            json!("boot"),
        ),
        (
            "topology",
            "templates.test",
            "skipStart",
            "skip_start",
            json!(false),
        ),
        ("topology", "root", "readOnly", "read_only", json!(false)),
        ("topology", "store", "readOnly", "read_only", json!(true)),
        (
            "topology",
            "store",
            "blobDevice",
            "blob_device",
            json!("/dev/xvdf"),
        ),
        (
            "topology",
            "store",
            "viewBackend",
            "view_backend",
            json!("erofs-objectmap"),
        ),
        (
            "topology",
            "state",
            "persistentSize",
            "persistent_size",
            json!(8192),
        ),
        (
            "topology",
            "",
            "qrexecPolicy",
            "qrexec_policy",
            json!({"rules": []}),
        ),
    ];
    for (library, context, authored, normalized, expected) in cases {
        for invalid in [false, true] {
            let assignment = if invalid {
                json!({authored: expected, "unrecognizedField": expected})
            } else {
                json!({authored: expected})
            };
            let (config, selector) = if ["root", "store", "state"].contains(&context) {
                let mut backend = json!({
                    "type": "nix-closure",
                    "system": {"source": "/nix/store/test-system"},
                    "kernel": {"name": "test-kernel"},
                    "root": {"source": "/nix/store/test-root"},
                    "store": {"source": "/nix/store/test-store"}
                });
                if backend[context].is_null() {
                    backend[context] = json!({});
                }
                backend[context]
                    .as_object_mut()
                    .unwrap()
                    .extend(assignment.as_object().unwrap().clone());
                (
                    json!({"domains": {"test": {"label": "blue", "backend": backend}}}),
                    vec!["domains", "test", "backend", context, normalized],
                )
            } else {
                let mut config = assignment;
                let mut selector = if context.is_empty() {
                    Vec::new()
                } else {
                    context.split('.').collect()
                };
                for segment in selector.iter().rev() {
                    config = json!({*segment: config});
                }
                selector.push(normalized);
                (config, selector)
            };

            let reply = schema(&nix, library, config, &selector)?;
            if invalid {
                assert!(!reply.success);
                assert!(
                    reply.stderr.contains("unrecognizedField"),
                    "{error}",
                    error = reply.stderr
                );
            } else {
                assert_eq!(reply.value()?, expected);
            }
        }
    }
    Ok(())
}

/// Template and closure backends keep explicit ownership, immutable store policy and ordered RPC rules.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn topology_normalizes_each_backend_and_rejects_unsafe_policy() -> Result<()> {
    let nix = context()?;
    let report = scenario(&nix, "valid", "")?.value()?;
    assert_eq!(report["schema"], "airlock.qubes-topology.v1");
    let template = &report["templates"]["airlock-template"];
    assert_eq!(template["import"], true);
    assert_eq!(template["skip_start"], true);
    assert!(
        template["source"]
            .as_str()
            .unwrap()
            .ends_with("/template-source")
    );
    assert!(template["source_ref"].is_null());
    let built = &report["templates"]["airlock-built-template"];
    assert_eq!(built["import"], true);
    assert!(built["source"].is_null());
    assert_eq!(
        built["source_ref"],
        json!({
            "type": "flake-output",
            "attr": "packages.x86_64-linux.airlock-built-template",
            "subpath": "airlock-built-template"
        })
    );
    assert!(
        report["kernels"]["6.12.64-1.fc41"]["source"]
            .as_str()
            .unwrap()
            .ends_with("/kernel-source")
    );

    let app = &report["domains"]["test-qube"];
    assert_eq!(app["class"], "AppVM");
    assert_eq!(app["template"], "airlock-template");
    assert_eq!(
        app["backend"],
        json!({"type": "qubes-template", "template": "airlock-template"})
    );
    assert!(app["netvm"].is_null());
    for key in ["autostart", "include_in_backups"] {
        assert_eq!(app["prefs"][key], false);
    }
    assert_eq!(app["prefs"]["memory"], 1024);
    assert_eq!(app["services"]["network-manager"], false);
    assert_eq!(
        app["devices"]["pci"],
        json!([{
            "backend": "dom0", "id": "00_14.0", "match": "port", "mode": "required",
            "options": {"no-strict-reset": "True"}
        }])
    );
    let legacy = &report["domains"]["legacy-fedora"];
    assert_eq!(legacy["class"], "AppVM");
    assert_eq!(legacy["template"], "airlock-template");
    assert_eq!(legacy["label"], "yellow");
    assert_eq!(legacy["backend"], app["backend"]);

    let vm = &report["domains"]["nix-work"];
    assert_eq!(vm["class"], "StandaloneVM");
    assert!(vm["template"].is_null());
    assert_eq!(vm["label"], "blue");
    assert_eq!(vm["prefs"]["memory"], 2048);
    let backend = &vm["backend"];
    assert_eq!(backend["type"], "nix-closure");
    assert!(
        backend["system"]["source"]
            .as_str()
            .unwrap()
            .ends_with("/system-source")
    );
    assert!(backend["system"]["source_ref"].is_null());
    assert_eq!(backend["kernel"]["name"], "6.12.64-1.fc41");
    for key in ["source", "source_ref"] {
        assert!(backend["kernel"][key].is_null());
    }
    for key in ["initrd", "modules"] {
        assert!(backend[key]["name"].is_null());
    }
    assert_eq!(backend["root"]["mode"], "qubes-root-image");
    assert_eq!(backend["root"]["read_only"], false);
    assert_eq!(backend["store"]["mode"], "shared-closure");
    assert_eq!(backend["store"]["mountpoint"], "/nix/store");
    assert_eq!(backend["store"]["device"], "/dev/xvde");
    assert_eq!(backend["store"]["read_only"], true);
    for (artifact, suffix, subpath) in [
        ("root", "root-image", "root.img"),
        ("store", "store-closure", ""),
    ] {
        assert!(backend[artifact]["source"].is_null());
        assert_eq!(
            backend[artifact]["source_ref"],
            json!({
                "type": "flake-output",
                "attr": format!("packages.x86_64-linux.nix-work-{suffix}"),
                "subpath": subpath
            })
        );
    }
    assert_eq!(backend["state"]["persistent"], "nix-work-private");
    assert!(backend["state"]["volatile"].is_null());
    assert_eq!(
        report["qrexec_policy"]["rules"],
        json!([
            {"service": "airlock.PolicyProbe", "argument": "*", "source": "dom0", "target": "test-qube", "action": "allow"},
            {"service": "airlock.PolicyProbe", "argument": "*", "source": "@anyvm", "target": "@anyvm", "action": "deny"}
        ])
    );

    for (case, error) in [
        (
            "missing-appvm-template",
            "VM class, backend and template must agree.",
        ),
        ("invalid-label", "domains.bad.label"),
        ("unknown-option", "domains.bad.servies"),
        ("invalid-pci-device-id", ".id"),
        ("invalid-pci-assignment-mode", ".mode"),
        (
            "template-import-without-source",
            "An imported template requires source or sourceRef.",
        ),
        (
            "template-source-and-source-ref",
            "An artifact cannot set both source and sourceRef.",
        ),
        (
            "template-invalid-build-ref",
            "templates.airlock-template.source.build",
        ),
        ("non-airlock-qrexec-service", ".service"),
        ("reserved-qrexec-service", ".service"),
        (
            "qrexec-allow-any-source",
            "Topology policy cannot target dom0 or grant wildcard permissions.",
        ),
        (
            "qrexec-target-dom0",
            "Topology policy cannot target dom0 or grant wildcard permissions.",
        ),
        (
            "nix-closure-with-template",
            "VM class, backend and template must agree.",
        ),
        (
            "nix-closure-missing-system",
            "A closure-backed VM requires a NixOS system source.",
        ),
        (
            "nix-closure-writable-store",
            "domains.bad.backend.store.readOnly",
        ),
        (
            "nix-closure-unsupported-store-device",
            "domains.bad.backend.store.device",
        ),
        (
            "nix-closure-missing-root",
            "A closure-backed VM requires a root image.",
        ),
        (
            "nix-closure-readonly-root",
            "domains.bad.backend.root.readOnly",
        ),
    ] {
        let reply = scenario(&nix, "invalid", case)?;
        assert!(!reply.success, "schema accepted {case}");
        assert!(
            reply.stderr.contains(error),
            "{case}: {diagnostic}",
            diagnostic = reply.stderr
        );
    }
    Ok(())
}

/// Activation plans distinguish live service work from boot installation and reboot-only changes.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn system_plans_distinguish_live_boot_and_reboot_actions() -> Result<()> {
    let nix = context()?;
    let live = scenario(&nix, "live", "")?.value()?;
    assert_eq!(live["schema"], "airlock.system.v1");
    assert_eq!(
        live["dom0"]["profile"]["source_ref"]["attr"],
        "packages.x86_64-linux.airlock-dom0-profile"
    );
    assert_eq!(
        live["dom0"]["profile"]["activation_command"],
        "bin/airlock-dom0-profile-activate"
    );
    assert_eq!(live["dom0"]["profile"]["message"], "hello");
    assert_eq!(live["dom0"]["packages"][0]["name"], "airlock-dom0-profile");
    assert_eq!(
        live["dom0"]["systemd"]["services"]["airlock-example"]["action"],
        "restart"
    );
    assert_eq!(live["plan"]["reboot_required"], false);
    assert_eq!(live["plan"]["reboot_actions"], json!([]));
    let actions = live["plan"]["live_actions"].as_array().unwrap();
    assert!(
        actions
            .iter()
            .any(|action| action["type"] == "dom0-profile" && action["message"] == "hello")
    );
    assert!(
        actions
            .iter()
            .any(|action| action["type"] == "systemd-service"
                && action["name"] == "airlock-example"
                && action["action"] == "restart")
    );

    let reboot = scenario(&nix, "reboot", "")?.value()?;
    assert_eq!(reboot["schema"], "airlock.system.v1");
    assert_eq!(
        reboot["dom0"]["nixos"]["system"]["source_ref"]["attr"],
        "nixosConfigurations.dom0.config.system.build.toplevel"
    );
    assert_eq!(reboot["dom0"]["nixos"]["system"]["switch_action"], "switch");
    for key in ["system", "kernel", "xen", "bootloader", "qubes"] {
        assert_eq!(reboot["dom0"]["nixos"][key]["enable"], true);
    }
    assert_eq!(reboot["plan"]["reboot_required"], true);
    assert!(
        reboot["plan"]["live_actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action["type"] == "dom0-nixos-system"
                && action["action"] == "switch"
                && action["reason"] == "new dom0 system generation")
    );
    let reasons: BTreeSet<_> = [
        "dom0 kernel change requires reboot",
        "dom0 xen change requires reboot",
        "dom0 bootloader change requires reboot",
        "dom0 qubes change requires reboot",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        reboot["plan"]["reboot_actions"].as_array().unwrap().len(),
        4
    );
    for actual in [
        reboot["plan"]["reboot_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        reboot["plan"]["reboot_actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| action["reason"].as_str().unwrap())
            .collect(),
    ] {
        assert_eq!(actual, reasons);
    }

    let boot = scenario(&nix, "boot", "")?.value()?;
    assert_eq!(boot["schema"], "airlock.system.v1");
    assert_eq!(boot["dom0"]["nixos"]["system"]["switch_action"], "boot");
    assert_eq!(boot["plan"]["reboot_required"], true);
    for (key, action_key) in [
        ("live_actions", "action"),
        ("reboot_actions", "switch_action"),
    ] {
        let actions = boot["plan"][key].as_array().unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0]["type"], "dom0-nixos-system");
        assert_eq!(actions[0][action_key], "boot");
        assert_eq!(actions[0]["reason"], "install next dom0 boot generation");
    }
    assert_eq!(
        boot["plan"]["reboot_reasons"],
        json!(["install next dom0 boot generation"])
    );
    Ok(())
}

/// Place an artifact beneath each real caller while keeping shared parser failures in one representative context.
fn source_fields(nix: &Nix, context: &str, fields: Value) -> Result<Reply> {
    let (library, config, selector) = if context.starts_with("dom0.") || context == "templates.test"
    {
        let mut config = fields;
        let selector: Vec<_> = context.split('.').collect();
        for segment in selector.iter().rev() {
            config = json!({(*segment): config});
        }
        (
            if context.starts_with("dom0.") {
                "system"
            } else {
                "topology"
            },
            config,
            selector,
        )
    } else {
        let mut backend = json!({"type": "nix-closure", "system": {"source": "/nix/store/test-system"},
            "kernel": {"name": "test-kernel"}, "root": {"source": "/nix/store/test-root"}, "store": {"source": "/nix/store/test-store"}});
        backend[context] = fields;
        (
            "topology",
            json!({"domains": {"test": {"label": "blue", "backend": backend}}}),
            vec!["domains", "test", "backend", context],
        )
    };
    schema(nix, library, config, &selector)
}

/// Every artifact caller preserves build and concrete source identities; shared invalid fields and paths fail closed.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn source_references_normalize_without_allowing_aliases_or_path_traversal() -> Result<()> {
    let nix = context()?;
    let output = "packages.x86_64-linux.test-source";
    for context in [
        "dom0.profile",
        "dom0.nixos.system",
        "dom0.nixos.kernel",
        "templates.test",
        "system",
        "kernel",
        "initrd",
        "modules",
        "root",
        "store",
    ] {
        let normalized =
            source_fields(&nix, context, json!({"sourceRef": {"build": output}}))?.value()?;
        assert!(normalized["source"].is_null());
        assert_eq!(
            normalized["source_ref"],
            json!({"type": "flake-output", "attr": output, "subpath": ""})
        );
        let concrete = source_fields(
            &nix,
            context,
            json!({"source": "/nix/store/test-source", "sourceRef": null}),
        )?
        .value()?;
        assert_eq!(concrete["source"], "/nix/store/test-source");
        assert!(concrete["source_ref"].is_null());
    }
    for context in ["dom0.profile", "templates.test", "root"] {
        let fields = if context == "root" {
            json!({"sourceRef": {"build": output, "subpath": "root.img"}})
        } else {
            json!({"source": {"build": output, "subpath": "root.img"}})
        };
        assert_eq!(
            source_fields(&nix, context, fields)?.value()?["source_ref"],
            json!({"type": "flake-output", "attr": output, "subpath": "root.img"})
        );
    }
    assert_eq!(
        source_fields(&nix, "dom0.profile", json!({"source": {"build": output}}))?.value()?["source_ref"]
            ["attr"],
        output
    );
    for (fields, expected) in [
        (json!({"source_ref": {"build": output}}), ".source_ref"),
        (json!({"source_ref": null}), ".source_ref"),
        (
            json!({"sourceRef": {"build": output}, "source_ref": {"build": output}}),
            ".source_ref",
        ),
        (
            json!({"source": "/nix/store/test-source", "sourceRef": {"build": output}}),
            "cannot set both source and sourceRef",
        ),
    ] {
        let reply = source_fields(&nix, "dom0.profile", fields)?;
        assert!(!reply.success);
        assert!(
            reply.stderr.contains(expected),
            "{error}",
            error = reply.stderr
        );
    }
    let mut references = vec![
        (json!({"attr": output}), ".attr"),
        (json!({"build": output, "attr": output}), ".attr"),
        (json!({}), ".build"),
        (json!({"build": "invalid/output"}), ".build"),
        (json!({"build": output, "subpath": "/root.img"}), ".subpath"),
    ];
    for subpath in [
        ".",
        "..",
        "../root.img",
        "images/../root.img",
        "images/./root.img",
        "images/..",
    ] {
        references.push((json!({"build": output, "subpath": subpath}), ".subpath"));
    }
    for (reference, expected) in references {
        let reply = source_fields(&nix, "dom0.profile", json!({"source": reference}))?;
        assert!(!reply.success);
        assert!(
            reply.stderr.contains(expected),
            "{error}",
            error = reply.stderr
        );
    }
    for subpath in [".hidden/root.img", "images/root..img", "images/.hidden"] {
        assert_eq!(
            source_fields(
                &nix,
                "dom0.profile",
                json!({"source": {"build": output, "subpath": subpath}})
            )?
            .value()?["source_ref"]["subpath"],
            subpath
        );
    }
    for command in ["..", "../activate", "bin/../activate", "bin/./activate"] {
        let reply = schema(
            &nix,
            "system",
            json!({"dom0": {"profile": {"activationCommand": command}}}),
            &["dom0", "profile"],
        )?;
        assert!(!reply.success);
        assert!(reply.stderr.contains("activationCommand"));
    }
    Ok(())
}
