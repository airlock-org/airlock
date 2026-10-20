//! Wire OpenSSL into curl and Git, then wire curl into MariaDB.
//! The pinned nixpkgs package set supplies the other dependencies.
use rusix::interop::raw::NixRepresentation;
use rusix::interop::raw::NixpkgsExt;

#[path = "../openssl-nixpkg/mod.rs"]
pub mod openssl;

#[path = "../curl-nixpkg/mod.rs"]
pub mod curl;

#[path = "../git-nixpkg/mod.rs"]
pub mod git;

#[path = "../mariadb-nixpkg/mod.rs"]
pub mod mariadb;

use rusix::interop::{NixAttrs, NixExpression, Nixpkgs, Package, raw::NixValue};
use rusix::{IntoRusixValue, RusixValue, nix_record};

pub fn arguments() -> NixAttrs {
    NixAttrs::new([] as [(&str, NixValue); 0])
}

pub fn packages() -> NixAttrs<Package> {
    let pkgs = Nixpkgs::new();
    let openssl = pkgs.call_package(
        &openssl::factory(openssl::model::Release::Preview),
        arguments(),
    );
    with_openssl(openssl)
}

/// Accepting an ordinary package value keeps the replacement boundary movable.
pub fn with_openssl(openssl: Package) -> NixAttrs<Package> {
    compose(
        openssl,
        |pkgs, openssl| {
            pkgs.try_call_package(&curl::factory(), curl_arguments(pkgs, openssl.clone()))
                .expect("fixed authoring arguments")
        },
        mariadb::model::Release::V1011.arguments(),
    )
}

/// Match the normal pkgs.curl flavour selected by MariaDB, rather than curlMinimal.
pub fn curl_arguments(pkgs: &Nixpkgs, openssl: Package) -> CurlArguments {
    CurlArguments {
        openssl,
        extra: NixAttrs::choose(
            (!pkgs.value("stdenv.hostPlatform.isStatic")).into_expr::<bool>(),
            NixAttrs::new([("brotliSupport", true.into())]),
            arguments(),
        ),
    }
}

/// Typed dependency wiring with a deferred partial record for dynamic interop.
pub struct CurlArguments {
    /// Shared OpenSSL package used by curl's build recipe.
    openssl: Package,
    /// Platform defaults and caller overrides merged into curl's argument record.
    extra: NixAttrs,
}

impl CurlArguments {
    /// Additional overrides take precedence without reconstructing the dependency.
    pub fn with_overrides(mut self, overrides: NixAttrs) -> Self {
        self.extra = self.extra.merge(overrides);
        self
    }
}

impl IntoRusixValue for CurlArguments {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        RusixValue::leaf(
            NixAttrs::from_expression(nix_record! {
                "openssl": self.openssl,
                "idnSupport": true,
                "pslSupport": true,
                "zstdSupport": true,
            })
            .merge(self.extra),
        )
    }
}

/// Ordinary Rust callbacks allow replacing either side of the authoring boundary.
pub fn compose(
    openssl: Package,
    make_curl: impl FnOnce(&Nixpkgs, &Package) -> Package,
    mariadb_arguments: impl IntoRusixValue,
) -> NixAttrs<Package> {
    let pkgs = Nixpkgs::new();
    // Existing callbacks supply lazy lexical bindings, sharing each dependency once.
    openssl.bind(|openssl| {
        let curl = make_curl(&pkgs, &openssl);
        let git = pkgs
            .try_call_package(
                &git::factory(),
                git::arguments().with_openssl(openssl.clone()),
            )
            .expect("fixed authoring arguments");
        curl.bind(|curl| {
            let mariadb = pkgs.call_package(
                &mariadb::factory(),
                NixAttrs::try_from_record(mariadb_arguments)
                    .expect("fixed MariaDB arguments")
                    .merge(NixAttrs::new([("curl", curl.clone().into())])),
            );
            NixAttrs::new([
                ("openssl", openssl),
                ("curl", curl),
                ("git", git),
                ("mariadb", mariadb),
            ])
        })
    })
}
