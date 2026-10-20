//! Explicit Nix text coercion retains dependency contexts and Rust provenance.
use super::raw::NixRepresentation;
use super::{NixExpression, NixValue, Package, PackageRef};
use crate::{Expr, nixos::OptionRef};

/// A value supported by Nix's text coercion, not an eagerly evaluated Rust string.
/// Dynamic NixValue is an escape hatch: Nix checks its actual coercibility later.
/// Attribute sets and callables do not acquire this capability automatically.
pub trait ToNixText {
    /// Describe native Nix toString, preserving the expression's dependency context.
    #[track_caller]
    fn to_nix_text(self) -> Expr<String>;
}

macro_rules! text_value {
    ($ty:ty) => {
        impl ToNixText for $ty {
            #[track_caller]
            fn to_nix_text(self) -> Expr<String> {
                NixValue::from(self).to_text().into_expr()
            }
        }
    };
}

text_value!(NixValue);

text_value!(Package);

text_value!(PackageRef);

text_value!(String);

text_value!(&str);

text_value!(bool);

text_value!(i64);

text_value!(i32);

text_value!(u8);

text_value!(u16);

text_value!(u32);

text_value!(i8);

text_value!(i16);

text_value!(f64);

text_value!(Expr<String>);

text_value!(Expr<bool>);

text_value!(Expr<i64>);

impl<T> ToNixText for OptionRef<T>
where
    Expr<T>: NixExpression + ToNixText,
{
    #[track_caller]
    fn to_nix_text(self) -> Expr<String> {
        self.into_expr().to_nix_text()
    }
}

impl Expr<String> {
    /// Replace text through native builtins while retaining the string interface.
    #[track_caller]
    pub fn replace_text(
        self,
        replacements: impl IntoIterator<Item = (impl Into<Self>, impl Into<Self>)>,
    ) -> Self {
        self.as_expression()
            .replace_text(
                replacements
                    .into_iter()
                    .map(|(from, to)| (NixValue::from(from.into()), NixValue::from(to.into()))),
            )
            .into_expr()
    }
}
