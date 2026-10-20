//! Describes the dependencies and feature switches accepted by the pinned Git Nix function.
//! Rust accessors generate references; Nix resolves their values only when the recipe needs them.
use rusix::interop::raw::NixpkgsExt;
use rusix::{
    IntoRusixValue, RusixValue,
    interop::{NixPath, Nixpkgs, Package, raw::NixValue},
};

/// Declares named accessors for the dependencies and features passed to Git's Nix function.
/// The views describe lookups; Nix supplies their values when applying the function.
#[rusix::args]
pub(super) mod args {
    use rusix::{
        Expr,
        interop::{NixCallable, NixList, Package, raw::NixValue},
    };

    /// Resolved arguments stay deferred; accessors record each lookup's Rust call site.
    #[rusix(root)]
    struct Inputs {
        /// Source fetcher; Nix constructs the fixed-output derivation.
        fetchurl: NixCallable<Package>,
        /// Patch fetcher with nixpkgs normalization semantics.
        fetchpatch: NixCallable<Package>,
        /// Caller-supplied library functions and package metadata.
        lib: Lib,
        /// Builder and platform properties supplied by the caller.
        stdenv: Stdenv,
        /// Tools that run on the build platform, including cross builds.
        build_packages: BuildPackages,
        /// HTTP transport library used by Git.
        curl: Package,
        /// TLS and cryptography library used by Git.
        openssl: Package,
        /// Compression library used for Git objects.
        zlib: Package,
        /// XML parser used by Git's HTTP transport.
        expat: Package,
        /// Interpreter for optional Python helper programs.
        python3: Package,
        /// Translation tools and runtime paths used by Git's shell helpers.
        gettext: Package,
        /// Archive tool used during the build.
        cpio: Package,
        /// Search tool whose store path is embedded in installed Git helpers.
        gnugrep: Package,
        /// Text-editing tool whose store path is embedded in installed Git helpers.
        gnused: Package,
        /// AWK interpreter whose store path is embedded in installed Git helpers.
        gawk: Package,
        /// Basic command-line tools whose store paths are embedded in installed helpers.
        coreutils: Package,
        /// SSH client embedded in invocation paths when withSsh is enabled.
        openssh: Package,
        /// Regular-expression library selected when withpcre2 is enabled.
        pcre2: Package,
        /// Shell dependency used by Git's build and scripts.
        bash: Package,
        /// Documentation generator used when withManual is enabled.
        asciidoc: Package,
        /// Documentation tools used when withManual is enabled.
        texinfo: Package,
        /// XML conversion tools used when withManual is enabled.
        xmlto: Package,
        /// DocBook-to-Texinfo converter used when withManual is enabled.
        docbook2x: Package,
        /// DocBook stylesheets used to generate the manual.
        #[rusix(rename = "docbook_xsl")]
        docbook_xsl: Package,
        /// DocBook document definitions used to generate the manual.
        #[rusix(rename = "docbook_xml_dtd_45")]
        docbook_xml_dtd_45: Package,
        /// XML transformation library and tools used by the recipe.
        libxslt: Package,
        /// Tcl interpreter used by the optional GUI programs.
        tcl: Package,
        /// Tk toolkit whose wish executable runs the optional GUI programs.
        tk: Package,
        /// Build hook that sets interpreter and library paths for installed helpers.
        make_wrapper: Package,
        /// Character-encoding library used on platforms other than FreeBSD.
        libiconv: Package,
        /// Character-encoding library selected on FreeBSD.
        libiconv_real: Package,
        /// Perl interpreter, library-path helper and gitweb dependencies.
        perl_packages: PerlPackages,
        /// Enables git-svn and its Perl bindings; requires perlSupport.
        svn_support: bool,
        /// Enables Perl helpers; defaults to equal build and host platforms in Nix.
        perl_support: bool,
        /// Enables translated messages; defaults to true in Nix.
        nls_support: bool,
        /// Enables the Apple keychain credential helper; defaults to true on macOS only.
        osxkeychain_support: bool,
        /// Enables the Tcl/Tk GUI programs; defaults to false in Nix.
        gui_support: bool,
        /// Builds and installs manual and HTML documentation; defaults to true in Nix.
        with_manual: bool,
        /// Enables Python helper programs; defaults to true in Nix.
        python_support: bool,
        /// Enables PCRE2 regular expressions; defaults to true in Nix.
        withpcre2: bool,
        /// Enables git-send-email; defaults to perlSupport and requires Perl.
        send_email_support: bool,
        /// Installs the libsecret credential helper; defaults to false in Nix.
        with_libsecret: bool,
        /// Embeds the supplied OpenSSH client in Git's SSH paths; defaults to false in Nix.
        with_ssh: bool,
        /// Runs tests against installed Git during a build; defaults to false on macOS.
        do_install_check: bool,
        /// Subversion client whose Perl bindings are selected for git-svn.
        subversion_client: Package,
        /// Perl libraries placed on the search path of installed Perl helpers.
        perl_libs: NixList<Package>,
        /// SMTP and TLS Perl libraries placed on git-send-email's search path.
        smtp_perl_libs: NixList<Package>,
        /// Apple Security framework linked by the macOS keychain helper.
        #[rusix(rename = "Security")]
        security: Package,
        /// Apple CoreServices framework included on macOS.
        #[rusix(rename = "CoreServices")]
        core_services: Package,
        /// Existing NixOS integration tests exposed on the package for separate use.
        nixos_tests: NixosTests,
        /// Build tool that locates dependency headers and libraries.
        #[rusix(rename = "pkg-config")]
        pkg_config: Package,
        /// GLib library included when the libsecret credential helper is enabled.
        glib: Package,
        /// Secret-storage library used by the optional credential helper.
        libsecret: Package,
        /// Compression tool whose store path is embedded in gitweb.cgi.
        gzip: Package,
        /// System-information tool used by installed tests on macOS and FreeBSD.
        sysctl: Package,
        /// Build tool that makes platform detection use the host when cross-compiling.
        #[rusix(rename = "deterministic-host-uname")]
        deterministic_host_uname: Package,
        /// Existing fetchgit package tests merged with Git's own package checks.
        tests: Tests,
    }

    /// The caller's standard builder and its finite platform/tool dependencies.
    /// Whole-subtree access retains the entire supplied stdenv for build/host equality.
    #[rusix(value)]
    struct Stdenv {
        /// Existing mkDerivation function; Rusix does not reimplement the builder.
        mk_derivation: NixCallable<Package>,
        /// Platform where Git will run.
        #[rusix(expression)]
        host_platform: rusix::interop::Platform,
        /// Compiler metadata needed for the glibc linker workaround.
        cc: Compiler,
        /// Shell executable selected by stdenv.
        shell: String,
        /// Shell package excluded from cross-built runtime references.
        shell_package: Package,
    }

    /// The compiler property used by the pinned Git recipe.
    struct Compiler {
        /// Indicates whether the selected compiler is GNU GCC.
        #[rusix(rename = "isGNU")]
        is_gnu: bool,
    }

    /// Only the build-platform interpreter is needed from this package set.
    struct BuildPackages {
        /// Interpreter used for documentation and installed checks in cross builds.
        perl: Package,
    }

    /// Perl package categories remain opaque; only the needed children are declared.
    struct PerlPackages {
        /// Interpreter and the library-directory metadata needed for wrapper scripts.
        perl: Perl,
        /// Existing helper that formats Perl package library paths.
        make_perl_path: NixCallable<Expr<String>>,
        /// CGI dependencies used when wrapping gitweb.
        #[rusix(rename = "CGI")]
        cgi: Package,
        /// HTML parser used by gitweb.
        #[rusix(rename = "HTMLParser")]
        html_parser: Package,
        /// FastCGI library used by gitweb.
        #[rusix(rename = "CGIFast")]
        cgi_fast: Package,
        /// FastCGI bindings used by gitweb.
        #[rusix(rename = "FCGI")]
        fcgi: Package,
        /// FastCGI process manager used by gitweb.
        #[rusix(rename = "FCGIProcManager")]
        fcgi_proc_manager: Package,
        /// Tag-cloud library used by gitweb.
        #[rusix(rename = "HTMLTagCloud")]
        html_tag_cloud: Package,
    }

    /// The Perl derivation remains opaque when used as a dependency or store path.
    #[rusix(value)]
    struct Perl {
        /// Relative library directory used by installed Perl wrappers.
        lib_prefix: String,
    }

    /// Nix library dispatch stays dynamic; metadata dependencies are declared explicitly.
    #[rusix(value)]
    struct Lib {
        /// License metadata included in the package.
        licenses: Licenses,
        /// Supported-platform metadata included in the package.
        platforms: Platforms,
        /// Maintainer records included in the package.
        maintainers: Maintainers,
    }

    /// The pinned Git license record.
    struct Licenses {
        /// GPL version 2 metadata from the caller's Nix library.
        gpl2: NixValue,
    }

    /// The supported-platform list from the caller's Nix library.
    struct Platforms {
        /// All platforms listed by nixpkgs, retained exactly as upstream.
        all: NixValue,
    }

    /// The pinned package's maintainers; these records remain opaque.
    struct Maintainers {
        /// Maintainer metadata from nixpkgs.
        primeos: NixValue,
        /// Maintainer metadata from nixpkgs.
        wmertens: NixValue,
        /// Maintainer metadata from nixpkgs.
        globin: NixValue,
        /// Maintainer metadata from nixpkgs.
        kashw2: NixValue,
    }

    /// NixOS integration test referenced by Git's passthru.
    struct NixosTests {
        /// Existing buildbot test; not executed by this example.
        buildbot: NixValue,
    }

    /// Package test record used by Git's passthru.
    struct Tests {
        /// Existing fetchgit tests merged with Git's own tests.
        fetchgit: NixValue,
    }
}

pub(super) use args::Inputs;

impl Inputs {
    /// Refer to an asset in the pinned Git source directory, such as a patch or updater.
    /// Returns a Nix path without reading the file or fetching anything.
    #[track_caller]
    pub(super) fn file(&self, name: &str) -> NixPath {
        Nixpkgs::new().source_path(&format!("pkgs/applications/version-management/git/{name}"))
    }
}

/// Supplies Git's explicit dependencies while callPackage selects the remaining scope.
pub(super) fn arguments(model: super::model::Git) -> Arguments {
    Arguments {
        model,
        openssl: None,
    }
}

/// Named authoring inputs retain the model and supplied dependency until lowering.
pub struct Arguments {
    /// Rust feature choices converted into explicit arguments to the Git package function.
    model: super::model::Git,
    /// Optional OpenSSL dependency; None keeps nixpkgs' automatic callPackage lookup.
    openssl: Option<Package>,
}

impl Arguments {
    /// Supplies the OpenSSL dependency explicitly, replacing Nix's automatic package lookup.
    /// The package remains deferred; this method neither builds it nor inspects its contents.
    pub fn with_openssl(mut self, openssl: Package) -> Self {
        self.openssl = Some(openssl);
        self
    }
}

impl IntoRusixValue for Arguments {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        let mut fields = self.model.arguments();

        // These explicit arguments reproduce Git's callPackage wiring in all-packages.nix.
        // Other dependencies are selected by callPackage, including cross-build splicing.
        let pkgs = Nixpkgs::new();
        fields.extend([
            (
                "Security",
                pkgs.value("darwin.apple_sdk.frameworks.Security"),
            ),
            (
                "CoreServices",
                pkgs.value("darwin.apple_sdk.frameworks.CoreServices"),
            ),
            (
                "perlLibs",
                NixValue::list(
                    ["LWP", "URI", "TermReadKey"]
                        .map(|p| pkgs.get(&format!("perlPackages.{p}")).into()),
                ),
            ),
            (
                "smtpPerlLibs",
                NixValue::list(
                    [
                        "libnet",
                        "NetSMTPSSL",
                        "IOSocketSSL",
                        "NetSSLeay",
                        "AuthenSASL",
                        "DigestHMAC",
                    ]
                    .map(|p| pkgs.get(&format!("perlPackages.{p}")).into()),
                ),
            ),
        ]);

        if let Some(openssl) = self.openssl {
            fields.push(("openssl", openssl.into()));
        }
        RusixValue::leaf(NixValue::record(fields))
    }
}
