//! Versioned requests select declared evidence sources rather than commands or paths.

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, str::FromStr};

/// Existing Qubes transport service; service arguments are always empty.
pub const SERVICE: &str = "airlock.Diagnostics";

/// Maximum serialized request size, including the discovery operation.
pub const REQUEST_LIMIT: usize = 8192;

/// Hard upper bound on combined evidence bytes for a single read.
pub const OUTPUT_LIMIT: u64 = 1024 * 1024;

/// Exact installed contract version.
pub const CONFIG_SCHEMA: &str = "airlock.diagnostics-config.v1";

/// Exact query wire version.
pub const REQUEST_SCHEMA: &str = "airlock.diagnostics-request.v1";

/// Exact response wire version.
pub const RESPONSE_SCHEMA: &str = "airlock.diagnostics-response.v1";

/// Validate opaque identifiers before using them in transport, journal filters or audit records.
fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte)),
        "invalid diagnostic identity"
    );
    Ok(())
}

/// Separate names at the Rust and deserialization boundaries without accepting raw strings.
macro_rules! identity {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(
            /// Validated identity without transport argument or shell syntax.
            String,
        );

        impl $name {
            /// Borrow the validated external name.
            pub fn as_str(&self) -> &str { &self.0 }
        }

        impl TryFrom<String> for $name {
            type Error = anyhow::Error;

            fn try_from(value: String) -> Result<Self> {
                identifier(&value)?;
                Ok(Self(value))
            }
        }

        impl FromStr for $name {
            type Err = anyhow::Error;

            fn from_str(value: &str) -> Result<Self> { Self::try_from(value.to_owned()) }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self { value.0 }
        }

        impl fmt::Display for $name {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result { out.write_str(&self.0) }
        }
    };
}

identity!(/// VM or dom0 endpoint selected independently of the authenticated caller.
    TargetId);

identity!(/// Authenticated Qubes identity; wire requests cannot supply this field.
    CallerId);

identity!(/// Source name resolved only through the endpoint's installed inventory.
    SourceId);

identity!(/// Correlation name for one request, without granting authority by itself.
    RequestId);

/// Kernel boot identity, distinct from request, source and endpoint names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BootId(
    /// Canonical UUID read from the endpoint's kernel.
    String,
);

impl BootId {
    /// Borrow the canonical UUID used to bind collection to one endpoint boot.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BootId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        ensure!(valid_boot(&value), "invalid boot identity");
        Ok(Self(value))
    }
}

impl FromStr for BootId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value.to_owned())
    }
}

impl From<BootId> for String {
    fn from(value: BootId) -> Self {
        value.0
    }
}

/// A service unit filter, excluding options, paths, wildcard patterns and journal expressions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UnitName(
    /// Exact systemd service identity, including its suffix.
    String,
);

impl UnitName {
    /// Borrow the single exact unit name used as a journal filter.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for UnitName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        ensure!(
            value.len() <= 255
                && value.ends_with(".service")
                && value.as_bytes()[0].is_ascii_alphanumeric()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.@:-\\".contains(&byte)),
            "expected an exact service unit name"
        );
        Ok(Self(value))
    }
}

impl FromStr for UnitName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value.to_owned())
    }
}

impl From<UnitName> for String {
    fn from(value: UnitName) -> String {
        value.0
    }
}

/// Installed decision mode; clients cannot choose or override this setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    /// Explicit initial policy, reported honestly as automatic approval.
    Automatic,
    /// Refuse evidence and inventory requests until access is configured.
    Deny,
}

/// Fixed read-only operations; no variant accepts arbitrary argv or a shell command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Probe {
    /// Discover installed system services and their states.
    Services,
    /// Inspect PCI devices and driver bindings.
    Pci,
    /// Inspect link and assigned-address state.
    Addresses,
    /// Inspect routing tables without changing them.
    Routes,
    /// Inspect mounted filesystems.
    Mounts,
    /// Inspect Xen's host information, available only in dom0 contracts.
    XenInfo,
    /// Inspect Xen's retained messages without clearing them.
    XenLog,
    /// Inspect Qubes VM state without starting any VM.
    Qubes,
}

/// A package-authored evidence reader; file paths never come from a query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reader {
    /// System journal for the current boot, optionally narrowed to one unit.
    Journal {},
    /// User-service journal entries for a fixed installed UID.
    UserJournal {
        /// Local account whose service journal may be read.
        uid: u32,
    },
    /// Kernel journal entries for the current boot.
    Kernel {},
    /// Read one predeclared regular file without following symlinks.
    File {
        /// Absolute installed path; mutable parents cannot redirect traversal through symlinks.
        path: String,
        /// Select the newest bytes for growing logs; false reads configuration from its beginning.
        tail: bool,
        /// Account owning this evidence; substituted files from other accounts are rejected.
        owner: u32,
    },
    /// Read a declared NixOS configuration file only when it resolves into the immutable store.
    StoreFile {
        /// Installed configuration link, never a caller-supplied path.
        path: String,
    },
    /// Invoke one closed read-only probe.
    Probe {
        /// Fixed operation selected by component authoring.
        probe: Probe,
    },
}

/// Discoverable evidence declaration owned by a component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Stable name selected by read requests.
    pub id: SourceId,
    /// Human explanation for the future approval view, never used as a command.
    pub description: String,
    /// Exact reader selected by the installed contract.
    pub reader: Reader,
}

/// Frozen per-endpoint scope generated with the matching Qubes policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// Installed protocol version, rejected when unknown.
    pub schema: String,
    /// Exact identity served by this endpoint.
    pub target: TargetId,
    /// Qubes-authenticated clients with access to this endpoint.
    pub callers: Vec<CallerId>,
    /// Trusted decision mode, independent of all incoming fields.
    pub approval: ApprovalMode,
    /// Output ceiling; requests above it are rejected rather than silently widened.
    pub max_bytes: u64,
    /// Declared evidence inventory; duplicate identities are invalid.
    pub sources: Vec<Source>,
}

impl ServerConfig {
    /// Reject unknown schemas, duplicate sources and unsafe file declarations.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == CONFIG_SCHEMA,
            "unknown diagnostics contract schema"
        );
        ensure!(
            (1..=OUTPUT_LIMIT).contains(&self.max_bytes),
            "invalid output ceiling"
        );
        ensure!(
            self.callers.len() <= 64 && self.sources.len() <= 128,
            "contract inventory too large"
        );
        let mut ids = std::collections::BTreeSet::new();
        for source in &self.sources {
            ensure!(ids.insert(&source.id), "duplicate diagnostic source");
            ensure!(
                source.description.len() <= 1024,
                "source description too long"
            );
            if let Reader::File { path, .. } | Reader::StoreFile { path } = &source.reader {
                ensure!(
                    !path.as_bytes().contains(&0)
                        && path.len() <= 4096
                        && Path::new(path).is_absolute()
                        && Path::new(path)
                            .components()
                            .skip(1)
                            .all(|part| matches!(part, std::path::Component::Normal(_))),
                    "source file must have an absolute traversal-free path"
                );
            }
        }
        Ok(())
    }
}

/// Parameters retained in the exact approval binding and never interpreted as argv.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadOptions {
    /// Combined stdout and stderr budget, bounded by the installed endpoint.
    pub max_bytes: u64,
    /// Maximum journal tail length, between one and two thousand entries.
    pub lines: u32,
    /// Optional exact service filter, accepted only by journal readers.
    pub unit: Option<UnitName>,
    /// Optional boot identity previously observed by discovery; stale requests fail.
    pub boot: Option<BootId>,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            max_bytes: 65536,
            lines: 200,
            unit: None,
            boot: None,
        }
    }
}

/// Operations available through the single diagnostic RPC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Discover evidence sources after the same caller and approval checks as reading.
    Catalog {},
    /// Read exactly one source with bounded parameters.
    Read {
        /// Source selected from the installed inventory.
        source: SourceId,
        /// Limits and filters included in the approval binding.
        options: ReadOptions,
    },
}

/// Untrusted request, intentionally lacking caller or approval fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// Exact supported request version.
    pub schema: String,
    /// Caller-selected correlation identity, never an authorization token.
    pub id: RequestId,
    /// Endpoint requested by the client, checked independently on the target.
    pub target: TargetId,
    /// Short explanation available to a future approval provider.
    pub reason: String,
    /// Catalog or read request with no command-execution alternative.
    pub action: Operation,
}

impl Query {
    /// Reject invalid schema, excessive text, out-of-range limits and malformed boot identities.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == REQUEST_SCHEMA,
            "unknown diagnostics request schema"
        );
        ensure!(
            !self.reason.trim().is_empty() && self.reason.len() <= 1024,
            "a bounded investigation reason is required"
        );
        if let Operation::Read { options, .. } = &self.action {
            ensure!(
                (1..=OUTPUT_LIMIT).contains(&options.max_bytes),
                "invalid read budget"
            );
            ensure!(
                (1..=2000).contains(&options.lines),
                "invalid journal tail length"
            );
        }
        Ok(())
    }
}

/// Check Linux's canonical boot UUID without accepting journal expressions.
pub(crate) fn valid_boot(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

/// Evidence outcome; absence and failure never imply a healthy component.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The selected reader finished successfully within its output budget.
    Complete,
    /// Output reached its byte limit and the reader was stopped.
    Truncated,
    /// Caller, source or installed approval policy refused the request.
    Denied,
    /// A source or required local facility was unavailable.
    Unavailable,
    /// The reader or request validation failed; no successful observation is claimed.
    Failed,
    /// The configured collection deadline expired.
    TimedOut,
    /// Boot identity changed or differs from the query's expected boot.
    StaleBoot,
}

/// Returned evidence is untrusted data and never grants change authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// Supported diagnostic response version.
    pub schema: String,
    /// Exact request correlation identity.
    pub id: RequestId,
    /// Installed target, checked against the transport destination by the client.
    pub target: TargetId,
    /// Exact operation, filters and bounds, preserved so saved evidence retains its query context.
    pub action: Operation,
    /// Kernel boot UUID, absent only when collection could not establish it.
    pub boot: Option<BootId>,
    /// Collection start as Unix milliseconds; an observation, not an authorization clock.
    pub started_ms: u64,
    /// Collection end as Unix milliseconds.
    pub finished_ms: u64,
    /// Honest decision provenance; never implies a human approved an automatic request.
    pub approval: ApprovalMode,
    /// Explicit collection outcome.
    pub outcome: Outcome,
    /// Declared sources, populated only for a successful catalog operation.
    pub sources: Vec<Source>,
    /// Bounded standard output or file contents, decoded for JSON display.
    pub stdout: String,
    /// Bounded reader diagnostic output or collection error.
    pub stderr: String,
    /// Whether invalid UTF-8 was replaced while representing evidence as text.
    pub lossy: bool,
}

/// Decode and validate one bounded query before evaluating installed authority.
pub fn decode_query(bytes: &[u8]) -> Result<Query> {
    if bytes.len() > REQUEST_LIMIT {
        bail!("diagnostic request exceeds 8 KiB");
    }
    let query: Query = serde_json::from_slice(bytes)?;
    query.validate()?;
    Ok(query)
}
