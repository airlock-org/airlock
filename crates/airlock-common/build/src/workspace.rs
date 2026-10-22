//! Select Cargo inputs without copying every implementation into every package source.

use rusix::{
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record,
};

mod inputs;

/// Cargo workspace sources supplied by the caller, including manifests and the shared lockfile.
#[derive(Clone)]
pub struct Workspace {
    /// Symbolic workspace root; generation never captures the developer's checkout path.
    root: NixPath,
}

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

    /// Select sources for a package with its default Cargo features enabled.
    /// Keep implementation sources for its local normal/build dependencies and required embedded assets.
    /// Cargo resolution also needs the workspace manifests and vendored library discovery entry files.
    ///
    /// # Panics
    /// Panics under the same conditions as [`Self::source_with_features`].
    pub fn source(&self, lib: &NixValue, package: &str) -> NixValue {
        self.source_with_features(lib, package, true, &[])
    }

    /// Select a package's local implementation inputs using the same features as its Cargo build.
    /// Resolve optional and transitive local dependencies from embedded Cargo declarations without
    /// invoking Cargo or reading the supplied checkout. Development dependencies are omitted;
    /// target-specific dependencies are conservatively retained for cross builds.
    ///
    /// # Panics
    /// Panics for a package or local path dependency absent from the source inventory,
    /// or for a local dependency inherited from `[workspace.dependencies]`.
    pub fn source_with_features(
        &self,
        lib: &NixValue,
        package: &str,
        default_features: bool,
        features: &[&str],
    ) -> NixValue {
        let files = inputs::sources(package, default_features, features)
            .iter()
            .map(|relative| {
                let path = self.file(relative).as_expression();
                if relative.ends_with("/build.rs") {
                    lib.clone().select("fileset.maybeMissing").call(path)
                } else {
                    path
                }
            })
            .collect::<Vec<_>>();

        lib.clone().select("fileset.toSource").call(nix_record! {
            "root": self.root.clone(),
            "fileset": lib.clone().select("fileset.unions").call(NixValue::list(files)),
        })
    }
}
