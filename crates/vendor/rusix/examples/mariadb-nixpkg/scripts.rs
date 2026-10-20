//! Shell commands used by the pinned MariaDB build recipes.
//! Interpolated package paths retain their dependencies for Nix to resolve during builds.
use rusix::{Expr, nix_text};

pub fn pre_patch() -> Expr<String> {
    nix_text!(
        r#"
            sed -i 's,[^"]*/var/log,/var/log,g' storage/mroonga/vendor/groonga/CMakeLists.txt
        "#,
    )
}

pub fn post_install_common() -> Expr<String> {
    nix_text!(
        r#"
            # Remove Development components. Need to use libmysqlclient.
            rm "$out"/lib/mysql/plugin/daemon_example.ini
            rm "$out"/lib/{{libmariadb.a,libmariadbclient.a,libmysqlclient.a,libmysqlclient_r.a,libmysqlservices.a}}
            rm -f "$out"/bin/{{mariadb-config,mariadb_config,mysql_config}}
            rm -r $out/include
            rm -r $out/lib/pkgconfig
        "#,
    )
}

pub fn post_fixup(bin_path: Expr<String>) -> Expr<String> {
    nix_text!(
        r#"
            wrapProgram $out/bin/mytop --set PATH {bin_path}
        "#,
        bin_path = bin_path,
    )
}

pub fn post_install_client(lib_ext: Expr<String>) -> Expr<String> {
    nix_text!(
        r#"
            rm "$out"/bin/{{mariadb-test,mysqltest}}
            libmysqlclient_path=$(readlink -f $out/lib/libmysqlclient{lib_ext})
            rm "$out"/lib/{{libmariadb{lib_ext},libmysqlclient{lib_ext},libmysqlclient_r{lib_ext}}}
            mv "$libmysqlclient_path" "$out"/lib/libmysqlclient{lib_ext}
            ln -sv libmysqlclient{lib_ext} "$out"/lib/libmysqlclient_r{lib_ext}
        "#,
        lib_ext = lib_ext,
    )
}

pub fn post_patch_server() -> Expr<String> {
    nix_text!(
        r#"
            substituteInPlace scripts/galera_new_cluster.sh \
              --replace ":-mariadb" ":-mysql"
        "#,
    )
}

pub fn pre_configure() -> Expr<String> {
    nix_text!(
        r#"
            patchShebangs scripts/mytop.sh
        "#,
    )
}

pub fn post_install_server() -> Expr<String> {
    nix_text!(
        r#"
            rm -r "$out"/share/aclocal
            chmod +x "$out"/bin/wsrep_sst_common
            rm -f "$out"/bin/{{mariadb-client-test,mariadb-test,mysql_client_test,mysqltest}}
        "#,
    )
}

pub fn install_mroonga() -> Expr<String> {
    nix_text!(
        r#"
            mv "$out"/share/{{groonga,groonga-normalizer-mysql}} "$out"/share/doc/mysql
        "#,
    )
}

pub fn install_pam() -> Expr<String> {
    nix_text!(
        r#"
            mv "$out"/OFF/suite/plugins/pam/pam_mariadb_mtr.so "$out"/share/pam/lib/security
            mv "$out"/OFF/suite/plugins/pam/mariadb_mtr "$out"/share/pam/etc/security
            rm -r "$out"/OFF
        "#,
    )
}
