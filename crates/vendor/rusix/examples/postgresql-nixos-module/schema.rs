//! Declare the services.postgresql options that NixOS checks when combining module definitions.
//! The optional Rust authoring model lives in model.rs; this file defines accepted options and defaults.

// Public documentation mirrors pinned nixpkgs (MIT).
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};
use rusix::{
    interop::{Nixpkgs, raw::NixValue},
    nix_record,
    nixos::{NixosModule, OptionDecl, OptionType},
};

/// Defines PostgreSQL's accepted NixOS options as a nested declaration tree.
/// These fields describe types and defaults; configuration contributions supply their values.
#[rusix::config]
mod declarations {
    use super::OptionDecl;

    // Passed to NixosModule::declare, this tree becomes options rather than config.
    #[rusix(root)]
    pub(super) struct Root {
        // Places the declarations under the standard NixOS services namespace.
        pub(super) services: Services,
    }

    // Owns just PostgreSQL's interface; it does not describe every NixOS service.
    pub(super) struct Services {
        // Nesting produces the services.postgresql option path automatically.
        pub(super) postgresql: Postgresql,
    }

    // Public compatibility options, separate from the stronger Rust model in model.rs.
    // Each OptionDecl supplies a NixOS type and metadata, not a concrete Rust setting.
    pub(super) struct Postgresql {
        // Declares the switch that gates the generated service implementation.
        pub(super) enable: OptionDecl,
        // Selects JIT support while retaining the upstream acronym spelling.
        #[rusix(rename = "enableJIT")]
        pub(super) enable_jit: OptionDecl,
        // Uses the real package validator and mkPackageOption's package-set default.
        pub(super) package: OptionDecl,
        // Allows disabling the generated configuration check; defaults to true.
        pub(super) check_config: OptionDecl,
        // Declares a path; lowering supplies its computed default, while this schema documents it.
        pub(super) data_dir: OptionDecl,
        // Mergeable pg_hba.conf text; mkForce can replace the implementation's default rules.
        pub(super) authentication: OptionDecl,
        // Mergeable pg_ident.conf text; PostgreSQL interprets the mapping syntax.
        pub(super) ident_map: OptionDecl,
        // An ordered, mergeable list of extra initdb arguments.
        pub(super) initdb_args: OptionDecl,
        // Accepts null or a SQL-file path, including a derivation, for cluster initialization.
        pub(super) initial_script: OptionDecl,
        // An ordered list of database names to ensure when the service starts.
        pub(super) ensure_databases: OptionDecl,
        // A list of role submodules; ownership relationships are checked by the implementation.
        pub(super) ensure_users: OptionDecl,
        // Controls the default TCP listening setting, preserving the legacy option name.
        #[rusix(rename = "enableTCPIP")]
        pub(super) enable_tcpip: OptionDecl,
        // Accepts a package-set callback; NixOS also coerces lists of extension paths to callbacks.
        pub(super) extra_plugins: OptionDecl,
        // Keeps arbitrary PostgreSQL setting names open while validating their value types.
        pub(super) settings: OptionDecl,
        // Nullable, mergeable recovery.conf text retained for older PostgreSQL versions.
        pub(super) recovery_config: OptionDecl,
        // An internal read-only bootstrap role name, fixed to postgres by the schema default.
        pub(super) super_user: OptionDecl,
    }

    // Declares one ensureUsers entry, retaining the public Nix shape instead of the Rust ownership model.
    pub(super) struct User {
        // A required role name; omitting it remains a NixOS evaluation error.
        pub(super) name: OptionDecl,
        // Requests ownership of a same-named ensured database; lowering checks that it exists.
        #[rusix(rename = "ensureDBOwnership")]
        pub(super) ensure_db_ownership: OptionDecl,
        // A nested submodule whose omitted clauses each default to null.
        pub(super) ensure_clauses: OptionDecl,
    }

    // Every clause accepts true, false or null: enable, disable or leave PostgreSQL's state alone.
    pub(super) struct Clauses {
        // Controls unrestricted superuser privileges.
        pub(super) superuser: OptionDecl,
        // Controls permission to create and manage roles.
        pub(super) createrole: OptionDecl,
        // Controls permission to create databases.
        pub(super) createdb: OptionDecl,
        // Controls automatic inheritance of privileges from granted roles.
        pub(super) inherit: OptionDecl,
        // Controls whether this role can authenticate as a database user.
        pub(super) login: OptionDecl,
        // Controls replication-role privileges.
        pub(super) replication: OptionDecl,
        // Controls whether the role bypasses row-level security policies.
        pub(super) bypassrls: OptionDecl,
    }

    // Only these settings have explicit declarations; freeformType validates all other setting values.
    // PostgreSQL's literal snake_case names override Rusix's default lowerCamelCase mapping.
    pub(super) struct Settings {
        // Accepts null, text or a string list coerced to comma-separated library names.
        #[rusix(rename = "shared_preload_libraries")]
        pub(super) shared_preload_libraries: OptionDecl,
        // Supplies the format prefix used for PostgreSQL log messages.
        #[rusix(rename = "log_line_prefix")]
        pub(super) log_line_prefix: OptionDecl,
        // Uses NixOS's actual port validator, with a schema default of 5432.
        pub(super) port: OptionDecl,
    }
}

fn literal(text: &str) -> NixValue {
    // Documentation code stays unevaluated; this does not inject executable Nix source.
    Nixpkgs::new().function("literalExpression").call(text)
}

fn markdown(text: &str) -> NixValue {
    // Retain NixOS's documentation wrapper for formatted default descriptions.
    Nixpkgs::new().function("literalMD").call(text)
}

fn clauses() -> OptionType {
    // Null means no SQL change, not false; PostgreSQL supplies the initial state for a new role.
    let clause = |description| {
        OptionDecl::new(OptionType::named("bool").null_or())
        .default(NixValue::null())
        .default_text(markdown("`null`: do not set. For newly created roles, use PostgreSQL's default. For existing roles, do not touch this clause.\n"))
        .description(description)
    };

    OptionType::submodule(
        declarations::Clauses {
            superuser: clause(docs::SUPERUSER),
            createrole: clause(docs::CREATEROLE),
            createdb: clause(docs::CREATEDB),
            inherit: clause(docs::INHERIT),
            login: clause(docs::LOGIN),
            replication: clause(docs::REPLICATION),
            bypassrls: clause(docs::BYPASSRLS),
        },
        None,
    )
    .unwrap()
}

fn users() -> OptionType {
    // The empty clause submodule still applies each clause's null default during NixOS evaluation.
    OptionType::submodule(declarations::User {
        name: OptionDecl::new(OptionType::named("str")).description(docs::USER_NAME),
        ensure_db_ownership: OptionDecl::new(OptionType::named("bool"))
            .default(false).description(docs::OWNERSHIP),
        ensure_clauses: OptionDecl::new(clauses())
            .default(nix_record! {})
            .default_text(markdown("The default, `null`, means that the user created will have the default permissions assigned by PostgreSQL. Subsequent server starts will not set or unset the clause, so imperative changes are preserved.\n"))
            .example(literal("{\n  superuser = true;\n  createrole = true;\n  createdb = true;\n}\n"))
            .description(docs::ENSURE_CLAUSES),
    }, None).unwrap().list_of()
}

fn settings() -> OptionType {
    // Unknown keys accept these four scalar types, not arbitrary lists, records or null.
    let freeform =
        OptionType::one_of(["bool", "float", "int", "str"].map(OptionType::named)).attrs_of();
    let preload = OptionType::named("str")
        .coerced_from(
            OptionType::named("str").list_of(),
            Nixpkgs::new().function("concatStringsSep").call(", "),
        )
        .null_or();

    OptionType::submodule(
        declarations::Settings {
            shared_preload_libraries: OptionDecl::new(preload)
                .default(NixValue::null())
                .example(literal("[ \"auto_explain\" \"anon\" ]"))
                .description(docs::SHARED_PRELOAD_LIBRARIES),
            log_line_prefix: OptionDecl::new(OptionType::named("str"))
                .default("[%p] ")
                .example("%m [%p] ")
                .description(docs::LOG_LINE_PREFIX),
            port: OptionDecl::new(OptionType::named("port"))
                .default(5432_i64)
                .description(docs::PORT),
        },
        Some(freeform),
    )
    .unwrap()
}

/// Complete public declarations and migration imports, without the original PostgreSQL module.
pub(crate) fn module() -> NixosModule {
    // Legacy extension lists become constant callbacks; function results keep the real path validator.
    let plugins = OptionType::named("path")
        .list_of()
        .function_to()
        .coerced_from(
            OptionType::named("path").list_of(),
            NixValue::function(|paths| NixValue::function(|_| paths)),
        );

    // Use NixOS's supplied pkgs so package defaults respect its platform, configuration and overlays.
    let package = Nixpkgs::new().function("mkPackageOption").apply([
        Nixpkgs::from_module().as_value(),
        "postgresql".into(),
        nix_record! { "example": "postgresql_15" },
    ]);

    let module = NixosModule::empty().declare(declarations::Root {
        services: declarations::Services {
            postgresql: declarations::Postgresql {
                enable: OptionDecl::enable("PostgreSQL Server"),
                enable_jit: OptionDecl::enable("JIT support"),
                package: OptionDecl::from_value(package),
                check_config: OptionDecl::new(OptionType::named("bool"))
                    .default(true).description(docs::CHECK_CONFIG),
                data_dir: OptionDecl::new(OptionType::named("path"))
                    .default_text(literal(r#""/var/lib/postgresql/${config.services.postgresql.package.psqlSchema}""#))
                    .example("/var/lib/postgresql/15").description(docs::DATA_DIR),
                authentication: OptionDecl::new(OptionType::named("lines"))
                    .default("").description(docs::AUTHENTICATION),
                ident_map: OptionDecl::new(OptionType::named("lines"))
                    .default("")
                    .example("map-name-0 system-username-0 database-username-0\nmap-name-1 system-username-1 database-username-1\n")
                    .description(docs::IDENT_MAP),
                initdb_args: OptionDecl::new(OptionType::named("str").list_of())
                    .default(NixValue::list([]))
                    .example(NixValue::list(["--data-checksums".into(), "--allow-group-access".into()]))
                    .description(docs::INITDB_ARGS),
                initial_script: OptionDecl::new(OptionType::named("path").null_or())
                    .default(NixValue::null())
                    .example(literal("pkgs.writeText \"init-sql-script\" ''\n  alter user postgres with password 'myPassword';\n'';"))
                    .description(docs::INITIAL_SCRIPT),
                ensure_databases: OptionDecl::new(OptionType::named("str").list_of())
                    .default(NixValue::list([]))
                    .example(NixValue::list(["gitea".into(), "nextcloud".into()]))
                    .description(docs::ENSURE_DATABASES),
                ensure_users: OptionDecl::new(users())
                    .default(NixValue::list([]))
                    .example(literal("[\n  {\n    name = \"nextcloud\";\n  }\n  {\n    name = \"superuser\";\n    ensureDBOwnership = true;\n  }\n]\n"))
                    .description(docs::ENSURE_USERS),
                enable_tcpip: OptionDecl::new(OptionType::named("bool"))
                    .default(false).description(docs::ENABLE_TCPIP),
                extra_plugins: OptionDecl::new(plugins)
                    .default(NixValue::function(|_| NixValue::list([])))
                    .example(literal("ps: with ps; [ postgis pg_repack ]"))
                    .description(docs::EXTRA_PLUGINS),
                settings: OptionDecl::new(settings())
                    .default(nix_record! {})
                    .example(literal("{\n  log_connections = true;\n  log_statement = \"all\";\n  logging_collector = true;\n  log_disconnections = true;\n  log_destination = lib.mkForce \"syslog\";\n}\n"))
                    .description(docs::SETTINGS),
                recovery_config: OptionDecl::new(OptionType::named("lines").null_or())
                    .default(NixValue::null()).description(docs::RECOVERY_CONFIG),
                super_user: OptionDecl::new(OptionType::named("str"))
                    .default("postgres").internal().read_only().description(docs::SUPER_USER),
            },
        },
    });

    // Existing NixOS migration helpers preserve removed/renamed-option semantics, not PostgreSQL code.
    let path = |parts: &[&str]| NixValue::list(parts.iter().map(|part| NixValue::from(*part)));
    module
        .import_value(Nixpkgs::new().function("mkRemovedOptionModule").apply([
            path(&["services", "postgresql", "extraConfig"]),
            "Use services.postgresql.settings instead.".into(),
        ]))
        .import_value(Nixpkgs::new().function("mkRenamedOptionModule").apply([
            path(&["services", "postgresql", "logLinePrefix"]),
            path(&["services", "postgresql", "settings", "log_line_prefix"]),
        ]))
        .import_value(Nixpkgs::new().function("mkRenamedOptionModule").apply([
            path(&["services", "postgresql", "port"]),
            path(&["services", "postgresql", "settings", "port"]),
        ]))
}

/// Retains the pinned upstream descriptions shown in PostgreSQL's NixOS option documentation.
/// The declarations attach these strings to their options alongside types and defaults.
mod docs {
    pub(super) const CHECK_CONFIG: &str =
        r#"Check the syntax of the configuration file at compile time"#;

    pub(super) const DATA_DIR: &str = r#"The data directory for PostgreSQL. If left as the default value
this directory will automatically be created before the PostgreSQL server starts, otherwise
the sysadmin is responsible for ensuring the directory exists with appropriate ownership
and permissions.
"#;

    pub(super) const AUTHENTICATION: &str = r#"Defines how users authenticate themselves to the server. See the
[PostgreSQL documentation for pg_hba.conf](https://www.postgresql.org/docs/current/auth-pg-hba-conf.html)
for details on the expected format of this option. By default,
peer based authentication will be used for users connecting
via the Unix socket, and md5 password authentication will be
used for users connecting via TCP. Any added rules will be
inserted above the default rules. If you'd like to replace the
default rules entirely, you can use `lib.mkForce` in your
module.
"#;

    pub(super) const IDENT_MAP: &str = r#"Defines the mapping from system users to database users.

See the [auth doc](https://postgresql.org/docs/current/auth-username-maps.html).
"#;

    pub(super) const INITDB_ARGS: &str = r#"Additional arguments passed to `initdb` during data dir
initialisation.
"#;

    pub(super) const INITIAL_SCRIPT: &str = r#"A file containing SQL statements to execute on first startup.
"#;

    pub(super) const ENSURE_DATABASES: &str = r#"Ensures that the specified databases exist.
This option will never delete existing databases, especially not when the value of this
option is changed. This means that databases created once through this option or
otherwise have to be removed manually.
"#;

    pub(super) const USER_NAME: &str = r#"Name of the user to ensure.
"#;

    pub(super) const OWNERSHIP: &str = r#"Grants the user ownership to a database with the same name.
This database must be defined manually in
[](#opt-services.postgresql.ensureDatabases).
"#;

    pub(super) const ENSURE_CLAUSES: &str = r#"An attrset of clauses to grant to the user. Under the hood this uses the
[ALTER USER syntax](https://www.postgresql.org/docs/current/sql-alteruser.html) for each attrName where
the attrValue is true in the attrSet:
`ALTER USER user.name WITH attrName`
"#;

    pub(super) const SUPERUSER: &str = r#"Grants the user, created by the ensureUser attr, superuser permissions. From the postgres docs:

A database superuser bypasses all permission checks,
except the right to log in. This is a dangerous privilege
and should not be used carelessly; it is best to do most
of your work as a role that is not a superuser. To create
a new database superuser, use CREATE ROLE name SUPERUSER.
You must do this as a role that is already a superuser.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const CREATEROLE: &str = r#"Grants the user, created by the ensureUser attr, createrole permissions. From the postgres docs:

A role must be explicitly given permission to create more
roles (except for superusers, since those bypass all
permission checks). To create such a role, use CREATE
ROLE name CREATEROLE. A role with CREATEROLE privilege
can alter and drop other roles, too, as well as grant or
revoke membership in them. However, to create, alter,
drop, or change membership of a superuser role, superuser
status is required; CREATEROLE is insufficient for that.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const CREATEDB: &str = r#"Grants the user, created by the ensureUser attr, createdb permissions. From the postgres docs:

A role must be explicitly given permission to create
databases (except for superusers, since those bypass all
permission checks). To create such a role, use CREATE
ROLE name CREATEDB.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const INHERIT: &str = r#"Grants the user created inherit permissions. From the postgres docs:

A role is given permission to inherit the privileges of
roles it is a member of, by default. However, to create a
role without the permission, use CREATE ROLE name
NOINHERIT.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const LOGIN: &str = r#"Grants the user, created by the ensureUser attr, login permissions. From the postgres docs:

Only roles that have the LOGIN attribute can be used as
the initial role name for a database connection. A role
with the LOGIN attribute can be considered the same as a
“database user”. To create a role with login privilege,
use either:

CREATE ROLE name LOGIN; CREATE USER name;

(CREATE USER is equivalent to CREATE ROLE except that
CREATE USER includes LOGIN by default, while CREATE ROLE
does not.)

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const REPLICATION: &str = r#"Grants the user, created by the ensureUser attr, replication permissions. From the postgres docs:

A role must explicitly be given permission to initiate
streaming replication (except for superusers, since those
bypass all permission checks). A role used for streaming
replication must have LOGIN permission as well. To create
such a role, use CREATE ROLE name REPLICATION LOGIN.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const BYPASSRLS: &str = r#"Grants the user, created by the ensureUser attr, replication permissions. From the postgres docs:

A role must be explicitly given permission to bypass
every row-level security (RLS) policy (except for
superusers, since those bypass all permission checks). To
create such a role, use CREATE ROLE name BYPASSRLS as a
superuser.

More information on postgres roles can be found [here](https://www.postgresql.org/docs/current/role-attributes.html)
"#;

    pub(super) const ENSURE_USERS: &str = r#"Ensures that the specified users exist.
The PostgreSQL users will be identified using peer authentication. This authenticates the Unix user with the
same name only, and that without the need for a password.
This option will never delete existing users or remove DB ownership of databases
once granted with `ensureDBOwnership = true;`. This means that this must be
cleaned up manually when changing after changing the config in here.
"#;

    pub(super) const ENABLE_TCPIP: &str = r#"Whether PostgreSQL should listen on all network interfaces.
If disabled, the database can only be accessed via its Unix
domain socket or via TCP connections to localhost.
"#;

    pub(super) const EXTRA_PLUGINS: &str = r#"List of PostgreSQL plugins.
"#;

    pub(super) const SHARED_PRELOAD_LIBRARIES: &str = r#"List of libraries to be preloaded.
"#;

    pub(super) const LOG_LINE_PREFIX: &str = r#"A printf-style string that is output at the beginning of each log line.
Upstream default is `'%m [%p] '`, i.e. it includes the timestamp. We do
not include the timestamp, because journal has it anyway.
"#;

    pub(super) const PORT: &str = r#"The port on which PostgreSQL listens.
"#;

    pub(super) const SETTINGS: &str = r#"PostgreSQL configuration. Refer to
<https://www.postgresql.org/docs/current/config-setting.html#CONFIG-SETTING-CONFIGURATION-FILE>
for an overview of `postgresql.conf`.

::: {.note}
String values will automatically be enclosed in single quotes. Single quotes will be
escaped with two single quotes as described by the upstream documentation linked above.
:::
"#;

    pub(super) const RECOVERY_CONFIG: &str = r#"Contents of the {file}`recovery.conf` file.
"#;

    pub(super) const SUPER_USER: &str = r#"PostgreSQL superuser account to use for various operations. Internal since changing
this value would lead to breakage while setting up databases.
"#;
}
