//! Package the pinned upstream prover for an explicitly entered workstation shell.

use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Pin the release archive independently of nixpkgs' Verus version.
pub(super) fn package(pkgs: &NixValue) -> NixValue {
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../../verus.json")).expect("checked-in Verus pin");
    pkgs.clone().select("stdenvNoCC.mkDerivation").call(nix_record! {
        "pname": "airlock-verus",
        "version": pin["version"].as_str().unwrap(),
        "src": pkgs.clone().select("fetchurl").call(nix_record! {
            "url": pin["url"].as_str().unwrap(),
            "sha256": pin["sha256"].as_str().unwrap(),
        }),
        "nativeBuildInputs": NixValue::list([
            pkgs.clone().select("unzip"), pkgs.clone().select("autoPatchelfHook"),
        ]),
        "buildInputs": NixValue::list([
            pkgs.clone().select("stdenv.cc.cc.lib"), pkgs.clone().select("zlib"),
        ]),
        // The launcher obtains this library from the exact rustup sysroot at runtime.
        "autoPatchelfIgnoreMissingDeps": NixValue::list(["librustc_driver-*.so".into()]),
        "dontBuild": true,
        "dontStrip": true,
        "installPhase": r#"
            runHook preInstall
            mkdir -p "$out/libexec/verus"
            cp verus cargo-verus rust_verify z3 version.json vstd.vir libvstd.rlib libverus_*.rlib libverus_*.so "$out/libexec/verus/"
            runHook postInstall
        "#,
    })
}

/// Supply Linux runtime libraries and the pinned prover without changing deployed systems.
pub(super) fn shell(pkgs: &NixValue) -> NixValue {
    pkgs.clone().select("mkShell").call(nix_record! {
        "nativeBuildInputs": NixValue::list([
            pkgs.clone().select("rustup"), pkgs.clone().select("gcc"),
            pkgs.clone().select("gnumake"), pkgs.clone().select("pkg-config"),
        ]),
        "AIRLOCK_VERUS_ROOT": nix_text!("{verus}/libexec/verus", verus = package(pkgs)),
    })
}
