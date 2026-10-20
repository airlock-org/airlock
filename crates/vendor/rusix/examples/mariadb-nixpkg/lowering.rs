//! Describe MariaDB client and server build recipes for the pinned releases.
//! Nix evaluates the recipes using the CMake tools and build environment supplied by nixpkgs.
use super::{
    inputs::{Inputs, args},
    model::Release,
    scripts,
};
use rusix::interop::raw::NixRepresentation;
use rusix::{
    Expr, IntoRusixValue,
    interop::{
        NixAttrs, NixExpression, NixLibrary, NixList, NixPath, Nixpkgs, Package, PackageFunction,
        raw::NixValue,
    },
    nix_record, nix_text,
};

/// Creates a deferred Nix function for a server package with separate `client` and `server` members.
/// Callers supply a version and source checksum; `callPackage` supplies other dependencies.
pub fn factory() -> PackageFunction<Package> {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |arguments| {
        let i = args::from_value(arguments);
        let defaults = defaults();
        let lib = i.lib();
        let body = common_view::Common::try_bind_record(common(&i, &lib), |common| {
            let client = client(&i, &lib, &common);
            let server = server(&i, &lib, &common);
            server.bind(|server| {
                server.extend(NixAttrs::new([
                    ("client", client.into()),
                    ("server", server.clone().into()),
                ]))
            })
        })
        .expect("fixed common MariaDB attributes");
        (defaults, body)
    })
}

/// Describes all four pinned releases under their package-set names, sharing one recipe.
/// Each value is a deferred server package with separate `client` and `server` members.
pub fn family() -> NixAttrs<Package> {
    factory().bind(|factory| {
        NixAttrs::new(Release::ALL.map(|release| {
            // The lexical reference remains a PackageFunction<Package>.
            let selected = PackageFunction::from_function_attrs(
                args::argument_names().iter().copied(),
                |arguments| (defaults(), factory.call(arguments)),
            );
            (
                release.attribute(),
                Nixpkgs::new()
                    .try_call_package(&selected, release.arguments())
                    .expect("fixed authoring arguments"),
            )
        }))
    })
}

fn defaults() -> Vec<(&'static str, NixValue)> {
    vec![
        ("withStorageMroonga", true.into()),
        ("withStorageRocks", true.into()),
        ("withEmbedded", false.into()),
        ("withNuma", false.into()),
    ]
}

fn file(name: &str) -> NixPath {
    Nixpkgs::new().source_path(&format!("pkgs/servers/sql/mariadb/patch/{name}"))
}

fn common(i: &Inputs, lib: &NixLibrary) -> Common {
    let native = NixList::concat([
        NixList::new([i.cmake(), i.pkg_config()]),
        NixList::optional(
            lib,
            i.stdenv().host_platform().is_darwin(),
            i.fix_darwin_dylib_names(),
        ),
        NixList::optional(
            lib,
            !i.stdenv().host_platform().is_darwin(),
            i.make_wrapper(),
        ),
    ]);
    let inputs = NixList::concat([
        NixList::new([
            i.libiconv(),
            i.ncurses(),
            i.zlib(),
            i.pcre2(),
            i.openssl(),
            i.curl(),
        ]),
        (NixList::concat([
            NixList::new([i.libkrb5(), i.systemd()]),
            NixList::choose(
                lib.version_older(i.version(), "10.6"),
                NixList::new([i.libaio()]),
                NixList::new([i.liburing()]),
            ),
        ]))
        .when(lib, i.stdenv().host_platform().is_linux()),
        (NixList::new([i.core_services(), i.cctools(), i.perl(), i.libedit()]))
            .when(lib, i.stdenv().host_platform().is_darwin()),
        (NixList::new([i.jemalloc()])).when(lib, !i.stdenv().host_platform().is_darwin()),
    ]);
    let cmake_flags = NixList::concat([
        NixList::<Expr<String>>::new(
            [
                "-DBUILD_CONFIG=mysql_release",
                "-DMANUFACTURER=nixos.org",
                "-DDEFAULT_CHARSET=utf8mb4",
                "-DDEFAULT_COLLATION=utf8mb4_unicode_ci",
                "-DSECURITY_HARDENED=ON",
                "-DINSTALL_UNIX_ADDRDIR=/run/mysqld/mysqld.sock",
                "-DINSTALL_BINDIR=bin",
                "-DINSTALL_DOCDIR=share/doc/mysql",
                "-DINSTALL_DOCREADMEDIR=share/doc/mysql",
                "-DINSTALL_INCLUDEDIR=include/mysql",
                "-DINSTALL_LIBDIR=lib",
                "-DINSTALL_PLUGINDIR=lib/mysql/plugin",
                "-DINSTALL_INFODIR=share/mysql/docs",
                "-DINSTALL_MANDIR=share/man",
                "-DINSTALL_MYSQLSHAREDIR=share/mysql",
                "-DINSTALL_SCRIPTDIR=bin",
                "-DINSTALL_SUPPORTFILESDIR=share/doc/mysql",
                "-DINSTALL_MYSQLTESTDIR=OFF",
                "-DINSTALL_SQLBENCHDIR=OFF",
                "-DINSTALL_PAMDIR=share/pam/lib/security",
                "-DINSTALL_PAMDATADIR=share/pam/etc/security",
                "-DWITH_ZLIB=system",
                "-DWITH_SSL=system",
                "-DWITH_PCRE=system",
                "-DWITH_SAFEMALLOC=OFF",
                "-DWITH_UNIT_TESTS=OFF",
                "-DEMBEDDED_LIBRARY=OFF",
            ]
            .map(Expr::from),
        ),
        (NixList::<Expr<String>>::new([
            "-DCONNECT_WITH_JDBC=OFF".into(),
            nix_text!(
                "-DCURSES_LIBRARY={ncurses}/lib/libncurses.dylib",
                ncurses = i.ncurses().output("out")
            ),
        ]))
        .when(lib, i.stdenv().host_platform().is_darwin()),
        (NixList::<Expr<String>>::new(["-Dhave_C__Wl___as_needed=".into()])).when(
            lib,
            (i.stdenv().host_platform().is_darwin()).and(lib.version_at_least(i.version(), "10.6")),
        ),
        (NixList::<Expr<String>>::new([
            "-DSTACK_DIRECTION=-1".into(),
            nix_text!(
                "-DCMAKE_CROSSCOMPILING_EMULATOR={emulator}",
                emulator = i
                    .stdenv()
                    .host_platform()
                    .emulator()
                    .call(i.build_packages())
            ),
        ]))
        .when(lib, !i.stdenv().build_host_equal()),
    ]);
    let test_version = nix_text!(
        "mariadb_{version}",
        version = i
            .lib()
            .as_expression()
            .select("versions.majorMinor")
            .call(i.version())
            .replace_text([(".", "")])
    );
    let tests = NixValue::function(|version| {
        NixValue::record(
            [
                "mariadb-galera-rsync",
                "mysql",
                "mysql-autobackup",
                "mysql-backup",
                "mysql-replication",
            ]
            .map(|name| {
                let scope = if name == "mariadb-galera-rsync" {
                    "mariadb-galera"
                } else {
                    name
                };
                (
                    name,
                    NixValue::builtin("getAttr")
                        .apply([version.clone(), i.nixos_tests().select(scope)]),
                )
            }),
        )
    })
    .call(test_version);
    Common {
        version: i.version(),
        src: i.fetchurl().call(nix_record! {
            "url": nix_text!(
                "https://archive.mariadb.org/mariadb-{version}/source/mariadb-{version}.tar.gz",
                version = i.version()
            ),
            "hash": i.hash(),
        }),
        outputs: vec!["out", "man"],
        native_build_inputs: native,
        build_inputs: inputs,
        pre_patch: scripts::pre_patch(),
        patches: NixList::concat([
            NixList::new([file("cmake-includedir.patch")]),
            lib.optional(
                (!i.stdenv().host_platform().is_linux())
                    .and(lib.version_at_least(i.version(), "10.6")),
                file("macos-MDEV-26769-regression-fix.patch"),
            ),
        ]),
        cmake_flags,
        post_install: lib.optional_text(!i.with_embedded(), scripts::post_install_common()),
        post_fixup: lib.optional_text(
            !i.stdenv().host_platform().is_darwin(),
            scripts::post_fixup(
                i.lib()
                    .as_expression()
                    .select("makeBinPath")
                    .call(NixList::new([i.less(), i.ncurses()]))
                    .into_expr(),
            ),
        ),
        passthru: nix_record! { "tests": tests },
        meta: nix_record! {
            "description": "Enhanced, drop-in replacement for MySQL",
            "homepage": "https://mariadb.org/",
            "license": i.lib().as_expression().select("licenses.gpl2Plus"),
            "maintainers": NixValue::concat_lists([
                NixValue::list([i.lib().as_expression().select("maintainers.thoughtpolice")]),
                i.lib().as_expression().select("teams.helsinki-systems.members"),
            ]),
            "platforms": i.lib().as_expression().select("platforms.all"),
        },
    }
}

fn client(i: &Inputs, lib: &NixLibrary, common: &common_view::Common) -> Package {
    i.stdenv().mk_derivation(
        common
            .as_attrs()
            .merge(NixAttrs::from_expression(nix_record! {
                "pname": "mariadb-client",
                "patches": NixList::concat([
                    common.patches(),
                    NixList::new([file("cmake-plugin-includedir.patch")]),
                ]),
                "buildInputs": NixList::concat([
                    common.build_inputs(),
                    (NixList::new([i.fmt_8()])).when(lib, lib.version_at_least(common.version(), "10.7")),
                ]),
                "cmakeFlags": NixList::concat([
                    common.cmake_flags(),
                    NixList::<Expr<String>>::new([
                        "-DPLUGIN_AUTH_PAM=NO".into(),
                        "-DWITHOUT_SERVER=ON".into(),
                        "-DWITH_WSREP=OFF".into(),
                        "-DINSTALL_MYSQLSHAREDIR=share/mysql-client".into(),
                    ]),
                ]),
                "postInstall": Expr::concat([
                    common.post_install(),
                    scripts::post_install_client(i.stdenv().host_platform().extensions.shared_library()),
                ]),
            })),
    )
}

fn server(i: &Inputs, lib: &NixLibrary, common: &common_view::Common) -> Package {
    // Perl's withPackages and its package scope remain backend values, excluded on Darwin.
    let mytop = Package::from_expression(i.build_packages().select("perl.withPackages").call(
        NixValue::function(|p| {
            NixValue::list([
                p.clone().select("DBDmysql"),
                p.clone().select("DBI"),
                p.select("TermReadKey"),
            ])
        }),
    ));
    let inputs = NixList::concat([
        common.build_inputs(),
        NixList::new([
            i.bzip2(),
            i.lz4(),
            i.lzo(),
            i.snappy(),
            i.xz(),
            i.zstd(),
            i.cracklib(),
            i.judy(),
            i.libevent(),
            i.libxml2(),
        ]),
        NixList::optional(lib, i.with_numa(), i.numactl()),
        (NixList::new([i.linux_pam()])).when(lib, i.stdenv().host_platform().is_linux()),
        NixList::optional(lib, !i.stdenv().host_platform().is_darwin(), mytop),
        (NixList::new([i.kytea(), i.libsodium(), i.msgpack(), i.zeromq()]))
            .when(lib, i.with_storage_mroonga()),
        (NixList::new([i.fmt_8()])).when(lib, lib.version_at_least(common.version(), "10.7")),
    ]);
    let flags = NixList::concat([
        common.cmake_flags(),
        NixList::<Expr<String>>::new([
            "-DMYSQL_DATADIR=/var/lib/mysql".into(),
            "-DENABLED_LOCAL_INFILE=OFF".into(),
            "-DWITH_READLINE=ON".into(),
            "-DWITH_EXTRA_CHARSETS=all".into(),
            nix_text!(
                "-DWITH_EMBEDDED_SERVER={enabled}",
                enabled = NixValue::if_else(i.with_embedded(), "ON", "OFF")
            ),
            "-DWITH_UNIT_TESTS=OFF".into(),
            "-DWITH_WSREP=ON".into(),
            "-DWITH_INNODB_DISALLOW_WRITES=ON".into(),
            "-DWITHOUT_EXAMPLE=1".into(),
            "-DWITHOUT_FEDERATED=1".into(),
            "-DWITHOUT_TOKUDB=1".into(),
        ]),
        (NixList::<Expr<String>>::new(["-DWITH_NUMA=ON".into()])).when(lib, i.with_numa()),
        (NixList::<Expr<String>>::new(["-DWITHOUT_MROONGA=1".into()]))
            .when(lib, !i.with_storage_mroonga()),
        (NixList::<Expr<String>>::new(["-DWITHOUT_ROCKSDB=1".into()]))
            .when(lib, !i.with_storage_rocks()),
        (NixList::<Expr<String>>::new(["-DWITH_ROCKSDB_JEMALLOC=ON".into()])).when(
            lib,
            (!i.stdenv().host_platform().is_darwin()).and(i.with_storage_rocks()),
        ),
        (NixList::<Expr<String>>::new(["-DWITH_JEMALLOC=yes".into()]))
            .when(lib, !i.stdenv().host_platform().is_darwin()),
        (NixList::<Expr<String>>::new([
            "-DPLUGIN_AUTH_PAM=NO".into(),
            "-DPLUGIN_AUTH_PAM_V1=NO".into(),
            "-DWITHOUT_OQGRAPH=1".into(),
            "-DWITHOUT_PLUGIN_S3=1".into(),
        ]))
        .when(lib, i.stdenv().host_platform().is_darwin()),
    ]);
    i.stdenv().mk_derivation(
        common
            .as_attrs()
            .merge(NixAttrs::from_expression(nix_record! {
                "pname": "mariadb-server",
                "nativeBuildInputs": NixList::concat([
                    common.native_build_inputs(),
                    NixList::new([i.bison(), i.boost().output("dev"), i.flex()]),
                ]),
                "buildInputs": inputs,
                "propagatedBuildInputs": lib.optional(i.with_numa(), i.numactl()),
                "postPatch": scripts::post_patch_server(),
                "cmakeFlags": flags,
                "preConfigure":
                    lib.optional_text(!i.stdenv().host_platform().is_darwin(), scripts::pre_configure()),
                "postInstall": Expr::concat([
                    common.post_install(),
                    scripts::post_install_server(),
                    lib.optional_text(i.with_storage_mroonga(), scripts::install_mroonga()),
                    lib.optional_text(
                        (!i.stdenv().host_platform().is_darwin()).and(lib.version_at_least(common.version(), "10.4")),
                        scripts::install_pam(),
                    ),
                ]),
                "CXXFLAGS": lib.optional_text(i.stdenv().host_platform().is_i686(), "-fpermissive"),
            })),
    )
}

// Both derivations start with this recipe. Nix's standard builder runs the phase scripts
// when building a package; creating this Rust record only describes those steps.
#[derive(IntoRusixValue)]
struct Common {
    // Source release shared by client and server.
    version: rusix::Expr<String>,
    // Archive-fetching recipe with the caller's URL version and expected checksum.
    src: Package,
    // Separate store outputs for installed files and manual pages.
    outputs: Vec<&'static str>,
    // Tools that execute on the build machine, including when cross-compiling.
    native_build_inputs: NixList<Package>,
    // Libraries used on the host platform, where the resulting MariaDB package runs.
    build_inputs: NixList<Package>,
    // Shell text run before patching to remove upstream log-directory assumptions.
    pre_patch: Expr<String>,
    // Checked-in patch files shared by both recipes, applied in order.
    patches: NixList<NixPath>,
    // Shared CMake switches and platform-dependent configuration choices.
    cmake_flags: NixList<rusix::Expr<String>>,
    // Shell text that removes development files unless the embedded server is requested.
    post_install: Expr<String>,
    // Shell text that supplies the mytop monitoring script's program search path.
    post_fixup: Expr<String>,
    // Existing NixOS tests exposed as package attributes for separate use.
    passthru: NixValue,
    // Package description, license, maintainers and supported platforms.
    meta: NixValue,
}

/// Declares accessors for recipe fields shared by MariaDB's client and server variants.
/// The views retain the complete shared recipe so each variant can extend it in Nix.
#[rusix::args]
mod common_view {
    use rusix::interop::{NixList, NixPath, Package};

    /// Arguments carrying the recipe shared by both MariaDB package variants.
    #[rusix(root)]
    struct Inputs {
        /// Shared recipe fields bound once and reused by the MariaDB client and server packages.
        common: Common,
    }

    /// Shared build choices extended separately for the client and server packages.
    #[rusix(value)]
    struct Common {
        /// Shared source version referenced by both deferred build recipes.
        version: String,
        /// Ordered patch-file paths shared by the client and server builds.
        patches: NixList<NixPath>,
        /// Shared libraries used where the resulting MariaDB programs run.
        build_inputs: NixList<Package>,
        /// Shared tools that execute on the build machine, including during cross-compilation.
        native_build_inputs: NixList<Package>,
        /// Shared CMake switches extended with client- or server-specific choices.
        cmake_flags: NixList<rusix::Expr<String>>,
        /// Shared shell commands to run after installation during a Nix build.
        post_install: String,
    }
}
