//! Refer to PostgreSQL options after NixOS combines definitions from all modules.
//! Accessors describe deferred `config.*` lookups; they do not declare options or read values in Rust.

/// Declares accessors for the final NixOS settings used by the PostgreSQL implementation.
/// References follow config.* after merging, including defaults and choices from other modules.
#[rusix::options]
mod references {
    use rusix::interop::raw::NixValue;

    /// The parts of the final NixOS configuration needed to configure PostgreSQL.
    #[rusix(root)]
    struct Root {
        /// Service options after NixOS combines the Rust and ordinary Nix contributions.
        services: Services,
        /// System compatibility settings used to choose PostgreSQL defaults.
        system: System,
        /// Operating-system user and group identifiers assigned by NixOS.
        ids: Ids,
    }

    /// Service options supplied by NixOS modules.
    struct Services {
        /// Final PostgreSQL service options, including values supplied by other modules.
        postgresql: Postgresql,
    }

    /// Final PostgreSQL choices used to generate files and service commands.
    struct Postgresql {
        /// Whether to include the database service and its supporting configuration.
        enable: bool,
        /// Whether the selected package should support just-in-time query compilation.
        #[rusix(rename = "enableJIT")]
        enable_jit: bool,
        /// Whether the server should listen for network connections as well as local sockets.
        #[rusix(rename = "enableTCPIP")]
        enable_tcpip: bool,
        /// Whether NixOS should include the generated PostgreSQL configuration check.
        check_config: bool,
        /// Directory where the server stores its databases and configuration files.
        data_dir: String,
        /// Client authentication rules written to pg_hba.conf after NixOS text merging.
        authentication: String,
        /// Operating-system-to-database user mappings written to pg_ident.conf.
        ident_map: String,
        /// Bootstrap database role used to initialize and manage the cluster, normally postgres.
        super_user: String,
        /// Selected PostgreSQL package; lookups describe its fields without building it.
        package: NixValue,
        /// Server settings rendered into postgresql.conf after other modules can override them.
        settings: Settings,
        /// Function choosing extension packages from the selected PostgreSQL package's package set.
        extra_plugins: NixValue,
        /// Flags passed to initdb when creating a new database directory.
        initdb_args: Vec<String>,
        /// Optional SQL file applied when the database cluster is first initialized.
        initial_script: Option<NixValue>,
        /// Optional recovery.conf contents retained for older PostgreSQL versions.
        recovery_config: Option<String>,
        /// Database names to provision when the service starts, in the requested order.
        ensure_databases: Vec<String>,
        /// Database roles and requested privileges to provision when the service starts.
        ensure_users: NixValue,
    }

    /// Server settings may have arbitrary keys; the value accessor retains the whole Nix attribute set.
    #[rusix(value)]
    struct Settings {
        /// Final listening port used by generated configuration and startup commands.
        port: i64,
        /// Runtime query-compilation setting rendered as text, such as on or off.
        jit: String,
    }

    /// Compatibility settings used to choose defaults for an existing NixOS installation.
    struct System {
        /// NixOS compatibility release used to retain existing installation defaults.
        state_version: String,
    }

    /// Numeric Unix account identifiers assigned by NixOS.
    struct Ids {
        /// Numeric Unix user IDs, including the dedicated PostgreSQL account.
        uids: PostgresIds,
        /// Numeric Unix group IDs, including the dedicated PostgreSQL group.
        gids: PostgresIds,
    }

    /// The PostgreSQL identity within either the user-ID or group-ID namespace.
    struct PostgresIds {
        /// PostgreSQL user or group ID, depending on the parent uids or gids namespace.
        postgres: i64,
    }
}

pub(super) use references::root;
