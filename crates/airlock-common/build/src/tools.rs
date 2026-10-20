//! Install native entry points with explicit dependencies on services and upstream tools.

use super::{Application, Workspace};
use rusix::{
    interop::{
        NixExpression, NixPath, Package,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// A packaged operation used by Qubes services, bootstrap or integration checks.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    /// Apply a validator-approved Airlock config candidate from dom0.
    Apply,
    /// Validate a staged Airlock config candidate and archive it into sys-config-vault.
    Archive,
    /// Bootstrap a Qubes VM kernel and TemplateVM on a Airlock dom0.
    BootstrapAssets,
    /// Bootstrap sys-config-staging from the current sys-config-vault baseline.
    BootstrapCurrent,
    /// Validate a Airlock nix-closure domain through a real Qubes VM lifecycle.
    ClosureSmoke,
    /// Run a Airlock smoke test for sys-config-vault, sys-config-staging, and sys-config-validator.
    ConfigSmoke,
    /// Dom0 qrexec control entry point for Airlock config generations.
    Control,
    /// Run a Qubes GUI window smoke test on a Airlock dom0.
    GuiSmoke,
    /// Import Airlock config inputs into closure-backed config VMs.
    ImportInputs,
    /// Run the Airlock qrexec smoke test on a booted Qubes dom0.
    QrexecSmoke,
    /// Set up Airlock's declared Qubes config topology on dom0.
    Setup,
    /// Start declared Airlock root network-provider qubes after setup-config.
    StartProviders,
    /// Start a Qube and require exact-generation qrexec readiness.
    StartReady,
    /// Report Airlock config topology and apply status as JSON.
    Status,
    /// Verify dom0 has booted into the staged Airlock NixOS generation.
    VerifyBoot,
    /// Native dom0 service with Qubes storage and GPU bridges.
    Dom0,
}

/// One executable dependency, keeping Airlock operations distinct from upstream packages.
enum Dependency {
    /// Attribute path in the caller's package set.
    Package(&'static str),
    /// Another packaged Airlock operation.
    Tool(Tool),
    /// The native service without its Qubes wrapper, avoiding a recursive runtime closure.
    NativeDom0,
    /// Native CLI used to receive generated source without executing authoring code.
    NativeCli,
    /// Read-only smoke-test report processing with no Qubes control authority.
    TestReport,
}

/// Native command contract exported by the machine package catalogue.
struct Script {
    /// Executable name installed into the package's bin directory.
    name: &'static str,
    /// Human-readable purpose carried in package metadata.
    description: &'static str,
    /// Fixed native subcommand arguments; caller arguments follow these.
    arguments: &'static [&'static str],
    /// Commands added to the executable's PATH, in their declared order.
    dependencies: &'static [Dependency],
}

impl Tool {
    /// Every shell operation available to the machine's package exports.
    pub const ALL: &'static [Self] = &[
        Self::Apply,
        Self::Archive,
        Self::BootstrapAssets,
        Self::BootstrapCurrent,
        Self::ClosureSmoke,
        Self::ConfigSmoke,
        Self::Control,
        Self::GuiSmoke,
        Self::ImportInputs,
        Self::QrexecSmoke,
        Self::Setup,
        Self::StartProviders,
        Self::StartReady,
        Self::Status,
        Self::VerifyBoot,
        Self::Dom0,
    ];

    /// Stable package export name used by generated Nix consumers.
    pub fn export_name(self) -> &'static str {
        match self {
            Self::Apply => "airlock-config-apply",
            Self::Archive => "airlock-config-archive",
            Self::BootstrapAssets => "airlock-qube-bootstrap",
            Self::BootstrapCurrent => "airlock-config-bootstrap-current",
            Self::ClosureSmoke => "airlock-nix-closure-domain-smoke",
            Self::ConfigSmoke => "airlock-config-vms-smoke",
            Self::Control => "airlock-config-control",
            Self::GuiSmoke => "airlock-qube-gui-smoke",
            Self::ImportInputs => "airlock-config-inputs-import",
            Self::QrexecSmoke => "airlock-qubes-smoke",
            Self::Setup => "airlock-setup-config",
            Self::StartProviders => "airlock-start-network-providers",
            Self::StartReady => "airlock-start-qube-ready",
            Self::Status => "airlock-config-status",
            Self::VerifyBoot => "airlock-config-boot-verify",
            Self::Dom0 => "airlock-dom0-service",
        }
    }

    /// Select the script's implementation and dependency contract.
    fn script(self) -> Script {
        match self {
            Self::Apply => Script {
                name: "airlock-config-apply",
                description: "Apply a validator-approved Airlock config candidate from dom0",
                arguments: &["config", "apply"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Tool(Tool::Archive),
                    Dependency::NativeCli,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("git"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("jq"),
                    Dependency::Package("nix"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                    Dependency::Tool(Tool::Setup),
                    Dependency::Package("systemd"),
                ],
            },
            Self::Archive => Script {
                name: "airlock-config-archive-candidate",
                description: "Validate a staged Airlock config candidate and archive it into sys-config-vault",
                arguments: &["config", "archive"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("git"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("jq"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                    Dependency::Package("systemd"),
                ],
            },
            Self::BootstrapAssets => Script {
                name: "airlock-qube-bootstrap",
                description: "Bootstrap a Qubes VM kernel and TemplateVM on a Airlock dom0",
                arguments: &["config", "bootstrap-assets"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("curl"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnused"),
                    Dependency::Package("rpm"),
                    Dependency::Package("systemd"),
                ],
            },
            Self::BootstrapCurrent => Script {
                name: "airlock-config-bootstrap-current",
                description: "Bootstrap sys-config-staging from the current sys-config-vault baseline",
                arguments: &["config", "bootstrap-current"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("jq"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                ],
            },
            Self::ClosureSmoke => Script {
                name: "airlock-nix-closure-domain-smoke",
                description: "Validate a Airlock nix-closure domain through a real Qubes VM lifecycle",
                arguments: &["closure"],
                dependencies: &[
                    Dependency::Package("coreutils"),
                    Dependency::Package("e2fsprogs"),
                    Dependency::Package("erofs-utils"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnused"),
                    Dependency::Package("jq"),
                    Dependency::Tool(Tool::Apply),
                    Dependency::Tool(Tool::GuiSmoke),
                    Dependency::Tool(Tool::Dom0),
                    Dependency::Tool(Tool::Setup),
                    Dependency::TestReport,
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                    Dependency::Package("systemd"),
                    Dependency::Package("util-linux"),
                ],
            },
            Self::ConfigSmoke => Script {
                name: "airlock-config-vms-smoke",
                description: "Run a Airlock smoke test for sys-config-vault, sys-config-staging, \
                              and sys-config-validator",
                arguments: &["config"],
                dependencies: &[
                    Dependency::TestReport,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("jq"),
                    Dependency::Tool(Tool::Apply),
                    Dependency::Tool(Tool::ImportInputs),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                    Dependency::Package("systemd"),
                ],
            },
            Self::Control => Script {
                name: "airlock-config-control",
                description: "Dom0 qrexec control entry point for Airlock config generations",
                arguments: &["config", "control"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Tool(Tool::Apply),
                    Dependency::Tool(Tool::Archive),
                    Dependency::Tool(Tool::Status),
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnused"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("jq"),
                    Dependency::Package("nix"),
                ],
            },
            Self::GuiSmoke => Script {
                name: "airlock-qube-gui-smoke",
                description: "Run a Qubes GUI window smoke test on a Airlock dom0",
                arguments: &["--root", "@prefix@", "gui"],
                dependencies: &[
                    Dependency::TestReport,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnused"),
                    Dependency::Package("systemd"),
                    Dependency::Package("xdotool"),
                ],
            },
            Self::ImportInputs => Script {
                name: "airlock-config-inputs-import",
                description: "Import Airlock config inputs into closure-backed config VMs",
                arguments: &["config", "import-inputs"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("gzip"),
                    Dependency::Package("jq"),
                    Dependency::Package("nix"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                ],
            },
            Self::QrexecSmoke => Script {
                name: "airlock-qubes-smoke",
                description: "Run the Airlock qrexec smoke test on a booted Qubes dom0",
                arguments: &["--root", "@prefix@", "qubes"],
                dependencies: &[
                    Dependency::TestReport,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnused"),
                    Dependency::Package("systemd"),
                ],
            },
            Self::Setup => Script {
                name: "airlock-setup-config",
                description: "Set up Airlock's declared Qubes config topology on dom0",
                arguments: &["setup-config", "run", "--script", "@self@"],
                dependencies: &[
                    Dependency::Package("coreutils"),
                    Dependency::Package("diffutils"),
                    Dependency::Package("e2fsprogs"),
                    Dependency::Package("erofs-utils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("gnugrep"),
                    Dependency::Package("gnutar"),
                    Dependency::Package("gnused"),
                    Dependency::Package("iproute2"),
                    Dependency::Package("jq"),
                    Dependency::Package("lvm2"),
                    Dependency::Package("nix"),
                    Dependency::Tool(Tool::Dom0),
                    Dependency::Package("python3.pkgs.qubes-core-admin"),
                    Dependency::Package("python3.pkgs.qubes-desktop-linux-common"),
                    Dependency::Package("sudo"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("systemd"),
                    Dependency::Package("util-linux"),
                ],
            },
            Self::StartProviders => Script {
                name: "airlock-start-network-providers",
                description: "Start declared Airlock root network-provider qubes after setup-config",
                arguments: &["start-network-providers"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("jq"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Tool(Tool::StartReady),
                ],
            },
            Self::StartReady => Script {
                name: "airlock-start-qube-ready",
                description: "Start a Qube and require exact-generation qrexec readiness",
                arguments: &["start-qube-ready"],
                dependencies: &[
                    Dependency::Package("coreutils"),
                    Dependency::NativeDom0,
                    Dependency::Package("procps"),
                    Dependency::Package("systemd"),
                    Dependency::Package("xen"),
                    Dependency::Package("jq"),
                    Dependency::Package("python3.pkgs.qubes-core-admin-client"),
                    Dependency::Package("python3.pkgs.qubes-core-qrexec.dom0"),
                ],
            },
            Self::Status => Script {
                name: "airlock-config-status",
                description: "Report Airlock config topology and apply status as JSON",
                arguments: &["config", "status"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Package("coreutils"),
                    Dependency::Package("gawk"),
                    Dependency::Package("jq"),
                ],
            },
            Self::VerifyBoot => Script {
                name: "airlock-config-boot-verify",
                description: "Verify dom0 has booted into the staged Airlock NixOS generation",
                arguments: &["config", "boot-verify"],
                dependencies: &[
                    Dependency::NativeDom0,
                    Dependency::Tool(Tool::Status),
                    Dependency::Package("coreutils"),
                    Dependency::Package("jq"),
                ],
            },
            Self::Dom0 => unreachable!("the native service has a dedicated wrapper"),
        }
    }

    /// Construct a service package without evaluating or running its scripts in Rust.
    pub fn build(self, pkgs: &NixValue, workspace: &Workspace, machine: &NixPath) -> Package {
        let mut order = Vec::new();
        self.order(&mut order, &mut Vec::new());
        // An operation can need the same service through both PATH and its fixed
        // executable. Bind each dependency once instead of expanding its graph twice.
        Package::from_expression(
            bind_packages(
                &order,
                std::collections::BTreeMap::new(),
                pkgs,
                workspace,
                machine,
            )
            .select(self.export_name()),
        )
    }

    /// Export the operation graph with each shared package constructed once.
    /// Dependency ordering is chosen in Rust; packages stay lazy in the generated expression.
    pub fn packages(pkgs: &NixValue, workspace: &Workspace, machine: &NixPath) -> NixValue {
        let mut order = Vec::new();
        for tool in Self::ALL {
            tool.order(&mut order, &mut Vec::new());
        }

        bind_packages(
            &order,
            std::collections::BTreeMap::new(),
            pkgs,
            workspace,
            machine,
        )
    }

    /// Append dependencies before their consumer and reject accidental authoring cycles.
    fn order(self, ordered: &mut Vec<Self>, active: &mut Vec<Self>) {
        if ordered.contains(&self) {
            return;
        }

        assert!(
            !active.contains(&self),
            "cycle in packaged Airlock operations"
        );
        active.push(self);
        if self == Self::Dom0 {
            Self::StartReady.order(ordered, active);
        } else {
            for dependency in self.script().dependencies {
                if let Dependency::Tool(tool) = dependency {
                    tool.order(ordered, active);
                }
            }
        }

        active.pop();
        ordered.push(self);
    }

    /// Assemble one package using dependencies supplied by the enclosing graph.
    fn construct(
        self,
        pkgs: &NixValue,
        workspace: &Workspace,
        _machine: &NixPath,
        resolve: &impl Fn(Self) -> Package,
    ) -> Package {
        if matches!(self, Self::Dom0) {
            return dom0(pkgs, workspace, resolve(Self::StartReady));
        }

        let script = self.script();
        let dependencies = script
            .dependencies
            .iter()
            .map(|dependency| match dependency {
                Dependency::Package(path) => pkgs.clone().select(path),
                Dependency::Tool(tool) => resolve(*tool).as_expression(),
                Dependency::NativeDom0 => Application::Dom0.build(pkgs, workspace).as_expression(),
                Dependency::NativeCli => Application::Cli.build(pkgs, workspace).as_expression(),
                Dependency::TestReport => Application::TestReport
                    .build(pkgs, workspace)
                    .as_expression(),
            });
        let smoke = matches!(
            self,
            Self::QrexecSmoke | Self::GuiSmoke | Self::ConfigSmoke | Self::ClosureSmoke
        );
        let binary = if smoke {
            "airlock-smoke"
        } else {
            "airlock-dom0-service"
        };
        let native = if smoke {
            Application::TestReport.build(pkgs, workspace)
        } else if self == Self::Setup {
            resolve(Self::Dom0)
        } else {
            Application::Dom0.build(pkgs, workspace)
        };
        let path = pkgs
            .clone()
            .select("lib.makeBinPath")
            .call(NixValue::list(dependencies));
        let prefix = if self == Self::Setup {
            nix_text!("/run/wrappers/bin:{path}", path = path).into()
        } else {
            path
        };
        let mut fields = vec![
            (
                "program",
                nix_text!("{native}/bin/{binary}", native = native, binary = binary).into(),
            ),
            (
                "program_env",
                if smoke {
                    "AIRLOCK_SMOKE_BIN".into()
                } else {
                    "AIRLOCK_DOM0_SERVICE".into()
                },
            ),
            (
                "args",
                NixValue::list(script.arguments.iter().map(|argument| (*argument).into())),
            ),
            ("prefix_env", nix_record! { "PATH": prefix }),
        ];
        if matches!(self, Self::ConfigSmoke | Self::ClosureSmoke) {
            fields.push(("env", nix_record! { "AIRLOCK_CONFIG_SMOKE_SOURCE": workspace.file("airlock-config/examples/candidate_fixture.rs").snapshot() }));
        }
        let package =
            crate::native::package(pkgs, workspace, script.name, NixValue::record(fields));
        Package::from_expression(package.as_expression().select("overrideAttrs").call(
            NixValue::function(|_| {
                nix_record! {
                    "meta": nix_record! {
                        "description": script.description,
                        "mainProgram": script.name,
                    },
                }
            }),
        ))
    }
}

/// Bind shared dependencies once before constructing their consumers.
fn bind_packages(
    remaining: &[Tool],
    mut available: std::collections::BTreeMap<Tool, Package>,
    pkgs: &NixValue,
    workspace: &Workspace,
    machine: &NixPath,
) -> NixValue {
    let Some((tool, remaining)) = remaining.split_first() else {
        return NixValue::record(
            available
                .into_iter()
                .map(|(tool, package)| (tool.export_name(), package.as_expression())),
        );
    };
    let package = tool.construct(pkgs, workspace, machine, &|dependency| {
        available[&dependency].clone()
    });

    package.bind(|package| {
        available.insert(*tool, package);
        bind_packages(remaining, available, pkgs, workspace, machine)
    })
}

/// Wrap native dom0 operations with only the Qubes integration executables they require.
fn dom0(pkgs: &NixValue, workspace: &Workspace, start_ready: Package) -> Package {
    let adapter = super::adapters::qubes(pkgs, workspace);
    let python = pkgs
        .clone()
        .select("python3.withPackages")
        .call(NixValue::function(|ps| {
            NixValue::list([
                ps.clone().select("qubes-core-admin"),
                ps.select("qubes-core-admin-client"),
                adapter.as_expression(),
            ])
        }));
    let native = Application::Dom0Qubes.build(pkgs, workspace);
    let mut dependencies = ["nix", "systemd", "procps", "xen"]
        .map(|path| pkgs.clone().select(path))
        .to_vec();
    dependencies.push(start_ready.as_expression());
    dependencies.extend(
        [
            "python3.pkgs.qubes-core-admin-client",
            "python3.pkgs.qubes-core-qrexec.dom0",
        ]
        .map(|path| pkgs.clone().select(path)),
    );

    Package::from_expression(pkgs.clone().select("writeShellApplication").call(nix_record! {
        "name": "airlock-dom0-service", "runtimeInputs": NixValue::list(dependencies),
        "text": nix_text!(r#"
            export PYTHONPATH={python}/{site_packages}
            export PYTHONNOUSERSITE=1
            export PYTHONSAFEPATH=1
            export AIRLOCK_DOM0_SERVICE={output}/bin/airlock-dom0-service
            exec {native}/bin/airlock-dom0-service "$@"
        "#, python = python, site_packages = pkgs.clone().select("python3.sitePackages"), output = NixValue::builtin("placeholder").call("out"), native = native),
        "meta": nix_record! {
            "description": "Airlock system services for dom0", "license": pkgs.clone().select("lib.licenses.mit"),
        },
    }))
}
