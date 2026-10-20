//! Shell commands used by the pinned OpenSSL build recipes.
//! Rust constructs their text; Nix runs them during package builds.
use rusix::{
    Expr,
    interop::{Package, raw::NixValue},
    nix_text,
};

pub fn patch_configure() -> Expr<String> {
    nix_text!(
        r#"
            patchShebangs Configure
        "#,
    )
}

pub fn patch_old_tests() -> Expr<String> {
    nix_text!(
        r#"
            patchShebangs test/*
            for a in test/t* ; do
              substituteInPlace "$a" \
                --replace /bin/rm rm
            done
        "#,
    )
}

pub fn patch_env(coreutils: Package) -> Expr<String> {
    nix_text!(
        r#"
            substituteInPlace config --replace '/usr/bin/env' '{coreutils}/bin/env'
        "#,
        coreutils = coreutils,
    )
}

pub fn patch_musl() -> Expr<String> {
    nix_text!(
        r#"
            substituteInPlace crypto/async/arch/async_posix.h \
              --replace '!defined(__ANDROID__) && !defined(__OpenBSD__)' \
                        '!defined(__ANDROID__) && !defined(__OpenBSD__) && 0'
        "#,
    )
}

pub fn patch_static_engines() -> Expr<String> {
    nix_text!(
        r#"
            substituteInPlace Configurations/unix-Makefile.tmpl \
              --replace 'ENGINESDIR=$(libdir)/engines-{{- $sover_dirname -}}' \
                        'ENGINESDIR=$(OPENSSLDIR)/engines-{{- $sover_dirname -}}'
        "#,
    )
}

pub fn install_static() -> Expr<String> {
    nix_text!(
        r#"
            # OPENSSLDIR has a reference to self
            remove-references-to -t $out $out/lib/*.a
        "#,
    )
}

pub fn install_shared() -> Expr<String> {
    nix_text!(
        r#"
            # If we're building dynamic libraries, then don't install static
            # libraries.
            if [ -n "$(echo $out/lib/*.so $out/lib/*.dylib $out/lib/*.dll)" ]; then
                rm "$out/lib/"*.a
            fi

            # 'etc' is a separate output on static builds only.
            etc=$out
        "#,
    )
}

pub fn install_bin() -> Expr<String> {
    nix_text!(
        r#"
            mkdir -p $bin
            mv $out/bin $bin/bin

        "#,
    )
}

pub fn install_rehash() -> Expr<String> {
    nix_text!(
        r#"
            # c_rehash is a legacy perl script with the same functionality
            # as `openssl rehash`
            # this wrapper script is created to maintain backwards compatibility without
            # depending on perl
            makeWrapper $bin/bin/openssl $bin/bin/c_rehash \
              --add-flags "rehash"
        "#,
    )
}

pub fn install_dev() -> Expr<String> {
    nix_text!(
        r#"

            mkdir $dev
            mv $out/include $dev/

            # remove dependency on Perl at runtime
            rm -r $etc/etc/ssl/misc

            rmdir $etc/etc/ssl/{{certs,private}}
        "#,
    )
}

pub fn install_conf(conf: NixValue) -> Expr<String> {
    nix_text!(
        r#"
            cat {conf} > $etc/etc/ssl/openssl.cnf
        "#,
        conf = conf,
    )
}

pub fn fixup_perl(perl: Package) -> Expr<String> {
    nix_text!(
        r#"
            # Check to make sure the main output and the static runtime dependencies
            # don't depend on perl
            if grep -r '{perl}' $out $etc; then
              echo "Found an erroneous dependency on perl ^^^" >&2
              exit 1
            fi
        "#,
        perl = perl,
    )
}

pub fn fixup_cmake() -> Expr<String> {
    nix_text!(
        r#"
            # cleanup cmake helpers for now (for OpenSSL >= 3.3), only rely on pkg-config.
            # pkg-config gets its paths fixed correctly
            rm -rf $dev/lib/cmake
        "#,
    )
}
