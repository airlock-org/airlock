//! Describe OpenSSL build recipes for the pinned releases.
//! Nix evaluates them with the source fetchers and compiler environment supplied by nixpkgs.
use super::{
    inputs::{Inputs, args},
    model::Release,
    scripts,
};
use rusix::interop::raw::NixRepresentation;
use rusix::{
    Expr, IntoRusixValue,
    interop::{
        FinalAttrs, NixAttrs, NixCallable, NixExpression, NixLibrary, NixList, NixPath, Nixpkgs,
        Package, PackageFunction, raw::NixValue,
    },
    nix_record, nix_text,
};

/// Creates a deferred Nix package function for the chosen OpenSSL release.
/// Nix callers supply dependencies through `callPackage` and can override feature arguments.
pub fn factory(release: Release) -> PackageFunction<Package> {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        let defaults = defaults(&i);
        (defaults, common(&i, release))
    })
}

/// Creates a deferred Nix function returning all three releases under their package-set names.
/// Each release shares the caller's dependencies and feature arguments.
pub fn family_factory() -> PackageFunction<NixAttrs<Package>> {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        (
            defaults(&i),
            NixAttrs::new(Release::ALL.map(|release| (release.attribute(), common(&i, release)))),
        )
    })
}

fn defaults(i: &Inputs) -> Vec<(&'static str, NixValue)> {
    vec![
        ("withCryptodev", false.into()),
        ("withZlib", false.into()),
        ("enableSSL2", false.into()),
        ("enableSSL3", false.into()),
        ("enableMD2", false.into()),
        ("enableKTLS", i.stdenv().host_platform().is_linux().into()),
        ("static", i.stdenv().host_platform().is_static().into()),
        ("conf", NixValue::null()),
    ]
}

fn common(i: &Inputs, release: Release) -> Package {
    let lib = i.lib();
    let version: Expr<String> = release.version().into();
    let path = |file: &str| {
        Nixpkgs::new().source_path(&format!("pkgs/development/libraries/openssl/{file}"))
    };
    let mut patches = vec![path(match release {
        Release::Legacy => "1.1/nix-ssl-cert-file.patch",
        _ => "3.0/nix-ssl-cert-file.patch",
    })];
    if !matches!(release, Release::Legacy) {
        patches.push(path("3.0/openssl-disable-kernel-detection.patch"));
    }
    patches.push(NixPath::choose(
        i.stdenv().host_platform().is_darwin(),
        path(if matches!(release, Release::Preview) {
            "3.3/use-etc-ssl-certs-darwin.patch"
        } else {
            "use-etc-ssl-certs-darwin.patch"
        }),
        path(if matches!(release, Release::Preview) {
            "3.3/use-etc-ssl-certs.patch"
        } else {
            "use-etc-ssl-certs.patch"
        }),
    ));
    let extra_meta = match release {
        Release::Legacy => nix_record! {
            "knownVulnerabilities": NixValue::list([
                "OpenSSL 1.1 is reaching its end of life on 2023/09/11 and cannot be supported through the NixOS 23.11 release cycle. https://www.openssl.org/blog/blog/2023/03/28/1.1.1-EOL/".into(),
            ]),
        },
        _ => nix_record! { "license": i.lib().as_expression().select("licenses.asl20") },
    };
    // Upstream's version/hash are lexically captured by common, not finalAttrs.
    i.stdenv().mk_derivation(
        NixCallable::<NixAttrs, FinalAttrs>::try_from_function(|final_attrs: FinalAttrs| {
            attributes(
                i,
                &lib,
                version,
                release.hash().into(),
                NixList::new(patches),
                true.into(),
                extra_meta,
                final_attrs,
            )
        })
        .expect("fixed OpenSSL recipe"),
    )
}

/// The common recipe parameters mirror the upstream shared function, not a core schema.
#[allow(clippy::too_many_arguments)]
fn attributes(
    i: &Inputs,
    lib: &NixLibrary,
    version: Expr<String>,
    hash: Expr<String>,
    patches: NixList<NixPath>,
    with_docs: Expr<bool>,
    extra_meta: NixValue,
    final_attrs: FinalAttrs,
) -> Recipe {
    let modern = lib.version_at_least(version.clone(), "1.1.1");
    let v3 = lib.version_at_least(version.clone(), "3.0.0");
    let old = lib.version_older(version.clone(), "3.0");
    let fixed = version.as_expression().replace_text([(".", "_")]);
    let source = i.fetchurl().call(nix_record! {
        "url": NixValue::if_else(
            old,
            nix_text!(
                "https://github.com/openssl/openssl/releases/download/OpenSSL_{fixed}/openssl-{version}.tar.gz",
                fixed = fixed,
                version = version.clone(),
            ),
            nix_text!(
                "https://github.com/openssl/openssl/releases/download/openssl-{version}/openssl-{version}.tar.gz",
                version = version.clone(),
            ),
        ),
        "hash": hash,
    });
    let flags = NixList::concat([
        NixList::<Expr<String>>::new([
            "shared".into(),
            "--libdir=lib".into(),
            Expr::choose(
                i.static_build(),
                "--openssldir=/.$(etc)/etc/ssl".into(),
                "--openssldir=etc/ssl".into(),
            ),
        ]),
        (NixList::<Expr<String>>::new([
            "-DHAVE_CRYPTODEV".into(),
            "-DUSE_CRYPTODEV_DIGESTS".into(),
        ]))
        .when(lib, i.with_cryptodev()),
        NixList::optional(lib, i.enable_md2(), "enable-md2".into()),
        NixList::optional(lib, i.enable_ssl2(), "enable-ssl2".into()),
        NixList::optional(lib, i.enable_ssl3(), "enable-ssl3".into()),
        NixList::optional(lib, (v3.clone()).and(i.enable_ktls()), "enable-ktls".into()),
        NixList::optional(
            lib,
            (modern.clone()).and(i.stdenv().host_platform().is_aarch64()),
            "no-afalgeng".into(),
        ),
        NixList::optional(
            lib,
            (modern.clone()).and(i.static_build()),
            "no-shared".into(),
        ),
        NixList::optional(lib, (v3).and(i.static_build()), "no-module".into()),
        NixList::optional(lib, i.static_build(), "no-ct".into()),
        NixList::optional(lib, i.with_zlib(), "zlib".into()),
        NixList::optional(
            lib,
            i.stdenv().host_platform().is_open_bsd(),
            "no-devcryptoeng".into(),
        ),
        (NixList::<Expr<String>>::new([nix_text!(
            "CFLAGS=-march={arch}",
            arch = i.stdenv().host_platform().gcc.arch()
        )]))
        .when(
            lib,
            (i.stdenv().host_platform().is_mips()).and(
                (i.stdenv()
                    .as_expression()
                    .select("hostPlatform")
                    .has_attr("gcc")
                    .into_expr::<bool>())
                .and(i.stdenv().host_platform().gcc.as_attrs().has("arch")),
            ),
        ),
    ]);
    Recipe {
        pname: "openssl",
        version: version.clone(),
        src: source,
        patches,
        post_patch: Expr::concat([
            scripts::patch_configure(),
            lib.optional_text(!modern.clone(), scripts::patch_old_tests()),
            lib.optional_text(
                modern.clone(),
                scripts::patch_env(Package::from_expression(
                    i.build_packages().select("coreutils"),
                )),
            ),
            lib.optional_text(
                (modern).and(i.stdenv().host_platform().is_musl()),
                scripts::patch_musl(),
            ),
            lib.optional_text(i.static_build(), scripts::patch_static_engines()),
        ]),
        outputs: NixList::concat([
            NixList::<Expr<String>>::new(["bin".into(), "dev".into(), "out".into(), "man".into()]),
            NixList::optional(lib, with_docs, "doc".into()),
            NixList::optional(lib, i.static_build(), "etc".into()),
        ]),
        set_output_flags: false,
        separate_debug_info: (!i.stdenv().host_platform().is_darwin()).and(
            (!i.stdenv()
                .as_expression()
                .select("hostPlatform")
                .attr_or("useLLVM", false)
                .into_expr::<bool>())
            .and(
                i.stdenv()
                    .as_expression()
                    .select("cc.isGNU")
                    .into_expr::<bool>(),
            ),
        ),
        native_build_inputs: NixList::concat([
            NixList::optional(
                lib,
                !i.stdenv().host_platform().is_windows(),
                i.make_binary_wrapper(),
            ),
            NixList::new([i.perl()]),
            (NixList::new([i.remove_references_to()])).when(lib, i.static_build()),
        ]),
        build_inputs: NixList::concat([
            NixList::optional(lib, i.with_cryptodev(), i.cryptodev()),
            NixList::optional(lib, i.with_zlib(), i.zlib()),
        ]),
        configure_platforms: Vec::<Expr<String>>::new(),
        configure_script: configure_script(i, lib, &version),
        dont_add_static_configure_flags: true,
        configure_flags: flags,
        make_flags: vec!["MANDIR=$(man)/share/man", "MANSUFFIX=ssl"],
        enable_parallel_building: true,
        post_install: Expr::concat([
            Expr::choose(
                i.static_build(),
                scripts::install_static(),
                scripts::install_shared(),
            ),
            scripts::install_bin(),
            lib.optional_text(
                !i.stdenv().host_platform().is_windows(),
                scripts::install_rehash(),
            ),
            scripts::install_dev(),
            lib.optional_text(
                !i.conf().is_null(),
                scripts::install_conf(i.conf().as_expression()),
            ),
        ]),
        post_fixup: Expr::concat([
            lib.optional_text(
                !i.stdenv().host_platform().is_windows(),
                scripts::fixup_perl(Package::from_expression(i.build_packages().select("perl"))),
            ),
            lib.optional_text(
                lib.version_at_least(version.clone(), "3.3.0"),
                scripts::fixup_cmake(),
            ),
        ]),
        passthru: nix_record! {
            "tests": nix_record! {
                "pkg-config": i.testers()
                    .select("testMetaPkgConfig")
                    .call(final_attrs.final_package()),
            },
        },
        meta: nix_record! {
            "homepage": "https://www.openssl.org/",
            "changelog": nix_text!(
                "https://github.com/openssl/openssl/blob/openssl-{version}/CHANGES.md",
                version = version,
            ),
            "description": "Cryptographic library that implements the SSL and TLS protocols",
            "license": i.lib().as_expression().select("licenses.openssl"),
            "mainProgram": "openssl",
            "maintainers": NixValue::concat_lists([
                NixValue::list([i.lib().as_expression().select("maintainers.thillux")]),
                i.lib().as_expression().select("teams.stridtech.members"),
            ]),
            "pkgConfigModules": NixValue::list([
                "libcrypto".into(), "libssl".into(), "openssl".into(),
            ]),
            "platforms": i.lib().as_expression().select("platforms.all"),
        }
        .merge_attrs(extra_meta),
    }
}

fn configure_script(i: &Inputs, lib: &NixLibrary, version: &Expr<String>) -> Expr<String> {
    let bits = i.stdenv().host_platform().parsed.cpu.bits();
    let bsd = NixValue::if_else(
        i.stdenv().host_platform().is_x86_64(),
        "./Configure BSD-x86_64",
        NixValue::if_else(
            i.stdenv().host_platform().is_x86_32(),
            nix_text!(
                "./Configure BSD-x86{elf}",
                elf = lib.optional_text(i.stdenv().host_platform().is_elf(), "-elf")
            ),
            nix_text!("./Configure BSD-generic{bits}", bits = bits.clone()),
        ),
    );
    let linux = NixValue::if_else(
        i.stdenv().host_platform().is_x86_64(),
        "./Configure linux-x86_64",
        NixValue::if_else(
            i.stdenv().host_platform().is_micro_blaze(),
            "./Configure linux-latomic",
            NixValue::if_else(
                i.stdenv().host_platform().is_mips32(),
                "./Configure linux-mips32",
                NixValue::if_else(
                    i.stdenv().host_platform().is_mips64n32(),
                    "./Configure linux-mips64",
                    NixValue::if_else(
                        i.stdenv().host_platform().is_mips64n64(),
                        "./Configure linux64-mips64",
                        nix_text!("./Configure linux-generic{bits}", bits = bits.clone()),
                    ),
                ),
            ),
        ),
    );
    let fallback = NixValue::if_else(
        i.stdenv().build_host_equal(),
        "./config",
        NixValue::if_else(
            i.stdenv().host_platform().is_bsd(),
            bsd,
            NixValue::if_else(
                i.stdenv().host_platform().is_min_gw(),
                nix_text!(
                    "./Configure mingw{bits}",
                    bits = lib.optional_text(
                        !bits.as_expression().equals(32_i64).into_expr::<bool>(),
                        bits.clone().to_text()
                    )
                ),
                NixValue::if_else(
                    i.stdenv().host_platform().is_linux(),
                    linux,
                    NixValue::if_else(
                        i.stdenv().host_platform().is_ios(),
                        nix_text!("./Configure ios{bits}-cross", bits = bits),
                        NixValue::builtin("throw").call(nix_text!(
                            "Not sure what configuration to use for {config}",
                            config = i.stdenv().host_platform().config()
                        )),
                    ),
                ),
            ),
        ),
    );
    let mut targets: Vec<(&str, NixValue)> = [
        ("armv5tel-linux", "./Configure linux-armv4 -march=armv5te"),
        ("armv6l-linux", "./Configure linux-armv4 -march=armv6"),
        ("armv7l-linux", "./Configure linux-armv4 -march=armv7-a"),
        ("x86_64-darwin", "./Configure darwin64-x86_64-cc"),
        ("aarch64-darwin", "./Configure darwin64-arm64-cc"),
        ("x86_64-linux", "./Configure linux-x86_64"),
        ("x86_64-solaris", "./Configure solaris64-x86_64-gcc"),
        ("powerpc64-linux", "./Configure linux-ppc64"),
        ("riscv64-linux", "./Configure linux64-riscv64"),
    ]
    .into_iter()
    .map(|(k, v)| (k, v.into()))
    .collect();
    targets.push((
        "riscv32-linux",
        NixValue::if_else(
            lib.version_at_least(version.clone(), "3.2"),
            "./Configure linux32-riscv32",
            "./Configure linux-latomic",
        ),
    ));
    // Dynamic attribute lookup/fallback is appropriate NixValue interop; no platform schema.
    NixValue::record(targets)
        .attr_or(i.stdenv().host_platform().system(), fallback)
        .into_expr::<String>()
}

// A Nix derivation describes how to build a package. These fields become the attributes
// passed to nixpkgs' standard builder; Rust constructs the recipe without running its phases.
#[derive(IntoRusixValue)]
struct Recipe {
    // Package name without its version.
    pname: &'static str,
    // Source release selected in Rust and emitted as a Nix string expression.
    version: Expr<String>,
    // Archive-fetching recipe; its output is not downloaded during source generation.
    src: Package,
    // Checked-in patch files applied before configuration, in this order.
    patches: NixList<NixPath>,
    // Shell text run by the builder after applying patches.
    post_patch: Expr<String>,
    // Separate store outputs for executables, headers, libraries, manuals and optional docs/config.
    outputs: NixList<Expr<String>>,
    // Disables standard output-directory flags because OpenSSL supplies its own.
    set_output_flags: bool,
    // Platform-dependent choice to put debugging symbols in a separate output.
    separate_debug_info: Expr<bool>,
    // Tools that execute on the build machine, including when cross-compiling.
    native_build_inputs: NixList<Package>,
    // Libraries used by OpenSSL on the host platform, where the resulting package runs.
    build_inputs: NixList<Package>,
    // Platforms for generic --build/--host/--target flags; empty for OpenSSL's custom Configure.
    configure_platforms: Vec<Expr<String>>,
    // Configure command selected for the platform and release during Nix evaluation.
    configure_script: Expr<String>,
    // Suppresses generic static-build flags in favour of OpenSSL-specific ones.
    dont_add_static_configure_flags: bool,
    // OpenSSL feature switches and installation paths passed to Configure.
    configure_flags: NixList<Expr<String>>,
    // Manual-page location and suffix supplied to make.
    make_flags: Vec<&'static str>,
    // Allows the builder to run make jobs concurrently.
    enable_parallel_building: bool,
    // Shell text that splits installed files among the declared outputs.
    post_install: Expr<String>,
    // Shell text that rejects unwanted Perl references and removes CMake helper files.
    post_fixup: Expr<String>,
    // Extra package attributes, including checks that refer to the final overridden package.
    passthru: NixValue,
    // Descriptive package information, supported platforms and release-specific restrictions.
    meta: NixValue,
}
