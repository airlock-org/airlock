//! User-defined PostgreSQL configuration types and an example server configuration.
//! Package references and arbitrary server settings can stay deferred to Nix; lowering.rs handles NixOS compatibility.
use rusix::interop::{PackageRef, raw::NixValue};
use std::collections::BTreeMap;

/// Models role-clause intent using ordinary Rust; Rusix core knows nothing about PostgreSQL.
pub enum Clause {
    /// Leave this clause unchanged on an existing role; new roles use PostgreSQL's default.
    Preserve,
    /// Emit the positive clause in provisioning SQL, such as LOGIN.
    Enable,
    /// Emit its negative clause, such as NOLOGIN, rather than simply omitting it.
    Disable,
}

/// The seven role clauses supported by the pinned NixOS schema.
/// None omits a definition; Some(Preserve) explicitly contributes null, which skips that SQL clause.
#[derive(Default)]
pub struct RoleClauses {
    /// Controls superuser status through SUPERUSER or NOSUPERUSER.
    pub superuser: Option<Clause>,
    /// Controls permission to create and manage other roles.
    pub createrole: Option<Clause>,
    /// Controls permission to create databases.
    pub createdb: Option<Clause>,
    /// Controls whether privileges of granted roles are inherited automatically.
    pub inherit: Option<Clause>,
    /// Controls whether the role can log in as a database user.
    pub login: Option<Clause>,
    /// Controls replication-role privileges.
    pub replication: Option<Clause>,
    /// Controls whether the role bypasses row-level security policies.
    pub bypassrls: Option<Clause>,
}

/// An additional database role, independent of the owned databases listed below.
pub struct Role {
    /// The role ensured by startup SQL; no same-named database is implied.
    pub name: String,
    /// Explicit changes to apply after ensuring the role exists.
    pub clauses: RoleClauses,
}

/// A provisioning choice: ensure a database alone, or also ensure its matching owner role.
pub enum Database {
    /// Ensure this named database exists without explicitly assigning its ownership.
    Unowned(String),
    /// Ensure a database and same-named owner; this shape prevents their names from diverging.
    Owned {
        /// One name used for both the database and the role that will own it.
        name: String,
        /// Role-clause changes for the generated owner role.
        clauses: RoleClauses,
    },
}

/// Rust inputs for this example's PostgreSQL component, not built-in Rusix domain types.
/// Unset optional fields and empty collections contribute no definitions; NixOS defaults still apply.
#[derive(Default)]
pub struct Postgresql {
    /// Defines the existing NixOS enable option, gating the generated service behavior.
    pub enable: bool,
    /// Selects an existing nixpkgs package; Nix resolves its version and build recipe.
    pub package: Option<PackageRef>,
    /// Requests the package's JIT variant; nixpkgs remains authoritative for its implementation.
    pub enable_jit: Option<bool>,
    /// Controls the generated TCP listening-address setting.
    pub enable_tcpip: Option<bool>,
    /// Controls inclusion of the upstream configuration-check derivation; this example builds nothing.
    pub check_config: Option<bool>,
    /// Extension attribute paths selected from the final PostgreSQL package's package set in Nix.
    pub extensions: Vec<String>,
    /// Overrides the data-directory definition; dependent paths follow the final NixOS value.
    pub data_dir: Option<String>,
    /// Arbitrary setting keys with mixed literal or deferred values; Nix/PostgreSQL validate them.
    pub settings: BTreeMap<String, NixValue>,
    /// Adds pg_hba.conf rules before upstream defaults; ordinary Nix mkForce can replace them.
    pub authentication: Option<String>,
    /// Supplies pg_ident.conf mappings as text interpreted by PostgreSQL.
    pub ident_map: Option<String>,
    /// Extra initdb arguments, shell-escaped by the existing Nix library.
    pub initdb_args: Vec<String>,
    /// SQL-file path, possibly supplied by a package output, applied only when initializing the cluster.
    pub initial_script: Option<NixValue>,
    /// Optional recovery.conf content, retained for compatibility with the pinned module.
    pub recovery_config: Option<String>,
    /// Ordered databases to ensure at startup; the owned alternative also generates matching roles.
    pub databases: Vec<Database>,
    /// Additional roles to ensure after the ownership-derived roles.
    pub roles: Vec<Role>,
}

/// Configure an enabled server and an app database with a same-named login role.
/// The compatibility adapter keeps dependent outputs symbolic for ordinary NixOS overrides.
pub fn model() -> Postgresql {
    Postgresql {
        enable: true,
        settings: BTreeMap::from([("max_connections".into(), 100_i64.into())]),
        databases: vec![Database::Owned {
            name: "app".into(),
            clauses: RoleClauses {
                login: Some(Clause::Enable),
                superuser: Some(Clause::Disable),
                ..RoleClauses::default()
            },
        }],
        ..Postgresql::default()
    }
}
