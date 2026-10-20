//! Select Cargo inputs without copying every implementation into every package source.

use rusix::{
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record,
};

/// Cargo workspace sources supplied by the caller, including manifests and the shared lockfile.
#[derive(Clone)]
pub struct Workspace {
    /// Symbolic workspace root; generation never captures the developer's checkout path.
    root: NixPath,
}

/// Cargo members and their source directories, including libraries used only during authoring.
const MEMBERS: &[(&str, &str)] = &[
    ("airlock-config", "airlock-config"),
    ("airlock-build", "airlock-common/build"),
    ("airlock-qubes", "airlock-common/qubes"),
    ("airlock-qubes-python", "airlock-qubes-python"),
    ("airlock-dev", "airlock-dev"),
    ("airlock-cli", "airlock-cli"),
    ("airlock-image", "airlock-image"),
    ("airlock-tests", "airlock-tests"),
    ("airlock-dom0-service", "airlock-dom0/service"),
    ("airlock-deployment-core", "airlock-common/deployment"),
    (
        "airlock-machine-planning",
        "airlock-common/machine-planning",
    ),
    ("airlock-machine-core", "airlock-common/machine"),
    ("airlock-nix-core", "airlock-common/nix"),
    ("airlock-sys-ctrl-gui", "airlock-sys-ctrl/gui"),
    ("airlock-sys-ctrl-service", "airlock-sys-ctrl/service"),
    ("airlock-vm-service", "airlock-vm/service"),
    ("airlock-vm-init", "airlock-vm/init"),
    ("airlock-sys-gui-service", "airlock-sys-gui/service"),
    ("airlock-sys-gui-setup", "airlock-sys-gui/setup"),
];

impl Workspace {
    /// Retain an explicit deferred path to the application's Cargo workspace.
    pub fn new(root: NixPath) -> Self {
        Self {
            root: NixPath::from_source_text(root.as_expression().to_text().into_expr::<String>()),
        }
    }

    /// Return the workspace root for consumers that admit the complete application source.
    pub fn root(&self) -> NixPath {
        self.root.clone()
    }

    /// Resolve an implementation-owned asset without reading or snapshotting the whole tree.
    pub fn file(&self, relative: &str) -> NixPath {
        self.root
            .join_relative(relative)
            .expect("workspace asset path is relative")
    }

    /// Build a minimal source tree for a workspace package with all Cargo manifests present.
    /// Cargo resolves every manifest even when it builds only one selected package.
    /// Runtime Qubes adapters are packaged separately from their native executables.
    ///
    /// # Panics
    /// Panics for a package not declared in this workspace's source inventory.
    pub fn source(&self, lib: &NixValue, package: &str) -> NixValue {
        let directory = MEMBERS
            .iter()
            .find(|(name, _)| *name == package)
            .expect("package belongs to the Airlock workspace")
            .1;
        let file = |relative: &str| self.file(relative).as_expression();
        let unions = |values: Vec<NixValue>| {
            lib.clone()
                .select("fileset.unions")
                .call(NixValue::list(values))
        };
        let selected = file(&format!("{directory}/src"));
        let mut files = vec![
            file("Cargo.toml"),
            file("Cargo.lock"),
            file("airlock-common/nix/src"),
            file("airlock-common/machine-planning/src"),
            selected,
            file("vendor/rusix"),
        ];
        files.extend(
            MEMBERS
                .iter()
                .map(|(_, directory)| file(&format!("{directory}/Cargo.toml"))),
        );
        if matches!(
            package,
            "airlock-dom0-service"
                | "airlock-config"
                | "airlock-dev"
                | "airlock-machine-core"
                | "airlock-sys-gui-service"
                | "airlock-vm-service"
                | "airlock-sys-gui-setup"
                | "airlock-tests"
        ) {
            files.push(file("airlock-common/machine/src"));
            files.push(file("airlock-common/machine-planning/src"));
        }
        if matches!(
            package,
            "airlock-deployment-core"
                | "airlock-dom0-service"
                | "airlock-qubes-python"
                | "airlock-tests"
        ) {
            files.push(file("airlock-common/deployment/src"));
            files.push(file("airlock-common/machine-planning/src"));
        }
        if matches!(
            package,
            "airlock-sys-gui-service" | "airlock-sys-gui-setup" | "airlock-tests"
        ) {
            files.push(file("airlock-vm/service/src"));
        }
        if matches!(
            package,
            "airlock-build" | "airlock-config" | "airlock-dev" | "airlock-sys-gui-service"
        ) {
            files.push(file("airlock-common/build/src"));
            // The shared Qubes overlay embeds this upstream-facing fixture at compile time.
            files.push(file("airlock-tests/fixtures/memory_accounting.c"));
        }
        if matches!(package, "airlock-config" | "airlock-dev") {
            files.push(file("airlock-sys-gui/service/src"));
            files.push(file("airlock-config/src"));
            files.push(file("verus.json"));
        }
        if matches!(
            package,
            "airlock-dom0-service" | "airlock-image" | "airlock-qubes-python" | "airlock-tests"
        ) {
            files.push(file("airlock-common/qubes/src"));
        }
        if matches!(package, "airlock-qubes-python" | "airlock-tests") {
            files.push(file("airlock-dom0/service/src"));
            files.push(file("airlock-common/machine/src"));
        }
        if package == "airlock-sys-ctrl-gui" {
            files.extend([
                file("airlock-sys-ctrl/gui/assets"),
                file("airlock-sys-ctrl/gui/examples"),
            ]);
        }

        lib.clone().select("fileset.toSource").call(nix_record! {
            "root": self.root.clone(), "fileset": unions(files),
        })
    }
}
