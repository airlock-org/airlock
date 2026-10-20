//! Machine role projections preserve Qubes runtime boundaries in both emitted candidates and image bootstrap configurations.

use super::{
    configuration::{inspect, text},
    *,
};

/// Command and personal roles keep their firewall/GUI relationships, development controls and persistent state across emission.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources and candidate authoring binary"]
fn command_and_personal_roles_preserve_bootstrap_and_candidate_relationships() -> Result<()> {
    let nix = context()?;
    let candidate = nix.candidate("roles")?;
    let data = inspect(
        &nix,
        "guest_roles/command_role",
        json!({"repo_root": nix.prepared.join("airlock-machine"), "repo_root2": nix.prepared.join("airlock-machine"), "candidate": candidate, "apps": nix.prepared.join("crates")}),
    )?;
    let expected: Value = serde_json::from_str(
        r#"{
    "role": "cmd",
    "hostname": "sample-cmd",
    "staged_dev_declared": true,
    "staged_sys_net_declared": true,
    "staged_sys_net_autostart": true,
    "staged_sys_net_virt_mode": "hvm",
    "staged_sys_net_updates_proxy": true,
    "staged_sys_firewall_declared": true,
    "staged_sys_firewall_netvm": "sys-net",
    "staged_sys_firewall_autostart": true,
    "staged_sys_firewall_provides_network": true,
    "staged_sys_firewall_servicevm": "1",
    "staged_sys_firewall_updates_proxy": true,
    "staged_sys_gui_declared": true,
    "staged_sys_gui_role": "sys-gui",
    "staged_sys_gui_netvm": null,
    "staged_sys_gui_guivm": "dom0",
    "staged_sys_gui_autostart": true,
    "staged_sys_gui_agent_feature": "1",
    "staged_sys_gui_service": true,
    "staged_default_guivm": "sys-gui",
    "staged_default_netvm": "sys-firewall",
    "generated_dev_domain_netvm": null,
    "generated_dev_domain_role_feature": "cmd",
    "generated_dev_domain_marker_feature": "1",
    "bootstrap_dev_domain_present": true,
    "bootstrap_sys_net_present": true,
    "bootstrap_sys_firewall_present": true,
    "bootstrap_sys_net_autostart": true,
    "bootstrap_sys_net_netvm": null,
    "bootstrap_sys_net_virt_mode": "hvm",
    "bootstrap_sys_net_updates_proxy": true,
    "bootstrap_sys_firewall_netvm": "sys-net",
    "bootstrap_sys_firewall_autostart": true,
    "bootstrap_sys_firewall_provides_network": true,
    "bootstrap_sys_firewall_servicevm": "1",
    "bootstrap_sys_firewall_updates_proxy": true,
    "bootstrap_sys_gui_present": true,
    "bootstrap_sys_gui_guivm": "dom0",
    "bootstrap_sys_gui_autostart": true,
    "bootstrap_sys_gui_agent_feature": "1",
    "bootstrap_default_guivm": "sys-gui",
    "bootstrap_topology_default_guivm": "sys-gui",
    "sys_gui_role_enabled": true,
    "sys_gui_agent_enabled": true,
    "sys_gui_login_enabled": true,
    "sys_gui_autologin": false,
    "sys_gui_login_wanted_by": [],
    "sys_gui_login_password": "airlock",
    "dom0_greeter_enabled": false,
    "dom0_autologin": true,
    "sys_gui_template_tools_removed": true,
    "dom0_xfce_enabled": false,
    "dom0_xterm_enabled": false,
    "sys_gui_rpc_present": true,
    "dom0_sys_gui_policy_present": true,
    "dom0_sys_gui_session_policy": true,
    "dom0_sys_gui_system_domain_visibility": true,
    "dom0_sys_gui_management_policy": true,
    "dom0_sys_gui_session_present": true,
    "dom0_default_session": "sys-gui",
    "dom0_bootstrap_status_device": "/dev/tty1",
    "dom0_bootstrap_status_environment": "/dev/tty1",
    "bootstrap_default_netvm": "sys-firewall",
    "bootstrap_topology_default_netvm": "sys-firewall",
    "bootstrap_dev_topology_present": true,
    "bootstrap_sys_net_topology_autostart": true,
    "bootstrap_sys_firewall_topology_netvm": "sys-net",
    "bootstrap_sys_firewall_topology_autostart": true,
    "image_dev_mode_enabled": true,
    "dom0_dhcp_enabled": true,
    "dom0_dhcpcd_no_gateway": true,
    "dom0_proxy_enabled": true,
    "dom0_proxy_target": "sys-firewall",
    "dom0_proxy_http": "http://127.0.0.1:8082",
    "dom0_proxy_https": "http://127.0.0.1:8082",
    "dom0_proxy_no_proxy": "127.0.0.1,localhost,::1",
    "dom0_nix_daemon_proxy_http": "http://127.0.0.1:8082",
    "dom0_dev_mode_marker": "enabled\n",
    "dom0_observability_enabled": true,
    "dom0_testing_tools_enabled": true,
    "release_image_dev_mode_enabled": false,
    "release_dom0_dhcp_enabled": false,
    "release_dom0_dhcpcd_no_gateway": false,
    "release_dom0_proxy_enabled": false,
    "release_dom0_proxy_http": null,
    "release_dom0_dev_marker_absent": true,
    "release_dom0_observability_enabled": false,
    "release_dom0_testing_tools_enabled": false,
    "release_bootstrap_dev_domain_present": true,
    "release_bootstrap_sys_net_present": true,
    "release_bootstrap_sys_net_netvm": null,
    "release_bootstrap_sys_net_autostart": true,
    "release_bootstrap_sys_net_virt_mode": "hvm",
    "release_bootstrap_sys_firewall_present": true,
    "release_bootstrap_sys_firewall_netvm": "sys-net",
    "release_bootstrap_sys_firewall_autostart": true,
    "non_dev_image_dev_mode_enabled": false,
    "non_dev_dom0_dhcp_enabled": false,
    "non_dev_dom0_dhcpcd_no_gateway": false,
    "non_dev_dom0_proxy_enabled": false,
    "non_dev_dom0_proxy_http": null,
    "non_dev_dom0_dev_marker_absent": true,
    "non_dev_dom0_observability_enabled": false,
    "non_dev_dom0_testing_tools_enabled": false,
    "network_client_resolvconf_disabled": true,
    "private_nix_enabled": true,
    "nix_allowed_users": [
        "user"
    ],
    "codex_home": "/home/user/.codex",
    "codex_home_env": "/home/user/.codex",
    "dev_init_contains_dev_path": true,
    "dev_init_contains_codex_home": true,
    "dev_init_contains_private_nix_root": true,
    "dev_forbidden_rpc_absent": true,
    "retired_config_policy_enabled": false,
    "dev_no_secret_env": true,
    "dev_no_codex_auth_etc": true,
    "base_role": "base",
    "base_private_workspace_absent": true,
    "base_forbidden_rpc_absent": true
}"#,
    )?;
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&data[key], value, "{key}");
    }
    for feature in ["nix-command", "flakes"] {
        assert!(
            data["nix_features"]
                .as_array()
                .unwrap()
                .contains(&json!(feature))
        );
    }
    assert!(
        data["dev_persistent_paths"]
            .as_array()
            .unwrap()
            .contains(&json!("/home/user/dev"))
    );
    for field in ["dev_qrexec_requires", "dev_qrexec_after"] {
        assert!(
            data[field]
                .as_array()
                .unwrap()
                .contains(&json!("airlock-cmd-guest-init.service"))
        );
    }
    assert!(text(&data["sys_gui_pam"]).contains("pam_systemd.so"));
    for forbidden in ["system-auth", "postlogin"] {
        assert!(!text(&data["sys_gui_pam"]).contains(forbidden));
    }
    assert!(text(&data["sys_gui_session_launcher"]).ends_with("/bin/airlock-gui-login-session"));
    assert!(text(&data["sys_gui_login_xserver"]).ends_with("/bin/airlock-gui-login-xserver"));
    for profile in ["usb-lab", "nested"] {
        let report = inspect(
            &nix,
            "usr_home_gnome/personal_gui_selection",
            json!({"root": nix.prepared.join("airlock-machine"), "profile": profile, "candidate": candidate, "apps": nix.prepared.join("crates")}),
        )?;
        assert_eq!(report["defaultGuiVM"], "sys-gui");
        assert_eq!(report["personalGuiVMs"], json!({"usr-home": "sys-gui"}));
        assert_eq!(report["runtimeGuiVMs"], report["personalGuiVMs"]);
    }
    let report = inspect(
        &nix,
        "guest_roles/minimal_vm",
        json!({"repo_root": nix.prepared.join("airlock-machine")}),
    )?;
    for field in [
        "documentation_disabled",
        "perlless_mechanics_enabled",
        "installer_tools_disabled",
        "stock_runtime_retained",
        "rust_authoring_inputs_available",
        "required_qubes_services_retained",
    ] {
        assert_eq!(report[field], true, "{field}");
    }
    for field in ["staging", "validator", "vault"] {
        assert_eq!(report["nix_cli_capability"][field], true);
        assert_eq!(report["config_inputs_import_rpc"][field], false);
    }
    Ok(())
}

/// Custom Rust-authored candidates retain VM artifacts, host profile provenance and live-versus-boot activation intent.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources and candidate authoring binaries"]
fn emitted_candidates_preserve_custom_vm_and_host_activation_contracts() -> Result<()> {
    let nix = context()?;
    let work = nix.candidate("custom")?;
    let evaluate = |label: &str, segments: &[&str]| -> Result<Value> {
        nix.evaluate(
            label,
            "candidate/evaluate",
            json!({"candidate": work, "segments": segments}),
        )?
        .value()
    };
    let topology = evaluate("candidate-default-topology", &["airlock", "topology"])?;
    let domains = &topology["domains"];
    assert!(domains["sys-net"]["netvm"].is_null());
    assert_eq!(domains["sys-firewall"]["netvm"], "sys-net");
    assert_eq!(domains["sys-ctrl"]["prefs"]["memory"], 8192);
    assert_eq!(domains["usr-home"]["netvm"], "sys-firewall");
    assert_eq!(domains["usr-home"]["prefs"]["autostart"], true);
    assert_eq!(
        domains["usr-home"]["backend"]["state"]["persistent"],
        "usr-home-private"
    );
    assert_eq!(domains["usr-home"]["features"]["menu-items"], "*");
    let system = evaluate("candidate-default-system", &["airlock", "system"])?;
    assert_eq!(
        system["dom0"]["profile"]["source_ref"]["attr"],
        "packages.x86_64-linux.airlock-dom0-profile"
    );
    assert_eq!(system["dom0"]["profile"]["message"], "initial");
    assert_eq!(system["dom0"]["nixos"]["system"]["switch_action"], "switch");
    assert_eq!(system["plan"]["reboot_required"], false);
    assert_eq!(
        evaluate(
            "candidate-default-message",
            &[
                "nixosConfigurations",
                "airlock-dom0",
                "config",
                "environment",
                "etc",
                "airlock/inside-config-applied-message",
                "text"
            ]
        )?,
        "initial\n"
    );

    let emitter = common::binary("AIRLOCK_CONFIG_FIXTURE_BIN", "examples/candidate_fixture");
    for action in ["switch", "boot"] {
        fs::write(
            work.join("smoke-inputs.json"),
            serde_json::to_vec(
                &json!({"created": "created-config", "message": "rust-candidate", "action": action}),
            )?,
        )?;
        common::run(Command::new(&emitter).current_dir(&work), None)?.checked()?;
        // Use distinct input identity after changing an emitted candidate; cached evaluations must never mask new source.
        let copy = nix.temporary.path().join(format!("custom-{action}"));
        copy_tree(&work, &copy)?;
        let value = |label: &str, segments: &[&str]| -> Result<Value> {
            nix.evaluate(
                label,
                "candidate/evaluate",
                json!({"candidate": copy, "segments": segments}),
            )?
            .value()
        };
        let topology = value(
            &format!("custom-topology-{action}"),
            &["airlock", "topology"],
        )?;
        let domain = &topology["domains"]["created-config"];
        assert_eq!(domain["class"], "StandaloneVM");
        assert!(domain["netvm"].is_null());
        assert_eq!(domain["prefs"]["memory"], 1024);
        assert_eq!(
            domain["backend"]["system"]["source_ref"]["attr"],
            "nixosConfigurations.airlock-domain-created-config.config.system.build.toplevel"
        );
        assert_eq!(
            domain["backend"]["root"]["source_ref"]["subpath"],
            "root.img"
        );
        assert_eq!(domain["backend"]["store"]["read_only"], true);
        assert_eq!(
            value(
                &format!("custom-message-{action}"),
                &[
                    "nixosConfigurations",
                    "airlock-domain-created-config",
                    "config",
                    "environment",
                    "etc",
                    "airlock/applied-message",
                    "text"
                ]
            )?,
            "rust-candidate\n"
        );
        let plan = value(
            &format!("custom-system-{action}"),
            &["airlock", "system", "plan"],
        )?;
        assert_eq!(plan["reboot_required"], action == "boot");
        if action == "switch" {
            assert!(
                plan["live_actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|action| action["type"] == "systemd-service"
                        && action["action"] == "restart")
            );
        }
    }
    Ok(())
}

/// The default candidate's host profile is a separately buildable artifact with controller provenance.
#[test]
#[ignore = "explicit candidate profile package build"]
fn package_candidate_profile() -> Result<()> {
    let nix = context()?;
    let candidate = nix.candidate("profile-package")?;
    crate::packaged::validate_candidate_profile(&candidate)
}
