//! Build VM boot filesystems and manifests for Airlock's shared read-only Nix store.
//! Qubes supplies the kernel and writable private volume independently of these artifacts.

use crate::{Application, Workspace};
use rusix::{
    interop::{
        Package,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// Target packages and application sources used to construct a VM's boot artifacts.
pub struct Images<'a> {
    /// Package set for the target architecture, including statically linked early-boot tools.
    pub pkgs: &'a NixValue,
    /// Application sources used by the native storage-descriptor writer.
    pub workspace: &'a Workspace,
}

impl Images<'_> {
    /// Build a deterministic boot filesystem, sharing one identity across VMs of equal capacity.
    /// Capacity is in MiB. VM names never enter this recipe; Qubes owns writable VM identity.
    /// The separate `metadata` output contains the native service's root descriptor.
    pub fn root(&self, size_mib: NixValue) -> Package {
        let native = Application::PrepareBoot.build(self.pkgs, self.workspace);
        let script = nix_text!(
            r#"
            exec {native}/bin/airlock-prepare-boot root --out "$out" --metadata "$metadata" \
              --size-mib {size} --busybox {busybox}/bin/busybox \
              --init {init}/bin/airlock-vm-init --dom0 {dom0}/bin/airlock-dom0-service
        "#,
            native = native,
            size = size_mib,
            busybox = self.pkgs.clone().select("pkgsStatic.busybox"),
            init = Application::VmInit.build(self.pkgs, self.workspace),
            dom0 = Application::Dom0.build(self.pkgs, self.workspace)
        );

        Package::from_expression(
            self.pkgs
                .clone()
                .select("runCommand")
                .call("airlock-nix-closure-root-image")
                .call(nix_record! {
                    "outputs": NixValue::list(["out".into(), "metadata".into()]),
                    "nativeBuildInputs": self.packages(&[
                        "coreutils", "e2fsprogs", "fakeroot", "findutils",
                    ]),
                    "preferLocalBuild": true,
                    "allowSubstitutes": false,
                })
                .call(script),
        )
    }

    /// Record a closure's store paths without constructing or copying its shared store image.
    /// The runtime storage service prepares EROFS views from this admitted closure manifest.
    pub fn manifest(
        &self,
        name: NixValue,
        roots: NixValue,
        device: NixValue,
        mountpoint: NixValue,
    ) -> Package {
        self.closure_manifest(name, roots, device, mountpoint, false)
    }

    /// Verify installed boot commands against the exact VM closure before publishing its manifest.
    pub fn boot_manifest(
        &self,
        name: NixValue,
        roots: NixValue,
        device: NixValue,
        mountpoint: NixValue,
    ) -> Package {
        self.closure_manifest(name, roots, device, mountpoint, true)
    }

    /// Generic payload closures contain data roots; only boot closures contain NixOS systems to inspect.
    fn closure_manifest(
        &self,
        name: NixValue,
        roots: NixValue,
        device: NixValue,
        mountpoint: NixValue,
        boot: bool,
    ) -> Package {
        let closure = self
            .pkgs
            .clone()
            .select("closureInfo")
            .call(nix_record! { "rootPaths": roots.clone() });
        let root_paths = NixValue::builtin("map")
            .call(NixValue::builtin("toString"))
            .call(roots);
        let settings = self.pkgs.clone().select("writeText").call("closure-settings.json")
            .call(NixValue::builtin("toJSON").call(nix_record! {
                "name": name.clone(), "device": device, "mountpoint": mountpoint, "root_paths": root_paths,
            }));
        let preflight: NixValue = if boot {
            nix_text!(
                "--launcher {launcher}/bin/airlock-exec --readelf {binutils}/bin/readelf",
                launcher = Application::Launcher.build(self.pkgs, self.workspace),
                binutils = self.pkgs.clone().select("binutils"),
            )
            .into()
        } else {
            "".into()
        };
        let script = nix_text!(
            r#"
            exec {native}/bin/airlock-prepare-boot manifest --out "$out" \
              --paths {closure}/store-paths --settings {settings} \
              {preflight}
        "#,
            native = Application::PrepareBoot.build(self.pkgs, self.workspace),
            closure = closure,
            settings = settings,
            preflight = preflight,
        );

        Package::from_expression(
            self.pkgs
                .clone()
                .select("runCommand")
                .call(nix_text!("{name}-store-closure", name = name))
                .call(nix_record! {
                    "nativeBuildInputs": self.packages(&["coreutils", "jq"]),
                    "preferLocalBuild": true,
                    "allowSubstitutes": false,
                })
                .call(script),
        )
    }

    /// Select build executables from the same package set as the artifacts they produce.
    fn packages(&self, names: &[&str]) -> NixValue {
        NixValue::list(names.iter().map(|name| self.pkgs.clone().select(name)))
    }
}
