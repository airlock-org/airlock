//! Deferred VM factories preserve caller context, priority helpers and lazy branches.

use super::*;

/// Compare the Rust export with its independent reference across target and debug branches.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn factories_preserve_target_debug_context_and_laziness() -> Result<()> {
    let nix = context()?;
    let evaluate = |name: &str, overrides: Value| -> Result<Value> {
        let values = nix
            .evaluate(
                name,
                "equivalence",
                json!({
                    "generated": nix.bootstrap,
                    "machine": nix.prepared.join("airlock-machine"),
                    "nixpkgs": nix.nixpkgs()?,
                    "overrides": serde_json::to_string(&overrides)?
                }),
            )?
            .value()?;
        Ok(values["candidate"].clone())
    };

    for (system, debug) in [
        ("x86_64-linux", false),
        ("x86_64-linux", true),
        ("aarch64-linux", false),
    ] {
        let result = evaluate(
            &format!("factory-{system}-{debug}"),
            json!({"system":system,"networkDebug":debug}),
        )?;
        let entries = &result["entries"];
        assert_eq!(entries["sys-net"]["builderSystem"], system);
        assert_eq!(entries["sys-firewall"]["prefs"]["autostart"], !debug);
        assert_eq!(
            entries["sys-net"]["modules"][0]["config"]["systemd"]["services"]["NetworkManager"]["wantedBy"],
            if debug {
                json!([])
            } else {
                json!(["multi-user.target"])
            }
        );
    }

    assert_eq!(
        evaluate("factory-default-debug", json!({"omitDebug":true}))?["entries"]["sys-firewall"]["prefs"]
            ["autostart"],
        true
    );

    assert_eq!(
        evaluate(
            "factory-caller-priority",
            json!({"networkDebug":true,"forceWantedBy":["custom.target"]})
        )?["entries"]["sys-net"]["modules"][0]["config"]["systemd"]["services"]["NetworkManager"]["wantedBy"],
        json!(["custom.target"])
    );

    evaluate(
        "factory-lazy-branch",
        json!({"networkDebug":false,"poisonForce":true}),
    )?;

    let result = evaluate(
        "factory-string-context",
        json!({"kernel":"caller-selected-kernel","contextKernel":true}),
    )?;
    assert!(!result["kernelContext"].as_object().unwrap().is_empty());

    for (selection, expected) in [
        ("driver-name", "sys-net"),
        ("firewall-name", "sys-firewall"),
    ] {
        assert_eq!(
            evaluate(selection, json!({"selection":selection,"poisonForce":true}))?,
            expected
        );
    }

    Ok(())
}
