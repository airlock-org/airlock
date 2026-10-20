//! Declare the GUI VM's explicit Qubes control rights without permitting host administration.

#[path = "policy_methods.rs"]
mod methods;

use airlock_build::script::substitute;
use rusix::{interop::raw::NixValue, nix_text, nixos::ModuleContext};

/// Build Qubes Manager policy from explicit API methods and the declared system VM identities.
pub(super) fn gui(context: &ModuleContext) -> NixValue {
    let domains = context
        .lib
        .clone()
        .select("filter")
        .call(context.lib.clone().select("hasPrefix").call("sys-"))
        .call(
            NixValue::builtin("attrNames")
                .call(context.config.clone().select("airlock.qubes.domains")),
        );
    let system_domains = context
        .lib
        .clone()
        .select("concatMapStringsSep")
        .call("\n")
        .call(NixValue::function(|domain| {
            nix_text!(
                r#"
            admin.vm.List * sys-gui {domain} allow target=dom0
            admin.Events * sys-gui {domain} allow target=dom0
        "#,
                domain = domain,
            )
            .into()
        }))
        .call(domains);
    let classes = ["AppVM", "StandaloneVM", "TemplateVM"]
        .map(|class| {
            format!(
                r#"admin.vm.Create.{class} * sys-gui dom0 allow
admin.vm.CreateInPool.{class} * sys-gui dom0 allow
"#
            )
        })
        .join("\n");
    let methods = methods::METHODS
        .iter()
        .map(|method| format!("{method} * sys-gui @anyvm allow target=dom0"))
        .collect::<Vec<_>>()
        .join("\n");
    let text = substitute(
        include_str!("gui.policy"),
        [
            ("systemDomains", system_domains),
            ("classes", classes.into()),
            ("methods", methods.into()),
        ],
    );

    context
        .pkgs
        .clone()
        .select("writeTextDir")
        .call("etc/qubes/policy.d/50-airlock-sys-gui.policy")
        .call(text)
}
