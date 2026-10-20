//! Package upstream executables whose integration is maintained by Airlock.

use rusix::{
    interop::{
        Package,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// Build the pinned Codex distribution with its complete upstream helper layout.
pub fn codex(pkgs: &NixValue) -> Package {
    let recipe = NixValue::function(|final_attrs| {
        let version = final_attrs.select("version");
        let lib = pkgs.clone().select("lib");
        let source = pkgs.clone().select("fetchurl").call(nix_record! {
            "url": nix_text!(
                "https://github.com/openai/codex/releases/download/\
                 rust-v{version}/codex-package-x86_64-unknown-linux-musl.tar.gz",
                version = version.clone(),
            ),
            "hash": "sha256-pnb1ciquDYbL43ZDMvCOqx/didwKMAs61P+/SGEfo+M=",
        });

        nix_record! {
            "pname": "codex", "version": "0.162.1", "src": source, "sourceRoot": ".",
            "nativeBuildInputs": NixValue::list([pkgs.clone().select("autoPatchelfHook")]),
            "buildInputs": NixValue::list(
                ["stdenv.cc.cc.lib", "alsa-lib", "ncurses"].map(|name| pkgs.clone().select(name)),
            ),
            "dontBuild": true, "dontStrip": true,
            "installPhase": nix_text!(r#"
            runHook preInstall
            mkdir -p "$out"
            cp -a bin codex-package.json codex-path codex-resources "$out/"
            runHook postInstall
        "#),
            // Patch dynamically linked voice helpers while retaining the static CLI's upstream bytes.
            "doInstallCheck": true,
            "installCheckPhase": nix_text!(r#"
            runHook preInstallCheck
            test "$($out/bin/codex --version)" = "codex-cli {version}"
            "$out/bin/codex" --help > /dev/null
            test -x "$out/bin/codex-code-mode-host"
            test -x "$out/codex-path/rg"
            test -x "$out/codex-resources/bwrap"
            "$out/codex-resources/zsh/bin/zsh" --version
            "$out/codex-path/rg" --version > /dev/null
            "$out/codex-resources/bwrap" --version
            runHook postInstallCheck
        "#, version = version),
            "meta": nix_record! {
                "description": "OpenAI Codex CLI with its complete upstream Linux runtime package",
                "homepage": "https://developers.openai.com/codex/cli/",
                "license": lib.clone().select("licenses.asl20"),
                "mainProgram": "codex", "platforms": NixValue::list(["x86_64-linux".into()]),
                "sourceProvenance": NixValue::list([lib.select("sourceTypes.binaryNativeCode")]),
            },
        }
    });

    Package::from_expression(pkgs.clone().select("stdenvNoCC.mkDerivation").call(recipe))
}

/// Build upstream Qubes input transport without its distribution-specific activation rules.
pub fn input_proxy(pkgs: &NixValue) -> Package {
    let source = pkgs.clone().select("fetchFromGitHub").call(nix_record! {
        "owner": "QubesOS", "repo": "qubes-app-linux-input-proxy",
        "rev": "ea8710166c6ac29dd4183cdf155bc9a1da34cf4a",
        "hash": "sha256-4yfdBcdAlahKnV76htdOojWgordZyM0H7JfvXzn8owE=",
    });

    Package::from_expression(
        pkgs.clone()
            .select("stdenv.mkDerivation")
            .call(nix_record! {
                "pname": "qubes-input-proxy", "version": "1.0.46", "src": source,
                "buildPhase": nix_text!(r#"
            runHook preBuild
            make -C src
            runHook postBuild
        "#),
                "installPhase": nix_text!(r#"
            runHook preInstall
            install -Dm755 src/input-proxy-sender "$out/bin/input-proxy-sender"
            install -Dm755 src/input-proxy-receiver "$out/bin/input-proxy-receiver"
            for service in qubes.InputKeyboard qubes.InputMouse qubes.InputTablet; do
              install -Dm755 "qubes-rpc/$service" "$out/etc/qubes-rpc/$service"
              substituteInPlace "$out/etc/qubes-rpc/$service" \
                --replace-fail /usr/bin/input-proxy-receiver "$out/bin/input-proxy-receiver"
            done
            runHook postInstall
        "#),
                "meta": nix_record! {
                    "description": "Upstream Qubes evdev/uinput event proxy",
                    "homepage": "https://github.com/QubesOS/qubes-app-linux-input-proxy",
                    "license": pkgs.clone().select("lib.licenses.gpl2Plus"),
                    "platforms": pkgs.clone().select("lib.platforms.linux"),
                },
            }),
    )
}

/// Package read-only host observations independently of the dom0 control service.
pub fn observation(pkgs: &NixValue) -> Package {
    let workspace = crate::Workspace::new(rusix::interop::NixPath::from_source_text(
        pkgs.clone()
            .select("airlockAppsSource")
            .to_text()
            .into_expr::<String>(),
    ));
    let native = crate::Application::Observation.build(pkgs, &workspace);
    crate::native::package(
        pkgs,
        &workspace,
        "airlock-observe-dom0",
        nix_record! {
            "program": nix_text!("{native}/bin/airlock-observe-dom0", native = native),
            "prefix_env": nix_record! { "PATH": pkgs.clone().select("lib.makeBinPath").call(NixValue::list(
                ["coreutils", "inetutils", "systemd"].map(|name| pkgs.clone().select(name)),
            )) },
        },
    )
}
