//! Convert user-defined Rust data into configuration fields and Nix values.
use crate::interop::raw::AsNixValue;
use crate::{Assignment, Config, ConfigValue, Node, Origin, ValidationError, ValueKind};

/// Convert a Rust value into data Rusix can use inside a configuration tree.
/// A nested value defines its fields, while its parent determines where they
/// appear. For example, the same endpoint type can be nested under several
/// services without containing a global option path.
///
/// Prefer the derive for ordinary structs, newtypes and unit enums. Implement
/// this trait when conversion expresses a domain decision. This is distinct
/// from [`crate::IntoConfig`], which creates a complete configuration contribution.
/// To pass a Rust struct to a Nix function as one record, use
/// [`Self::try_into_nix_value`]; the intermediate [`RusixValue`] is mainly useful
/// when implementing custom structural conversions.
///
/// An ordinary `Option<T>` maps `None` to Nix `null`. Explicit
/// `#[rusix(omit_none)]` changes field behavior so no definition is contributed.
/// On a named struct, it applies only to direct Option fields, not nested types.
pub trait IntoRusixValue {
    /// Convert this Rust value into fields, a list or a value expression.
    /// Conversion happens in Rust, but contained Nix expressions remain unevaluated.
    /// Tracked helpers preserve the author’s call location and existing child locations.
    #[track_caller]
    fn into_value(self) -> RusixValue;

    /// Convert this Rust value into one value that Nix can use later.
    ///
    /// A struct becomes a Nix attribute set (a record of named values), rather
    /// than separate configuration definitions. Use this when passing derived
    /// data to a Nix function. Expressions and package references remain
    /// unevaluated, and field names and Rust source locations are preserved.
    ///
    /// # Errors
    ///
    /// Returns an error if a `#[rusix(flatten)]` field does not produce a
    /// structural record. This can happen even with a derived type: a scalar,
    /// `None`, or an opaque [`crate::interop::raw::NixValue`] cannot be structurally
    /// flattened. Nested fields and list elements can contain the same error.
    /// Duplicate keys and other IR validity checks happen during compilation;
    /// Nix function and NixOS option types are checked later by Nix.
    ///
    /// ```
    /// use rusix::{IntoRusixValue, ValidationError, interop::raw::NixValue};
    ///
    /// #[derive(IntoRusixValue)]
    /// struct FileArguments {
    ///     // Filename supplied as the Nix function argument named name.
    ///     name: String,
    ///     // File contents supplied as the Nix function argument named text.
    ///     text: String,
    /// }
    ///
    /// fn file_arguments() -> Result<NixValue, ValidationError> {
    ///     let arguments = FileArguments {
    ///         name: "example.conf".into(),
    ///         text: "workers = 4\n".into(),
    ///     };
    ///
    ///     Ok(arguments.try_into_nix_value()?)
    /// }
    /// ```
    #[track_caller]
    fn try_into_nix_value(self) -> Result<crate::interop::raw::NixValue, ValidationError>
    where
        Self: Sized,
    {
        self.into_value().into_nix_value()
    }
}

/// Data ready to be placed inside a Rusix configuration tree.
/// It can contain named fields, ordered lists, literals and expressions that Nix
/// will evaluate later. Use this as the result of a custom [`IntoRusixValue`]
/// implementation; it is not evaluated Nix data or raw Nix source.
/// Ordinary derived values can use [`IntoRusixValue::try_into_nix_value`]
/// directly without handling this intermediate representation.
///
/// Structural records become nested configuration paths. Existing Nix handles
/// and expressions stay intact, including their Rust locations. To pass a
/// record as one Nix function argument instead, use [`Self::into_nix_value`].
#[derive(Clone, Debug)]
pub struct RusixValue {
    /// Rust conversion location used when these fields are placed in configuration.
    origin: Origin,
    /// A literal/expression, a group of named fields, or an ordered list of values.
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    Leaf(Box<Node>),
    Record(Vec<(Option<String>, RusixValue)>),
    List(Vec<RusixValue>),
}

impl RusixValue {
    /// Wrap a literal or existing Nix expression as one configuration value.
    /// Expressions and package references keep their captured Rust locations;
    /// concrete literals receive a location when converted and placed.
    #[track_caller]
    pub fn leaf(value: impl ConfigValue) -> Self {
        let origin = Origin::caller("configuration value");
        Self {
            kind: Kind::Leaf(Box::new(value.into_node(origin.clone()))),
            origin,
        }
    }

    /// Build named fields whose configuration paths are chosen by their parent.
    /// For example, a parent’s `endpoint` field adds `endpoint` before these names.
    /// [`Config::from_value`] expands this structural tree into complete settings.
    /// Unlike [`crate::interop::raw::NixValue::record`], it is not kept as one Nix value;
    /// keys still remain literal path segments.
    #[track_caller]
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, Self)>) -> Self {
        Self {
            origin: Origin::caller("configuration record"),
            kind: Kind::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| (Some(name.into()), value))
                    .collect(),
            ),
        }
    }

    // None represents flatten. Normal extensions use record; derive emits this.
    #[doc(hidden)]
    /// Derive implementation hook; `None` flattens a nested record into its parent.
    #[track_caller]
    pub fn __record(fields: Vec<(Option<&str>, Self)>) -> Self {
        Self {
            origin: Origin::caller("configuration record"),
            kind: Kind::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| (name.map(str::to_owned), value))
                    .collect(),
            ),
        }
    }

    /// Keep this data as one Nix value instead of expanding its fields into paths.
    /// Use this for a derived struct passed to a Nix function, or for definitions
    /// wrapped with NixOS conditions and priorities. Expressions remain unevaluated
    /// and keep their Rust source locations. Invalid flattening returns a Rust error;
    /// other validation happens when the configuration is compiled.
    /// For a Rust type implementing [`IntoRusixValue`], prefer its direct
    /// [`IntoRusixValue::try_into_nix_value`] method.
    pub fn into_nix_value(self) -> Result<crate::interop::raw::NixValue, ValidationError> {
        self.resolve(&[])
            .map(opaque_record_nodes)
            .map(crate::interop::raw::NixValue::from_node)
    }

    fn resolve(self, path: &[String]) -> Result<Node, ValidationError> {
        let placed = |purpose: &str| {
            Origin::new(
                &self.origin.file,
                self.origin.line,
                self.origin.column,
                format!("{purpose} {}", display_path(path)),
            )
        };

        let (origin, kind) = match self.kind {
            Kind::Leaf(mut node) => {
                if node.origin == self.origin {
                    node.origin = placed("value of");
                }
                return Ok(*node);
            }
            Kind::List(items) => {
                let mut nodes = Vec::new();

                for (index, item) in items.into_iter().enumerate() {
                    let mut item_path = path.to_vec();
                    item_path.push(format!("[{index}]"));
                    nodes.push(item.resolve(&item_path)?);
                }

                (placed("list at"), ValueKind::List(nodes))
            }
            Kind::Record(fields) => {
                let mut nodes = Vec::new();

                for (name, value) in fields {
                    if let Some(name) = name {
                        let mut child_path = path.to_vec();
                        child_path.push(name.clone());
                        nodes.push((name, value.resolve(&child_path)?));
                    } else {
                        let flattened = value.resolve(path)?;
                        let ValueKind::AttrSet(fields) = flattened.kind else {
                            return Err(ValidationError {
                                origin: flattened.origin,
                                message: "rusix flatten requires a record value".into(),
                            });
                        };

                        nodes.extend(fields);
                    }
                }

                (placed("record at"), ValueKind::AttrSet(nodes))
            }
        };

        Ok(Node { origin, kind })
    }
}

// Convert only structural containers; native deferred nodes stay untouched.
fn opaque_record_nodes(mut node: Node) -> Node {
    node.kind = match node.kind {
        ValueKind::AttrSet(fields) => ValueKind::OpaqueRecord(
            fields
                .into_iter()
                .map(|(name, value)| (name, opaque_record_nodes(value)))
                .collect(),
        ),
        ValueKind::List(items) => {
            ValueKind::List(items.into_iter().map(opaque_record_nodes).collect())
        }
        kind => kind,
    };

    node
}

fn display_path(path: &[String]) -> String {
    path.iter()
        .map(|part| {
            if part.contains('.') || part.contains('"') {
                format!("{part:?}")
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

impl Config {
    /// Turn a structural root record into a group of configuration settings.
    /// Nested field names determine their complete paths. Errors in the structure
    /// are saved for [`Self::validate`] to return. Existing Nix records remain single
    /// values rather than being expanded into extra paths.
    #[track_caller]
    pub fn from_value(value: RusixValue) -> Self {
        let mut config = Self::new();

        match value.resolve(&[]) {
            Ok(node) if matches!(node.kind, ValueKind::AttrSet(_)) => {
                flatten(&mut config, node, Vec::new());
            }
            Ok(node) => {
                config.error = Some(ValidationError {
                    origin: node.origin,
                    message: "a configuration contribution must be a rooted record".into(),
                })
            }
            Err(error) => config.error = Some(error),
        }

        config
    }
}

fn flatten(config: &mut Config, node: Node, path: Vec<String>) {
    if matches!(&node.kind, ValueKind::AttrSet(fields) if !fields.is_empty() || path.is_empty()) {
        let ValueKind::AttrSet(fields) = node.kind else {
            unreachable!()
        };

        for (name, child) in fields {
            let mut child_path = path.clone();
            child_path.push(name);
            flatten(config, child, child_path);
        }
    } else {
        let path_text = display_path(&path);
        config.assignments.push(Assignment {
            origin: Origin::new(
                &config.origin.file,
                config.origin.line,
                config.origin.column,
                format!("set {path_text}"),
            ),
            path: path_text,
            segments: path,
            value: node,
        });
    }
}

macro_rules! leaf {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusixValue for $ty {
            #[track_caller]
            fn into_value(self) -> RusixValue { RusixValue::leaf(self) }
        }
    )* };
}

leaf!(
    bool,
    i32,
    i64,
    u8,
    u16,
    u32,
    i8,
    i16,
    f64,
    String,
    &str,
    crate::Expr<i64>,
    crate::Expr<bool>,
    crate::Expr<String>,
    crate::interop::PackageRef,
    crate::interop::raw::NixValue
);

impl<T: IntoRusixValue> IntoRusixValue for Option<T> {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        match self {
            Some(value) => value.into_value(),
            None => RusixValue::leaf(crate::interop::raw::NixValue::null()),
        }
    }
}

impl<T: IntoRusixValue> IntoRusixValue for Vec<T> {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        // A function-pointer/closure adapter would stop tracked caller forwarding.
        let mut items = Vec::new();

        for value in self {
            items.push(value.into_value());
        }

        RusixValue {
            origin: Origin::caller("configuration list"),
            kind: Kind::List(items),
        }
    }
}

macro_rules! opaque {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusixValue for $ty {
            #[track_caller]
            fn into_value(self) -> RusixValue { RusixValue::leaf(self.as_value()) }
        }
    )* };
}

opaque!(
    crate::interop::ModuleRef,
    crate::interop::NixFunction,
    crate::interop::OverlayRef
);

macro_rules! option_ref {
    ($($ty:ty),* $(,)?) => { $(
        impl IntoRusixValue for crate::nixos::OptionRef<$ty> {
            #[track_caller]
            fn into_value(self) -> RusixValue { RusixValue::leaf(self.into_expr()) }
        }
    )* };
}

option_ref!(i64, bool, String);

/// Ordered Rust maps become structural records with literal, unmodified keys.
/// Child values retain their conversion locations and deferred expressions.
impl<K: Into<String>, V: IntoRusixValue> IntoRusixValue for std::collections::BTreeMap<K, V> {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        let mut fields = Vec::with_capacity(self.len());
        for (key, value) in self {
            fields.push((key.into(), value.into_value()));
        }

        RusixValue::record(fields)
    }
}
