//! Derive PostgreSQL service files and startup commands from the final NixOS options.
//! Rust constructs expressions; Nix resolves them after combining the declarations in schema.rs with other modules.
use super::{
    model::{Clause, Database, Postgresql, Role, RoleClauses},
    options,
};
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};
use rusix::{
    Config, Expr, IntoConfig, IntoRusixValue,
    interop::{Nixpkgs, PackageRef, raw::NixValue},
    nix_record as record, nix_text,
    nixos::{self, DefinitionPriority, NixosModule},
    package,
};

impl Clause {
    fn value(self) -> NixValue {
        // null means preserve; false must instead generate an explicit NO... clause.
        match self {
            Self::Preserve => NixValue::null(),
            Self::Enable => true.into(),
            Self::Disable => false.into(),
        }
    }
}

// This adapter has domain semantics: omit unspecified inputs and derive matching owner roles.
// The structural roots below determine placement under services.postgresql.
impl IntoConfig for Postgresql {
    #[track_caller]
    fn into_config(self) -> Config {
        // Empty collections mean no author-supplied definition here, not an empty override.
        let extra_plugins = (!self.extensions.is_empty()).then(|| {
            NixValue::function(move |packages| {
                NixValue::list(
                    self.extensions
                        .into_iter()
                        .map(|name| packages.clone().select(&name)),
                )
            })
        });
        let settings = (!self.settings.is_empty()).then(|| NixValue::record(self.settings));
        let initdb_args = (!self.initdb_args.is_empty()).then_some(self.initdb_args);

        let mut databases = Vec::new();
        let mut roles = Vec::new();

        for database in self.databases {
            match database {
                Database::Unowned(name) => databases.push(name.into()),
                Database::Owned { name, clauses } => {
                    databases.push(name.clone().into());
                    roles.push(role_value(Role { name, clauses }, true));
                }
            }
        }

        roles.extend(self.roles.into_iter().map(|role| role_value(role, false)));

        config::inputs(config::PostgresqlInput {
            enable: self.enable,
            package: self.package,
            enable_jit: self.enable_jit,
            enable_tcpip: self.enable_tcpip,
            check_config: self.check_config,
            data_dir: self.data_dir,
            authentication: self.authentication,
            ident_map: self.ident_map,
            initial_script: self.initial_script,
            recovery_config: self.recovery_config,
            extra_plugins,
            settings,
            initdb_args,
            ensure_databases: (!databases.is_empty()).then_some(databases),
            ensure_users: (!roles.is_empty()).then_some(roles),
        })
        .into_config()
    }
}

fn role_value(role: Role, owns_database: bool) -> NixValue {
    record! {
        "name": role.name,
        "ensureDBOwnership": owns_database,
        "ensureClauses": role_clauses(role.clauses),
    }
}

fn role_clauses(clauses: RoleClauses) -> NixValue {
    // Omit absent clauses, but preserve Some(Preserve) as an explicit null value.
    opaque(config::RoleClauseInputs {
        superuser: clauses.superuser.map(Clause::value),
        createrole: clauses.createrole.map(Clause::value),
        createdb: clauses.createdb.map(Clause::value),
        inherit: clauses.inherit.map(Clause::value),
        login: clauses.login.map(Clause::value),
        replication: clauses.replication.map(Clause::value),
        bypassrls: clauses.bypassrls.map(Clause::value),
    })
}

// NixOS path options accept strings or paths; toString preserves their context.
#[track_caller]
fn path_text(value: Expr<String>) -> NixValue {
    NixValue::from(value).to_text()
}

#[track_caller]
fn optional_file_script(value: NixValue, text: NixValue) -> NixValue {
    NixValue::if_else(value.equals(NixValue::null()), "", text)
}

fn effective_package() -> NixValue {
    let pg = options::root().services.postgresql;
    let package = pg.package();
    let base = NixValue::if_else(
        pg.enable_jit(),
        package.clone().select("withJIT"),
        package.select("withoutJIT"),
    );

    // The upstream empty-list special case avoids calling withPackages at all.
    // Otherwise nixpkgs resolves extensions for the selected package variant.
    NixValue::if_else(
        pg.extra_plugins().equals(NixValue::list([])),
        base.clone(),
        base.select("withPackages").call(pg.extra_plugins()),
    )
}

fn default_package() -> NixValue {
    let pg = options::root().services.postgresql;
    let removed = |version: &str| {
        Nixpkgs::new().library().throw_if_not(
            false,
            format!("postgresql_{version} was removed, please upgrade your postgresql version."),
            NixValue::null(),
        )
    };

    // Preserve state-version defaults and lazy errors for removed versions.
    // A later package definition can override this default without forcing those errors.
    let mut package = removed("9_5");

    for (state_version, candidate) in [
        ("17.09", removed("9_6")),
        ("20.03", removed("11")),
        ("21.11", Nixpkgs::from_module().get("postgresql_13").into()),
        ("22.05", Nixpkgs::from_module().get("postgresql_14").into()),
        ("23.11", Nixpkgs::from_module().get("postgresql_15").into()),
        ("24.11", Nixpkgs::from_module().get("postgresql_16").into()),
    ] {
        package = NixValue::if_else(
            Nixpkgs::new()
                .library()
                .version_at_least(options::root().system.state_version(), state_version),
            candidate,
            package,
        );
    }

    NixValue::if_else(pg.enable_jit(), package.clone().select("withJIT"), package)
}

fn settings_text() -> NixValue {
    let pg = options::root().services.postgresql;

    // null omits a setting; the callbacks run in Nix over the final merged settings.
    let printable = Nixpkgs::new().function("filterAttrs").apply([
        NixValue::function(|_| {
            NixValue::function(|value| {
                NixValue::if_else(value.equals(NixValue::null()), false, true)
            })
        }),
        pg.settings.as_value(),
    ]);

    let lines = Nixpkgs::new().function("mapAttrsToList").apply([
        NixValue::function(|name| {
            NixValue::function(|value| {
                let quoted = nix_text!(
                    "'{value}'",
                    value = Nixpkgs::new()
                        .library()
                        .replace_text(value.clone().into_expr::<String>(), [("'", "''")]),
                );

                // Match upstream boolean spelling and PostgreSQL single-quote escaping.
                let rendered = NixValue::if_else(
                    value.clone().equals(true),
                    "yes",
                    NixValue::if_else(
                        value.clone().equals(false),
                        "no",
                        NixValue::if_else(
                            Nixpkgs::new().function("isString").apply([value.clone()]),
                            quoted,
                            value.to_text(),
                        ),
                    ),
                );

                nix_text!("{name} = {rendered}", name = name, rendered = rendered).into()
            })
        }),
        printable,
    ]);

    NixValue::join_text("\n", lines)
}

fn configuration_file() -> NixValue {
    // This opaque helper call produces a derivation, retaining store-path string context.
    // Rust describes its inputs; nixpkgs owns file generation and nothing is built here.
    Nixpkgs::from_module()
        .pkgs_function("writeTextDir")
        .apply(["postgresql.conf".into(), settings_text()])
}

fn pre_start() -> NixValue {
    let pg = options::root().services.postgresql;
    let data = path_text(pg.data_dir());

    let recovery = optional_file_script(
        pg.recovery_config(),
        nix_text!(
            r#"
                ln -sfn "{file}" "{data}/recovery.conf"
            "#,
            file = Nixpkgs::from_module()
                .pkgs_function("writeText")
                .apply(["recovery.conf".into(), pg.recovery_config()]),
            data = data.clone(),
        )
        .into(),
    );

    // Leading-newline blocks dedent the shell text; holes remain deferred Nix values.
    nix_text!(
        r#"
            if ! test -e {data}/PG_VERSION; then
              # Cleanup the data directory.
              rm -f {data}/*.conf

              # Initialise the database.
              initdb -U {super_user} {initdb_args}

              # See postStart!
              touch "{data}/.first_startup"
            fi

            ln -sfn "{config_file}/postgresql.conf" "{data}/postgresql.conf"
            {recovery}
        "#,
        data = data,
        super_user = pg.super_user(),
        initdb_args = Nixpkgs::new()
            .function("escapeShellArgs")
            .apply([pg.initdb_args()]),
        config_file = configuration_file(),
        recovery = recovery,
    )
    .into()
}

fn post_start() -> NixValue {
    let pg = options::root().services.postgresql;
    let data = path_text(pg.data_dir());

    let initial = optional_file_script(
        pg.initial_script(),
        nix_text!(
            r#"
                $PSQL -f "{script}" -d postgres
            "#,
            script = pg.initial_script(),
        )
        .into(),
    );

    let databases = Nixpkgs::new().function("concatMapStrings").apply([
        NixValue::function(|database| {
            nix_text!(
                r#"
                    $PSQL -tAc "SELECT 1 FROM pg_database WHERE datname = '{database}'" | grep -q 1 || $PSQL -tAc 'CREATE DATABASE "{database}"'
                "#,
                database = database,
            ).into()
        }),
        pg.ensure_databases(),
    ]);

    let users = Nixpkgs::new().function("concatMapStrings").apply([
        NixValue::function(|user| {
            let name = user.clone().select("name");

            // Preserve leaves a clause out of ALTER ROLE; Disable emits NO plus its name.
            let clauses = Nixpkgs::new().function("filterAttrs").apply([
                NixValue::function(|_| {
                    NixValue::function(|value| {
                        NixValue::if_else(value.equals(NixValue::null()), false, true)
                    })
                }),
                user.clone().select("ensureClauses"),
            ]);

            let clauses = Nixpkgs::new().function("attrValues").apply([Nixpkgs::new()
                .function("mapAttrs")
                .apply([
                    NixValue::function(|name| {
                        NixValue::function(|enabled| {
                            NixValue::if_else(enabled, name.clone(), nix_text!("no{name}", name = name))
                        })
                    }),
                    clauses,
                ])]);

            let ownership = NixValue::if_else(
                user.select("ensureDBOwnership"),
                nix_text!(
                    r#"$PSQL -tAc 'ALTER DATABASE "{name}" OWNER TO "{name}";' "#,
                    name = name.clone(),
                ),
                "",
            );

            nix_text!(
                r#"
                    $PSQL -tAc "SELECT 1 FROM pg_roles WHERE rolname='{name}'" | grep -q 1 || $PSQL -tAc 'CREATE USER "{name}"'
                    $PSQL -tAc 'ALTER ROLE "{name}" {clauses}'{trailing_space}

                    {ownership}
                "#,
                name = name,
                clauses = NixValue::join_text(" ", clauses),
                // Preserve the upstream command's trailing space explicitly.
                trailing_space = " ",
                ownership = ownership,
            ).into()
        }),
        pg.ensure_users(),
    ]);

    nix_text!(
        r#"
            PSQL="psql --port={port}"

            while ! $PSQL -d postgres -c "" 2> /dev/null; do
                if ! kill -0 "$MAINPID"; then exit 1; fi
                sleep 0.1
            done

            if test -e "{data}/.first_startup"; then
              {initial}
              rm -f "{data}/.first_startup"
            fi
            {databases}{users}
        "#,
        port = pg.settings.port(),
        data = data,
        initial = initial,
        databases = NixValue::if_else(
            pg.ensure_databases().equals(NixValue::list([])),
            "",
            nix_text!("{databases}\n", databases = databases),
        ),
        users = users,
    )
    .into()
}

// Maps the service's lifecycle and hardening policy to systemd property names.
#[derive(IntoRusixValue)]
#[rusix(rename_all = "PascalCase")]
struct ServiceConfig {
    // Runs the database process as the dedicated Unix account.
    user: &'static str,
    // Shares database files through the dedicated Unix group.
    group: &'static str,

    // Selects notification support from the final PostgreSQL package version.
    r#type: NixValue,
    // Starts PostgreSQL from the package selected after NixOS combines all definitions.
    exec_start: NixValue,
    // Signals the running server to reread its configuration.
    exec_reload: NixValue,
    // Requests PostgreSQL fast shutdown rather than terminating transactions abruptly.
    kill_signal: &'static str,
    // Lets PostgreSQL shut down its children before systemd intervenes.
    kill_mode: &'static str,
    // Bounds the time systemd allows for startup and shutdown.
    timeout_sec: i64,

    // Asks systemd to manage the PostgreSQL directory under /run.
    runtime_directory: &'static str,
    // Gives the service its own temporary directories.
    private_tmp: bool,
    // Hides users’ home directories from the service.
    protect_home: bool,
    // Makes system paths read-only except for explicitly permitted locations.
    protect_system: &'static str,
    // Chooses file permissions according to package support for group access.
    #[rusix(rename = "UMask")]
    umask: NixValue,

    // Drops all Linux capabilities; systemd spells the empty set as an empty string.
    capability_bounding_set: Vec<&'static str>,
    // Denies access to devices except those allowed by systemd.
    device_policy: &'static str,
    // Defaults to rejecting writable executable memory when final settings disable JIT.
    memory_deny_write_execute: NixValue,
    // Prevents the service from gaining privileges through exec.
    no_new_privileges: bool,
    // Prevents changes to the process execution personality.
    lock_personality: bool,
    // Gives the service a restricted private device namespace.
    private_devices: bool,
    // Isolates mount changes from the rest of the system.
    private_mounts: bool,
    // Limits /proc to process information.
    proc_subset: &'static str,
    // Prevents changes to the system clock.
    protect_clock: bool,
    // Makes control-group interfaces read-only.
    protect_control_groups: bool,
    // Prevents changes to the system hostname.
    protect_hostname: bool,
    // Blocks access to kernel logs.
    protect_kernel_logs: bool,
    // Prevents loading or unloading kernel modules.
    protect_kernel_modules: bool,
    // Makes kernel tuning interfaces read-only.
    protect_kernel_tunables: bool,
    // Hides other users’ processes from the service.
    protect_proc: &'static str,
    // Removes IPC objects owned by the service account when it stops.
    #[rusix(rename = "RemoveIPC")]
    remove_ipc: bool,

    // Allows only the socket families used by PostgreSQL and service management.
    restrict_address_families: Vec<&'static str>,
    // Prevents creating additional namespaces.
    restrict_namespaces: bool,
    // Prevents requesting real-time scheduling.
    restrict_realtime: bool,
    // Prevents creating files with set-user-ID or set-group-ID bits.
    #[rusix(rename = "RestrictSUIDSGID")]
    restrict_suid_sgid: bool,
    // Accepts only the host’s native syscall ABI.
    system_call_architectures: &'static str,
    // Retains upstream syscall allow/deny groups and their ordering.
    system_call_filter: Vec<&'static str>,
}

// The NixOS service record contains deferred scripts and merged systemd properties.
#[derive(IntoRusixValue)]
struct ServiceDefinition {
    // Names the service in systemd status output.
    description: &'static str,
    // Starts the service with the normal multi-user target.
    wanted_by: Vec<&'static str>,
    // Preserves upstream ordering after network setup.
    after: Vec<&'static str>,
    // Passes the final data directory as PGDATA without reading it in Rust.
    environment: NixValue,
    // Makes the final PostgreSQL package and extensions available to generated scripts.
    path: Vec<NixValue>,
    // Initializes the data directory and installs generated configuration links.
    pre_start: NixValue,
    // Waits for readiness, then runs initialization and provisioning SQL.
    post_start: NixValue,
    // Carries the merged systemd properties, including conditional directory management.
    service_config: NixValue,
    // Keeps mount dependencies tied to the final data directory.
    unit_config: NixValue,
}

// Supplies upstream defaults, while settings themselves remain an open record.
#[derive(IntoRusixValue)]
struct PostgresqlDefaults {
    // Adds generated file locations and defaults to the open PostgreSQL settings map.
    settings: NixValue,
    // Supplies the state-version-dependent package at NixOS default priority.
    package: NixValue,
    // Defaults the data directory from the final package’s database schema.
    data_dir: NixValue,
    // Combines the upstream header and fallback rules using normal NixOS ordering.
    authentication: NixValue,
}

// The operating-system account is separate from PostgreSQL's database roles.
#[derive(IntoRusixValue)]
struct UnixUser {
    // Declares the operating-system account independently of database roles.
    name: &'static str,
    // Refers to the final NixOS allocation for the PostgreSQL Unix user.
    uid: Expr<i64>,
    // Places the account in the dedicated PostgreSQL Unix group.
    group: &'static str,
    // Labels the account in the generated user database.
    description: &'static str,
    // Uses the final data directory, retaining its Nix string dependency context.
    home: NixValue,
    // Retains the upstream account’s access to the default login shell.
    use_default_shell: bool,
}

// Account names remain keys in open maps; the containing NixOS namespaces are fixed.
#[derive(IntoRusixValue)]
struct UserDefinitions {
    // Maps account names to Unix user definitions without closing the NixOS namespace.
    users: NixValue,
    // Maps group names to definitions whose IDs are resolved by NixOS.
    groups: NixValue,
}

// Exposes PostgreSQL's package and shared files through the normal NixOS environment.
#[derive(IntoRusixValue)]
struct Environment {
    // Adds the effective PostgreSQL package, including selected extensions.
    system_packages: Vec<NixValue>,
    // Exposes PostgreSQL’s shared files in the system environment.
    paths_to_link: Vec<&'static str>,
}

// These fixed records have no fallible flattening. Keep them atomic when applying
// NixOS wrappers instead of turning their fields into separate option bindings.
#[track_caller]
fn opaque(value: impl IntoRusixValue) -> NixValue {
    value
        .try_into_nix_value()
        .expect("fixed compatibility record has no structural flattening errors")
}

fn assertions() -> NixValue {
    let pg = options::root().services.postgresql;

    // Rust's owned form guarantees matching names, but ordinary Nix contributors
    // can still violate the invariant; retain the upstream assertion for those inputs.
    Nixpkgs::new().function("map").apply([
        NixValue::function(|user| {
            let name = user.clone().select("name");

            nixos::assertion(
                user.select("ensureDBOwnership").implies(
                    Nixpkgs::new()
                        .function("elem")
                        .apply([name.clone(), pg.ensure_databases()]),
                ),
                nix_text!(
                    r#"
                        For each database user defined with `services.postgresql.ensureUsers` and
                        `ensureDBOwnership = true;`, a database with the same name must be defined
                        in `services.postgresql.ensureDatabases`.

                        Offender: {name} has not been found among databases.
                    "#,
                    name = name,
                ),
            )
        }),
        pg.ensure_users(),
    ])
}

fn service_config() -> NixValue {
    let pg = options::root().services.postgresql;
    let package = effective_package();
    let group_access = Nixpkgs::new().library().version_at_least(
        package.clone().select("version").into_expr::<String>(),
        "11.0",
    );

    let data = NixValue::from(pg.data_dir());
    let standard_data = nix_text!(
        "/var/lib/postgresql/{schema}",
        schema = pg.package().select("psqlSchema")
    );

    let properties = opaque(ServiceConfig {
        user: "postgres",
        group: "postgres",

        r#type: NixValue::if_else(
            Nixpkgs::new()
                .library()
                .version_at_least(pg.package().select("version").into_expr::<String>(), "9.6"),
            "notify",
            "simple",
        ),
        exec_start: nix_text!("{package}/bin/postgres", package = package).into(),
        exec_reload: nix_text!(
            "{coreutils}/bin/kill -HUP $MAINPID",
            coreutils = Nixpkgs::from_module().get("coreutils"),
        )
        .into(),
        kill_signal: "SIGINT",
        kill_mode: "mixed",
        timeout_sec: 120,

        runtime_directory: "postgresql",
        private_tmp: true,
        protect_home: true,
        protect_system: "strict",
        umask: NixValue::if_else(group_access.clone(), "0027", "0077"),

        capability_bounding_set: vec![""],
        device_policy: "closed",
        memory_deny_write_execute: NixValue::from(pg.settings.jit())
            .equals("off")
            .priority(DefinitionPriority::Default),
        no_new_privileges: true,
        lock_personality: true,
        private_devices: true,
        private_mounts: true,
        proc_subset: "pid",
        protect_clock: true,
        protect_control_groups: true,
        protect_hostname: true,
        protect_kernel_logs: true,
        protect_kernel_modules: true,
        protect_kernel_tunables: true,
        protect_proc: "invisible",
        remove_ipc: true,

        restrict_address_families: vec!["AF_INET", "AF_INET6", "AF_NETLINK", "AF_UNIX"],
        restrict_namespaces: true,
        restrict_realtime: true,
        restrict_suid_sgid: true,
        system_call_architectures: "native",
        system_call_filter: vec!["@system-service", "~@privileged @resources"],
    });

    // Data paths matching the package-schema default get StateDirectory management. Other
    // directories retain upstream ReadWritePaths behavior and ownership responsibility.
    nixos::merge([
        properties,
        record! { "ReadWritePaths": NixValue::list([data.clone()]) }.when(NixValue::if_else(
            data.clone().equals("/var/lib/postgresql"),
            false,
            true,
        )),
        record! {
            "StateDirectory": nix_text!(
                "postgresql postgresql/{schema}",
                schema = pg.package().select("psqlSchema"),
            ),
            "StateDirectoryMode": NixValue::if_else(group_access, "0750", "0700"),
        }
        .when(data.equals(standard_data)),
    ])
}

fn service() -> NixValue {
    let pg = options::root().services.postgresql;

    opaque(ServiceDefinition {
        description: "PostgreSQL Server",
        wanted_by: vec!["multi-user.target"],
        after: vec!["network.target"],

        environment: record! { "PGDATA": pg.data_dir() },
        path: vec![effective_package()],

        pre_start: pre_start(),
        post_start: post_start(),
        service_config: service_config(),
        unit_config: record! { "RequiresMountsFor": path_text(pg.data_dir()) },
    })
}

fn settings() -> NixValue {
    let pg = options::root().services.postgresql;

    // PostgreSQL owns this open namespace, including its literal snake_case keys.
    record! {
        "hba_file": Nixpkgs::from_module()
            .pkgs_function("writeText")
            .apply(["pg_hba.conf".into(), pg.authentication().into()])
            .to_text(),
        "ident_file": Nixpkgs::from_module()
            .pkgs_function("writeText")
            .apply(["pg_ident.conf".into(), pg.ident_map().into()])
            .to_text(),
        "log_destination": "stderr",
        "listen_addresses": NixValue::if_else(pg.enable_tcpip(), "*", "localhost"),
        "jit": NixValue::if_else(pg.enable_jit(), "on", "off")
            .priority(DefinitionPriority::Default),
    }
}

fn checks() -> NixValue {
    let pg = options::root().services.postgresql;

    let check = Nixpkgs::from_module().pkgs_function("runCommand").apply([
        "postgresql-configfile-check".into(),
        record! {},
        nix_text!(
            r#"
                    {package}/bin/postgres -D{config_file} -C config_file >/dev/null
                    touch $out
                "#,
            package = pg.package(),
            config_file = configuration_file(),
        )
        .into(),
    ]);

    let pkgs = Nixpkgs::from_module();
    // Upstream includes this executable check only when build and host values compare equal.
    let build_host_equal = package::build_host_equal(pkgs.value("stdenv"));
    let enabled = NixValue::if_else(pg.check_config(), build_host_equal, false);

    Nixpkgs::new()
        .library()
        .optional(enabled.into_expr::<bool>(), check)
        .into()
}

/// Defines the NixOS contributions for author-supplied inputs and computed PostgreSQL settings.
/// Their nested fields place defaults, generated files and service definitions at NixOS option paths.
#[rusix::config]
mod config {
    use super::*;

    // Places the author's optional input record at the existing PostgreSQL option path.
    #[rusix(root)]
    pub(super) struct Inputs {
        /// Places the optional Rust inputs under the existing NixOS services namespace.
        services: InputServices,
    }

    /// Author-supplied choices grouped under the NixOS service namespace.
    struct InputServices {
        /// PostgreSQL choices contributed before NixOS resolves defaults and other modules.
        postgresql: PostgresqlInput,
    }

    // Struct-level omission affects only this record's direct optional fields.
    #[rusix(omit_none)]
    pub(super) struct PostgresqlInput {
        // Always contribute enable; the remaining scalars leave upstream defaults intact when unset.
        pub(super) enable: bool,
        // Selects an existing package only when the author supplies one.
        pub(super) package: Option<PackageRef>,
        #[rusix(rename = "enableJIT")]
        // Leaves the package-specific JIT default intact when unset.
        pub(super) enable_jit: Option<bool>,
        #[rusix(rename = "enableTCPIP")]
        // Contributes a TCP-listening choice only when requested.
        pub(super) enable_tcpip: Option<bool>,
        // Optionally overrides inclusion of the configuration check.
        pub(super) check_config: Option<bool>,
        // Overrides the final data-directory definition without fixing dependent paths.
        pub(super) data_dir: Option<String>,
        // Adds author-supplied authentication text to the normal NixOS merge.
        pub(super) authentication: Option<String>,
        // Defines ident-map content only when supplied.
        pub(super) ident_map: Option<String>,
        // Retains the SQL-file reference for Nix to resolve; PostgreSQL applies it at initialization.
        pub(super) initial_script: Option<NixValue>,
        // Contributes optional recovery content rather than a null definition.
        pub(super) recovery_config: Option<String>,

        // The adapter explicitly decides whether empty collections should contribute.
        // Carries the callback that selects extensions from the final package set.
        pub(super) extra_plugins: Option<NixValue>,
        // Keeps arbitrary PostgreSQL setting names literal in the Nix attribute set.
        pub(super) settings: Option<NixValue>,
        // An absent list preserves upstream initialization defaults.
        pub(super) initdb_args: Option<Vec<String>>,
        // Keeps the author-derived database provisioning order.
        pub(super) ensure_databases: Option<Vec<NixValue>>,
        // Includes ownership-derived roles followed by explicitly declared roles.
        pub(super) ensure_users: Option<Vec<NixValue>>,
    }

    // The pinned role schema is finite; null inside Some remains a deliberate SQL preserve value.
    #[rusix(omit_none)]
    pub(super) struct RoleClauseInputs {
        // Contributes the requested SUPERUSER state, including explicit null to preserve it.
        pub(super) superuser: Option<NixValue>,
        // Contributes role-management privileges only when specified.
        pub(super) createrole: Option<NixValue>,
        // Contributes database-creation privileges only when specified.
        pub(super) createdb: Option<NixValue>,
        // Contributes privilege-inheritance behavior only when specified.
        pub(super) inherit: Option<NixValue>,
        // Contributes login permission only when specified.
        pub(super) login: Option<NixValue>,
        // Contributes replication privileges only when specified.
        pub(super) replication: Option<NixValue>,
        // Contributes row-security bypass behavior only when specified.
        pub(super) bypassrls: Option<NixValue>,
    }

    /// PostgreSQL defaults and authentication rules computed from the final options.
    struct Services {
        /// Computed PostgreSQL defaults and authentication rules contributed to services.postgresql.
        postgresql: NixValue,
    }

    // A separate contribution deriving service behavior from final merged options.
    #[rusix(root)]
    pub(super) struct Implementation {
        /// Deferred configuration rules NixOS checks when the PostgreSQL service is enabled.
        assertions: NixValue,
        /// PostgreSQL defaults derived from the final merged option values.
        services: Services,
        /// Unix account and group definitions used to own the database directory and process.
        users: NixValue,
        /// PostgreSQL packages and shared files included in the system environment.
        environment: NixValue,
        /// Build-time configuration checks described for NixOS; Rust does not execute them.
        system: NixValue,
        /// Service-unit commands, dependencies and hardening settings derived from PostgreSQL options.
        systemd: NixValue,
    }

    pub(super) fn inputs(postgresql: PostgresqlInput) -> Inputs {
        Inputs {
            services: InputServices { postgresql },
        }
    }

    pub(super) fn implementation() -> Implementation {
        let pg = options::root().services.postgresql;
        let enabled = pg.enable();
        let guarded = |value: NixValue| value.when(enabled.clone());

        // Normal rules go between the header and fallback rules; mkForce can replace all.
        let authentication = nixos::merge([
            NixValue::from("# Generated file; do not edit!").before(),
            NixValue::from("# default value of services.postgresql.authentication\nlocal all all              peer\nhost  all all 127.0.0.1/32 md5\nhost  all all ::1/128      md5\n").after(),
        ]);

        let postgres_user = opaque(UnixUser {
            name: "postgres",
            uid: options::root().ids.uids.postgres(),
            group: "postgres",
            description: "PostgreSQL server user",
            home: path_text(pg.data_dir()),
            use_default_shell: true,
        });

        Implementation {
            assertions: guarded(assertions()),
            services: Services {
                postgresql: guarded(opaque(PostgresqlDefaults {
                    settings: settings(),
                    package: default_package().priority(DefinitionPriority::Default),
                    data_dir: NixValue::from(nix_text!(
                        "/var/lib/postgresql/{schema}",
                        schema = pg.package().select("psqlSchema"),
                    ))
                    .priority(DefinitionPriority::Default),
                    authentication,
                })),
            },
            users: guarded(opaque(UserDefinitions {
                users: record! { "postgres": postgres_user },
                groups: record! {
                    "postgres": record! { "gid": options::root().ids.gids.postgres() },
                },
            })),
            environment: guarded(opaque(Environment {
                system_packages: vec![effective_package()],
                paths_to_link: vec!["/share/postgresql"],
            })),
            system: guarded(record! { "checks": checks() }),
            systemd: guarded(record! {
                "services": record! { "postgresql": service() },
            }),
        }
    }
}

/// Supplies configuration-generation definitions without importing the upstream implementation.
/// schema.rs separately supplies the public option declarations and migration imports.
pub(crate) fn implementation() -> NixosModule {
    NixosModule::empty().add(config::implementation())
}
