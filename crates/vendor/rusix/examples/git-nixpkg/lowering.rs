//! Reproduces the pinned Git package recipe using real nixpkgs builders and helpers.
//! Helpers construct deferred values and shell scripts; they never execute build commands in Rust.
use super::inputs::{Inputs, args};
use rusix::interop::raw::NixRepresentation;
use rusix::{
    Expr, IntoRusixValue,
    interop::{
        FinalAttrs, NixAttrs, NixCallable, NixExpression, NixLibrary, NixList, Package,
        PackageFunction, raw::NixValue,
    },
    nix_record, nix_text, package,
};

/// Creates a native Nix package function compatible with `callPackage` and argument overrides.
pub fn factory() -> PackageFunction<Package> {
    PackageFunction::from_function_attrs(args::argument_names().iter().copied(), |args| {
        let inputs = args::from_value(args);
        let defaults = vec![
            ("svnSupport", false.into()),
            (
                "perlSupport",
                package::build_host_equal(inputs.stdenv.as_value()).into(),
            ),
            ("nlsSupport", true.into()),
            (
                "osxkeychainSupport",
                inputs.stdenv.host_platform().is_darwin().into(),
            ),
            ("guiSupport", false.into()),
            ("withManual", true.into()),
            ("pythonSupport", true.into()),
            ("withpcre2", true.into()),
            ("sendEmailSupport", inputs.perl_support().into()),
            ("withLibsecret", false.into()),
            ("withSsh", false.into()),
            (
                "doInstallCheck",
                (!inputs.stdenv.host_platform().is_darwin()).into(),
            ),
        ];

        // Keep the public Nix interface's assertions even though the Rust model
        // already rules out the Perl-dependent invalid combinations. Implication
        // stays native, independent of any caller-supplied library predicates.
        let derivation = inputs.stdenv.mk_derivation().call(
            NixCallable::<NixAttrs, FinalAttrs>::try_from_function(|final_attrs: FinalAttrs| {
                attributes(&inputs, final_attrs)
            })
            .expect("fixed Git recipe"),
        );
        let checks = [
            inputs
                .osxkeychain_support()
                .implies(inputs.stdenv.host_platform().is_darwin()),
            inputs.send_email_support().implies(inputs.perl_support()),
            inputs.svn_support().implies(inputs.perl_support()),
        ];

        // Build guards inside out so Nix checks them in source order when the
        // package is demanded. The first failure leaves later checks unused.
        let body = checks
            .into_iter()
            .rev()
            .fold(derivation, |body, condition| body.asserted(condition));

        (defaults, body)
    })
}

// Fixed recipe fields retain their Rust interfaces and deferred Nix semantics.
#[derive(IntoRusixValue)]
struct Derivation {
    /// Feature-sensitive package name, including the upstream minimal/SVN suffixes.
    pname: Expr<String>,
    /// Release pinned by this compatibility implementation.
    version: &'static str,
    /// Fixed-output fetcher derivation, evaluated without fetching its contents.
    src: Package,
    /// Documentation is a separate output when enabled.
    outputs: NixList<Expr<String>>,
    /// Leaves debug information in stdenv's separate debug output.
    separate_debug_info: bool,
    /// Preserves upstream's format hardening exception.
    hardening_disable: Vec<&'static str>,
    /// Enables the standard builder's parallel make behavior.
    enable_parallel_building: bool,
    /// Pinned local patches and the conditional Darwin GUI fix.
    patches: NixValue,
    /// Applies gettext and optional SSH path substitutions.
    post_patch: Expr<String>,
    /// Tools selected for the build platform by nixpkgs.
    native_build_inputs: NixList<Package>,
    /// Host dependencies selected by the enabled features and platform.
    build_inputs: NixList<Package>,
    /// Platform-specific linker options retain their standard external name.
    #[rusix(rename = "NIX_LDFLAGS")]
    nix_ldflags: Expr<String>,
    /// Configure cache entries include cross-compilation answers.
    configure_flags: NixList<Expr<String>>,
    /// Determines the installed Perl library directory.
    pre_build: Expr<String>,
    /// Git's upstream build switches and interpreter paths.
    make_flags: NixList<Expr<String>>,
    /// Cross builds must not retain the build shell package.
    disallowed_references: NixList<Package>,
    /// Builds auxiliary Git programs and optional credential helpers.
    post_build: Expr<String>,
    /// Avoids hardlinks during installation.
    install_flags: Vec<&'static str>,
    /// Prepares credential-helper symlinks.
    pre_install: Expr<String>,
    /// Installs, patches, and wraps programs exactly as the pinned recipe does.
    post_install: Expr<String>,
    /// Upstream runs tests against the installed output instead of the build tree.
    do_check: bool,
    /// Retains the caller's platform-sensitive installed-test choice.
    do_install_check: Expr<bool>,
    /// Make target used for installed tests.
    install_check_target: &'static str,
    /// Uses the build-platform Perl interpreter for tests.
    install_check_flags: NixList<Expr<String>>,
    /// Supplies sysctl on systems whose tests require it.
    native_install_check_inputs: NixList<Package>,
    /// Excludes the same unsupported and flaky tests as upstream.
    pre_install_check: Expr<String>,
    /// Additional directories inspected by stdenv's debug stripping hook.
    strip_debug_list: Vec<&'static str>,
    /// Non-build attributes preserve tests, the shell path, and update script.
    passthru: NixValue,
    /// Standard nixpkgs package metadata remains authoritative in Nix.
    meta: NixValue,
}

/// Assemble the builder attributes from deferred feature and platform inputs.
/// The finalAttrs parameter preserves the install-check test's dependency on later overrides.
fn attributes(i: &Inputs, final_attrs: FinalAttrs) -> Derivation {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    let minimal = [
        i.svn_support(),
        i.gui_support(),
        i.send_email_support(),
        i.with_manual(),
        i.python_support(),
        i.withpcre2(),
    ]
    .into_iter()
    .fold(Expr::boolean(true), |result, condition| {
        result.and(!condition)
    });
    let svn = i.subversion_client().override_arguments(nix_record! {
        "perlBindings": i.perl_support(),
    });
    let perl = Package::from_expression(i.perl_packages.perl.as_expression());
    let patches = lib.concat_lists([
        NixList::new(
            [
                "docbook2texi.patch",
                "git-sh-i18n.patch",
                "git-send-email-honor-PATH.patch",
                "installCheck-path.patch",
            ]
            .map(|p| i.file(p)),
        ).into(),
        lib.optional(i.with_ssh(), i.file("ssh-path.patch")).into(),
        lib.optional(
            i.gui_support().and(i.stdenv.host_platform().is_darwin()),
            i.fetchpatch().call(nix_record! {
                "name": "gitk_check_main_window_visibility_before_waiting_for_it_to_show.patch",
                "url": "https://github.com/git/git/commit/1db62e44b7ec93b6654271ef34065b31496cd02e.patch",
                "hash": "sha256-ntvnrYFFsJ1Ebzc6vM9/AMFLHMS1THts73PIOG5DkQo=",
            }),
        ).into(),
    ].map(NixList::<NixValue>::from_expression));

    let native_build_inputs = NixList::concat_with(
        &lib,
        [
            NixList::new([
                i.deterministic_host_uname(),
                i.gettext(),
                Package::from_expression(i.perl_packages.perl.as_expression()),
                i.make_wrapper(),
                i.pkg_config(),
            ]),
            (NixList::new([
                i.asciidoc(),
                i.texinfo(),
                i.xmlto(),
                i.docbook2x(),
                i.docbook_xsl(),
                i.docbook_xml_dtd_45(),
                i.libxslt(),
            ]))
            .when(&lib, i.with_manual()),
        ],
    );
    let build_inputs = NixList::concat_with(
        &lib,
        [
            NixList::new([
                i.curl(),
                i.openssl(),
                i.zlib(),
                i.expat(),
                i.cpio(),
                Package::choose(
                    i.stdenv.host_platform().is_free_bsd(),
                    i.libiconv_real(),
                    i.libiconv(),
                ),
                i.bash(),
            ]),
            NixList::optional(&lib, i.perl_support(), perl.clone()),
            (NixList::new([i.tcl(), i.tk()])).when(&lib, i.gui_support()),
            NixList::optional(&lib, i.withpcre2(), i.pcre2()),
            (NixList::new([i.security(), i.core_services()]))
                .when(&lib, i.stdenv.host_platform().is_darwin()),
            (NixList::new([i.glib(), i.libsecret()])).when(&lib, i.with_libsecret()),
        ],
    );

    let make_flags = NixList::concat_with(
        &lib,
        [
            NixList::<Expr<String>>::new(["prefix=${out}".into()]),
            NixList::optional(
                &lib,
                package::build_host_equal(i.stdenv.as_value()),
                nix_text!("SHELL_PATH={shell}", shell = i.stdenv.shell()),
            ),
            NixList::choose(
                i.perl_support(),
                NixList::<Expr<String>>::new([nix_text!(
                    "PERL_PATH={perl}/bin/perl",
                    perl = perl.clone()
                )]),
                NixList::<Expr<String>>::new(["NO_PERL=1".into()]),
            ),
            NixList::choose(
                i.python_support(),
                NixList::<Expr<String>>::new([nix_text!(
                    "PYTHON_PATH={python}/bin/python",
                    python = i.python3()
                )]),
                NixList::<Expr<String>>::new(["NO_PYTHON=1".into()]),
            ),
            (NixList::<Expr<String>>::new(
                ["INSTALL=install", "NO_INET_NTOP=", "NO_INET_PTON="].map(Expr::from),
            ))
            .when(&lib, i.stdenv.host_platform().is_sun_os()),
            NixList::choose(
                i.stdenv.host_platform().is_darwin(),
                NixList::<Expr<String>>::new(["NO_APPLE_COMMON_CRYPTO=1".into()]),
                NixList::<Expr<String>>::new(["sysconfdir=/etc".into()]),
            ),
            (NixList::<Expr<String>>::new(
                ["NO_SYS_POLL_H=1", "NO_GETTEXT=YesPlease"].map(Expr::from),
            ))
            .when(&lib, i.stdenv.host_platform().is_musl()),
            NixList::optional(&lib, i.withpcre2(), "USE_LIBPCRE2=1".into()),
            NixList::optional(&lib, !i.nls_support(), "NO_GETTEXT=1".into()),
            NixList::optional(
                &lib,
                i.stdenv.host_platform().is_darwin(),
                "TKFRAMEWORK=/nonexistent".into(),
            ),
        ],
    );

    // This finite self-reference is evaluated by mkDerivation, not inspected in Rust.
    // overrideAttrs must see the eventual package, including later ordinary Nix overrides.
    let installed_test = final_attrs
        .final_package()
        .override_attrs(|_| NixAttrs::new([("doInstallCheck", true.into())]));
    let passthru_tests = nix_record! {
        "withInstallCheck": installed_test,
        "buildbot-integration": i.nixos_tests.buildbot(),
    }
    .merge_attrs(i.tests.fetchgit());

    Derivation {
        pname: Expr::concat([
            "git".into(),
            lib.optional_text(i.svn_support(), "-with-svn"),
            lib.optional_text(minimal, "-minimal"),
        ]),
        version: "2.47.0",
        src: i.fetchurl().call(nix_record! {
            "url": "https://www.kernel.org/pub/software/scm/git/git-2.47.0.tar.xz",
            "hash": "sha256-HOEU2ohwQnG0PgJ8UeBNk5n4yI6e91Qtrnrrrn2HvE4=",
        }),
        outputs: NixList::concat_with(
            &lib,
            [
                NixList::<Expr<String>>::new(["out".into()]),
                NixList::optional(&lib, i.with_manual(), "doc".into()),
            ],
        ),
        separate_debug_info: true,
        hardening_disable: vec!["format"],
        enable_parallel_building: true,
        patches: patches.into(),
        post_patch: post_patch(i),
        native_build_inputs,
        build_inputs,
        nix_ldflags: Expr::concat([
            lib.optional_text(
                i.stdenv.cc.is_gnu().and(
                    NixValue::from(i.stdenv.host_platform().libc())
                        .equals("glibc")
                        .into_expr::<bool>(),
                ),
                "-lgcc_s",
            ),
            lib.optional_text(i.stdenv.host_platform().is_free_bsd(), "-lthr"),
        ]),
        configure_flags: NixList::concat_with(
            &lib,
            [
                NixList::<Expr<String>>::new([nix_text!(
                    "ac_cv_prog_CURL_CONFIG={curl}/bin/curl-config",
                    curl = lib.get_dev(i.curl())
                )]),
                (NixList::<Expr<String>>::new(
                    [
                        "ac_cv_fread_reads_directories=yes",
                        "ac_cv_snprintf_returns_bogus=no",
                        "ac_cv_iconv_omits_bom=no",
                    ]
                    .map(Expr::from),
                ))
                .when(&lib, !package::build_host_equal(i.stdenv.as_value())),
            ],
        ),
        pre_build: pre_build(),
        make_flags,
        disallowed_references: NixList::optional(
            &lib,
            !package::build_host_equal(i.stdenv.as_value()),
            i.stdenv.shell_package(),
        ),
        post_build: post_build(i),
        install_flags: vec!["NO_INSTALL_HARDLINKS=1"],
        pre_install: pre_install(i),
        post_install: post_install(i, svn),
        do_check: false,
        do_install_check: i.do_install_check(),
        install_check_target: "test",
        install_check_flags: NixList::<Expr<String>>::new([
            "DEFAULT_TEST_TARGET=prove".into(),
            nix_text!("PERL_PATH={perl}/bin/perl", perl = i.build_packages.perl()),
        ]),
        native_install_check_inputs: NixList::optional(
            &lib,
            i.stdenv
                .host_platform()
                .is_darwin()
                .or(i.stdenv.host_platform().is_free_bsd()),
            i.sysctl(),
        ),
        pre_install_check: pre_install_check(i),
        strip_debug_list: vec![
            "lib",
            "libexec",
            "bin",
            "share/git/contrib/credential/libsecret",
        ],
        passthru: nix_record! {
            "shellPath": "/bin/git-shell",
            "tests": passthru_tests,
            "updateScript": i.file("update.sh"),
        },
        meta: metadata(i),
    }
}

/// Describe the pinned release, retaining licenses, platforms and maintainers from Nix.
fn metadata(i: &Inputs) -> NixValue {
    nix_record! {
        "homepage": "https://git-scm.com/",
        "description": "Distributed version control system",
        "license": i.lib.licenses.gpl2(),
        "changelog": "https://github.com/git/git/blob/v2.47.0/Documentation/RelNotes/2.47.0.txt",
        "longDescription": "Git, a popular distributed version control system designed to\nhandle very large projects with speed and efficiency.\n",
        "platforms": i.lib.platforms.all(),
        "maintainers": NixValue::list([
            i.lib.maintainers.primeos(),
            i.lib.maintainers.wmertens(),
            i.lib.maintainers.globin(),
            i.lib.maintainers.kashw2(),
        ]),
        "mainProgram": "git",
    }
}

/// Patch-phase commands that embed gettext and fix test-script interpreter paths.
fn gettext_patch(i: &Inputs) -> Expr<String> {
    nix_text!(
        r#"
            # Fix references to gettext introduced by ./git-sh-i18n.patch
            substituteInPlace git-sh-i18n.sh \
                --subst-var-by gettext {gettext}

            # ensure we are using the correct shell when executing the test scripts
            patchShebangs t/*.sh
        "#,
        gettext = i.gettext()
    )
}

/// Patch-phase substitutions that make Git use the selected OpenSSH executable.
fn ssh_patch(i: &Inputs) -> Expr<String> {
    nix_text!(
        r#"
            for x in connect.c git-gui/lib/remote_add.tcl ; do
              substituteInPlace "$x" \
                --subst-var-by ssh "{openssh}/bin/ssh"
            done
        "#,
        openssh = i.openssh()
    )
}

/// Build-phase setup that derives the installed Perl library directory from Perl itself.
fn pre_build() -> Expr<String> {
    nix_text!(
        r#"
            makeFlagsArray+=( perllibdir=$out/$(perl -MConfig -wle 'print substr $Config{{installsitelib}}, 1 + length $Config{{siteprefixexp}}') )
        "#
    )
}

/// Commands to build git-subtree in addition to Git's main programs.
fn subtree_build() -> Expr<String> {
    nix_text!(
        r#"
            make -C contrib/subtree
        "#
    )
}

/// Commands to build the Perl-based diff-highlight helper when Perl support is enabled.
fn diff_highlight_build() -> Expr<String> {
    nix_text!(
        r#"
            make -C contrib/diff-highlight
        "#
    )
}

/// Commands to build the optional macOS Keychain credential helper.
fn keychain_build() -> Expr<String> {
    nix_text!(
        r#"
            make -C contrib/credential/osxkeychain
        "#
    )
}

/// Commands to build the optional libsecret credential helper.
fn secret_build() -> Expr<String> {
    nix_text!(
        r#"
            make -C contrib/credential/libsecret
        "#
    )
}

/// Prepare the Keychain helper's executable symlink and remove its intermediate object.
fn keychain_install() -> Expr<String> {
    nix_text!(
        r#"
            mkdir -p $out/bin
            ln -s $out/share/git/contrib/credential/osxkeychain/git-credential-osxkeychain $out/bin/
            rm -f $PWD/contrib/credential/osxkeychain/git-credential-osxkeychain.o
        "#
    )
}

/// Prepare the libsecret helper's executable symlink and remove its intermediate object.
fn secret_install() -> Expr<String> {
    nix_text!(
        r#"
            mkdir -p $out/bin
            ln -s $out/share/git/contrib/credential/libsecret/git-credential-libsecret $out/bin/
            rm -f $PWD/contrib/credential/libsecret/git-credential-libsecret.o
        "#
    )
}

/// Common installation commands for contrib tools, completions and embedded runtime-tool paths.
/// Also defines the removal helper used by disabled-feature installation branches.
fn base_install(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    nix_text!(
        r#"
            notSupported() {{
              unlink $1 || true
            }}

            # Install git-subtree.
            make -C contrib/subtree install {install_doc}
            rm -rf contrib/subtree

            # Install contrib stuff.
            mkdir -p $out/share/git
            cp -a contrib $out/share/git/
            mkdir -p $out/share/bash-completion/completions
            ln -s $out/share/git/contrib/completion/git-completion.bash $out/share/bash-completion/completions/git
            ln -s $out/share/git/contrib/completion/git-prompt.sh $out/share/bash-completion/completions/
            # only readme, developed in another repo
            rm -r contrib/hooks/multimail
            mkdir -p $out/share/git-core/contrib
            cp -a contrib/hooks/ $out/share/git-core/contrib/
            substituteInPlace $out/share/git-core/contrib/hooks/pre-auto-gc-battery \
              --replace ' grep' ' {grep}/bin/grep' \

            # grep is a runtime dependency, need to patch so that it's found
            substituteInPlace $out/libexec/git-core/git-sh-setup \
                --replace ' grep' ' {grep}/bin/grep' \
                --replace ' egrep' ' {grep}/bin/egrep'

            # Fix references to the perl, sed, awk and various coreutil binaries used by
            # shell scripts that git calls (e.g. filter-branch)
            SCRIPT="$(cat <<'EOS'
              BEGIN{{
                @a=(
                  '{grep}/bin/grep', '{sed}/bin/sed', '{awk}/bin/awk',
                  '{coreutils}/bin/cut', '{coreutils}/bin/basename', '{coreutils}/bin/dirname',
                  '{coreutils}/bin/wc', '{coreutils}/bin/tr'
                  {perl_program}
                );
              }}
              foreach $c (@a) {{
                $n=(split("/", $c))[-1];
                s|(?<=[^#][^/.-])\b${{n}}(?=\s)|${{c}}|g
              }}
            EOS
            )"
            perl -0777 -i -pe "$SCRIPT" \
              $out/libexec/git-core/git-{{sh-setup,filter-branch,merge-octopus,mergetool,quiltimport,request-pull,submodule,subtree,web--browse}}


            # Also put git-http-backend into $PATH, so that we can use smart
            # HTTP(s) transports for pushing
            ln -s $out/libexec/git-core/git-http-backend $out/bin/git-http-backend
            ln -s $out/share/git/contrib/git-jump/git-jump $out/bin/git-jump
        "#,
        install_doc = lib.optional_text(i.with_manual(), "install-doc"),
        grep = i.gnugrep(),
        sed = i.gnused(),
        awk = i.gawk(),
        coreutils = i.coreutils(),
        perl_program = lib.optional_text(
            i.perl_support(),
            nix_text!(
                ", '{perl}/bin/perl'",
                perl = i.perl_packages.perl.as_value()
            )
        )
    )
}

/// Wrap Perl helpers with their library paths and patch gitweb's gzip and CGI dependencies.
fn perl_install(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    nix_text!(
        r#"
            # wrap perl commands
            makeWrapper "$out/share/git/contrib/credential/netrc/git-credential-netrc.perl" $out/bin/git-credential-netrc \
                        --set PERL5LIB   "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-cvsimport \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-archimport \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-instaweb \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"
            wrapProgram $out/libexec/git-core/git-cvsexportcommit \
                        --set GITPERLLIB "$out/{perl_prefix}:{perl_path}"

            # gzip (and optionally bzip2, xz, zip) are runtime dependencies for
            # gitweb.cgi, need to patch so that it's found
            sed -i -e "s|'compressor' => \['gzip'|'compressor' => ['{gzip}/bin/gzip'|" \
                $out/share/gitweb/gitweb.cgi
            # Give access to CGI.pm and friends (was removed from perl core in 5.22)
            for p in {gitweb_libs}; do
                sed -i -e "/use CGI /i use lib \"$p/{perl_prefix}\";" \
                    "$out/share/gitweb/gitweb.cgi"
            done
        "#,
        perl_prefix = i.perl_packages.perl.lib_prefix(),
        perl_path = i.perl_packages.make_perl_path().call(i.perl_libs()),
        gzip = i.gzip(),
        gitweb_libs = lib
            .as_expression()
            .clone()
            .select("concatStringsSep")
            .apply([
                " ".into(),
                NixList::new([
                    i.perl_packages.cgi(),
                    i.perl_packages.html_parser(),
                    i.perl_packages.cgi_fast(),
                    i.perl_packages.fcgi(),
                    i.perl_packages.fcgi_proc_manager(),
                    i.perl_packages.html_tag_cloud(),
                ])
                .into()
            ])
    )
}

/// Wrap git-svn with the selected Subversion package and its Perl libraries.
fn svn_install(i: &Inputs, svn: Package) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    nix_text!(
        r#"
            # wrap git-svn
            wrapProgram $out/libexec/git-core/git-svn \
              --set GITPERLLIB "$out/{perl_prefix}:{perl_path}" \
              --prefix PATH : "{svn}/bin"
        "#,
        perl_prefix = i.perl_packages.perl.lib_prefix(),
        perl_path = i.perl_packages.make_perl_path().call(NixList::concat_with(
            &lib,
            [i.perl_libs(), NixList::new([svn.output("out")])]
        )),
        svn = svn.output("out")
    )
}

/// Remove git-svn from the installed output when SVN support is disabled.
fn no_svn_install() -> Expr<String> {
    nix_text!(
        r#"
            # replace git-svn by notification script
            notSupported $out/libexec/git-core/git-svn
        "#
    )
}

/// Wrap git-send-email with the SMTP libraries supplied by the Nix caller.
fn email_install(i: &Inputs) -> Expr<String> {
    nix_text!(
        r#"
            # wrap git-send-email
            wrapProgram $out/libexec/git-core/git-send-email \
                         --set GITPERLLIB "$out/{perl_prefix}:{smtp_path}"
        "#,
        perl_prefix = i.perl_packages.perl.lib_prefix(),
        smtp_path = i.perl_packages.make_perl_path().call(i.smtp_perl_libs())
    )
}

/// Remove git-send-email from the installed output when email support is disabled.
fn no_email_install() -> Expr<String> {
    nix_text!(
        r#"
            # replace git-send-email by notification script
            notSupported $out/libexec/git-core/git-send-email
        "#
    )
}

/// Install documentation using the build-platform Perl, including in cross builds.
fn manual_install(i: &Inputs) -> Expr<String> {
    nix_text!(
        r#"
            # Install man pages
            make -j $NIX_BUILD_CORES PERL_PATH="{perl}/bin/perl" cmd-list.made install install-html \
              -C Documentation
        "#,
        perl = i.build_packages.perl()
    )
}

/// Point Tcl/Tk launchers at nixpkgs' wish interpreter and install gitk completion.
fn gui_install(i: &Inputs) -> Expr<String> {
    nix_text!(
        r#"
            # Wrap Tcl/Tk programs
            for prog in bin/gitk libexec/git-core/{{git-gui,git-citool,git-gui--askpass}}; do
              sed -i -e "s|exec 'wish'|exec '{tk}/bin/wish'|g" \
                     -e "s|exec wish|exec '{tk}/bin/wish'|g" \
                     "$out/$prog"
            done
            ln -s $out/share/git/contrib/completion/git-completion.bash $out/share/bash-completion/completions/gitk
        "#,
        tk = i.tk()
    )
}

/// Remove the GUI entry points when Tcl/Tk support is disabled.
fn no_gui_install() -> Expr<String> {
    nix_text!(
        r#"
            # Don't wrap Tcl/Tk, replace them by notification scripts
            for prog in bin/gitk libexec/git-core/git-gui; do
              notSupported "$out/$prog"
            done
        "#
    )
}

/// Generate the system Git configuration selecting the macOS Keychain credential helper.
fn keychain_config() -> Expr<String> {
    nix_text!(
        r#"
            # enable git-credential-osxkeychain on darwin if desired (default)
            mkdir -p $out/etc
            cat > $out/etc/gitconfig << EOF
            [credential]
              helper = osxkeychain
            EOF
        "#
    )
}

/// Prepare installed tests and exclude sandbox-incompatible tests exactly as upstream does.
/// SVN test selection remains dependent on the final factory arguments.
fn base_check(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    nix_text!(
        r#"
            installCheckFlagsArray+=(
              GIT_PROVE_OPTS="--jobs $NIX_BUILD_CORES --failures --state=failed,save"
              GIT_TEST_INSTALLED=$out/bin
              {no_svn}
            )

            function disable_test {{
              local test=$1 pattern=$2
              if [ $# -eq 1 ]; then
                mv t/{{,skip-}}$test.sh || true
              else
                sed -i t/$test.sh \
                  -e "/^\s*test_expect_.*$pattern/,/^\s*' *\$/{{s/^/: #/}}"
              fi
            }}

            # Shared permissions are forbidden in sandbox builds:
            substituteInPlace t/test-lib.sh \
              --replace "test_set_prereq POSIXPERM" ""
            # TODO: Investigate while these still fail (without POSIXPERM):
            # Tested to fail: 2.46.0
            disable_test t0001-init 'shared overrides system'
            # Tested to fail: 2.46.0
            disable_test t0001-init 'init honors global core.sharedRepository'
            # Tested to fail: 2.46.0
            disable_test t1301-shared-repo
            # /build/git-2.44.0/contrib/completion/git-completion.bash: line 452: compgen: command not found
            disable_test t9902-completion

            # Our patched gettext never fallbacks
            disable_test t0201-gettext-fallbacks
        "#,
        no_svn = lib.optional_text(!i.svn_support(), "NO_SVN_TESTS=y")
    )
}

/// Exclude send-email tests when their corresponding feature is disabled.
fn no_email_check() -> Expr<String> {
    nix_text!(
        r#"
            # Disable sendmail tests
            disable_test t9001-send-email
        "#
    )
}

/// Exclude the pinned recipe's known flaky and filesystem-sensitive tests on all platforms.
fn common_check() -> Expr<String> {
    nix_text!(
        r#"
            # Flaky tests:
            disable_test t6421-merge-partial-clone

            # Fails reproducibly on ZFS on Linux with formD normalization
            disable_test t0021-conversion
            disable_test t3910-mac-os-precompose
        "#
    )
}

/// Apply the pinned recipe's additional macOS test exclusions.
fn darwin_check() -> Expr<String> {
    nix_text!(
        r#"
            # XXX: Some tests added in 2.24.0 fail.
            # Please try to re-enable on the next release.
            disable_test t7816-grep-binary-pattern
            # fail (as of 2.33.0)
            #===(   18623;1208  8/?  224/?  2/? )= =fatal: Not a valid object name refs/tags/signed-empty
            disable_test t6300-for-each-ref
            # not ok 1 - populate workdir (with 2.33.1 on x86_64-darwin)
            disable_test t5003-archive-zip
        "#
    )
}

/// Exclude the upstream fsmonitor test on Apple Silicon.
fn darwin_arm_check() -> Expr<String> {
    nix_text!(
        r#"
            disable_test t7527-builtin-fsmonitor
        "#
    )
}

/// Exclude the upstream locale and encoding tests that fail with musl.
fn musl_check() -> Expr<String> {
    nix_text!(
        r#"
            # Test fails (as of 2.17.0, musl 1.1.19)
            disable_test t3900-i18n-commit
            # Fails largely due to assumptions about BOM
            # Tested to fail: 2.18.0
            disable_test t0028-working-tree-encoding
        "#
    )
}

/// Compose the patch phase, including SSH substitutions only when requested.
fn post_patch(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    Expr::concat([
        gettext_patch(i),
        lib.optional_text(i.with_ssh(), ssh_patch(i)),
    ])
}

/// Compose auxiliary builds in upstream order, retaining symbolic feature conditions.
fn post_build(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    Expr::concat([
        subtree_build(),
        lib.optional_text(i.perl_support(), diff_highlight_build()),
        lib.optional_text(i.osxkeychain_support(), keychain_build()),
        lib.optional_text(i.with_libsecret(), secret_build()),
    ])
}

/// Compose pre-install preparation for whichever credential helpers are enabled.
fn pre_install(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    Expr::concat([
        lib.optional_text(i.osxkeychain_support(), keychain_install()),
        lib.optional_text(i.with_libsecret(), secret_install()),
    ])
}

/// Compose installation and wrapping branches in upstream order.
/// Nix selects feature branches later, preserving laziness and string dependency context.
fn post_install(i: &Inputs, svn: Package) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    Expr::concat([
        base_install(i),
        lib.optional_text(i.perl_support(), perl_install(i)),
        Expr::choose(i.svn_support(), svn_install(i, svn), no_svn_install()),
        Expr::choose(i.send_email_support(), email_install(i), no_email_install()),
        lib.optional_text(i.with_manual(), manual_install(i)),
        Expr::choose(i.gui_support(), gui_install(i), no_gui_install()),
        lib.optional_text(i.osxkeychain_support(), keychain_config()),
    ])
}

/// Compose installed-test setup and feature/platform exclusions in upstream order.
fn pre_install_check(i: &Inputs) -> Expr<String> {
    let lib = NixLibrary::from_expression(i.lib.as_expression());

    Expr::concat([
        base_check(i),
        lib.optional_text(!i.send_email_support(), no_email_check()),
        common_check(),
        lib.optional_text(i.stdenv.host_platform().is_darwin(), darwin_check()),
        lib.optional_text(
            i.stdenv
                .host_platform()
                .is_darwin()
                .and(i.stdenv.host_platform().is_aarch64()),
            darwin_arm_check(),
        ),
        lib.optional_text(i.stdenv.host_platform().is_musl(), musl_check()),
    ])
}
