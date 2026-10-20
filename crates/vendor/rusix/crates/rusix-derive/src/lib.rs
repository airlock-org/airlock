//! Generate conversions and symbolic accessors for Rust configuration types.
//!
//! These macros turn ordinary Rust structure into Nix configuration, or describe
//! field lookups that Nix will evaluate later. They do not serialize Nix packages
//! into plain data or read Nix results into Rust. Normal users access the macros
//! through their re-exports in `rusix`, alongside its conversion traits.
#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input, parse_quote, spanned::Spanned};

mod config;

mod views;

mod text;

/// Internal expansion used by the hygienic public nix_text wrapper.
#[doc(hidden)]
#[proc_macro]
pub fn symbolic_text(input: TokenStream) -> TokenStream {
    text::expand(parse_macro_input!(input as text::Input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generate configuration conversions for local types in an inline Rust module.
/// Nested structs become nested Nix attribute sets: collections of named fields.
/// Mark a root with `#[rusix(root)]` to make it an independent contribution to
/// NixOS system configuration. No Nix evaluation happens in Rust.
///
/// Unmarked structs and unit enums receive nested value conversions. Imported
/// types must implement their own conversion traits. Multiple roots are allowed;
/// external module files are not inspected. See `rusix::config` for a small
/// example and the supported field naming and omission attributes.
#[proc_macro_attribute]
pub fn config(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(args),
            "config takes no arguments; mark root structs with #[rusix(root)]",
        )
        .into_compile_error()
        .into();
    }

    config::expand(parse_macro_input!(input as syn::ItemMod))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generate typed Rust accessors for final NixOS configuration values.
/// NixOS combines settings from many modules; these accessors describe references
/// to the resulting values. Rust never reads them, and ordinary Nix overrides
/// remain effective. This declares dependencies, not option types or defaults.
///
/// Use an inline module with one `#[rusix(root)]` struct. Nested structs describe
/// field paths; leaf methods create existing `OptionRef` expressions and record
/// the call location. NixOS checks existence and actual types. See
/// `rusix::options` for usage and supported leaf types.
#[proc_macro_attribute]
pub fn options(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(args),
            "options takes no arguments; mark one root with #[rusix(root)]",
        )
        .into_compile_error()
        .into();
    }

    views::expand(parse_macro_input!(input as syn::ItemMod))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generate typed Rust accessors for a Nix function’s named arguments.
/// Bind the generated view to an existing NixValue with `from_value`; accessor
/// calls describe field lookups for Nix to evaluate later. Rust does not read
/// the arguments or check their actual Nix types.
///
/// Nested structs describe nested fields. Naming, leaf types and optional
/// whole-subtree access match `options`; the difference is the supplied argument
/// value rather than NixOS’s final configuration. See `rusix::args` for usage.
#[proc_macro_attribute]
pub fn args(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new_spanned(
            proc_macro2::TokenStream::from(args),
            "args takes no arguments; mark one root with #[rusix(root)]",
        )
        .into_compile_error()
        .into();
    }

    views::expand_args(parse_macro_input!(input as syn::ItemMod))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Convert a reusable Rust struct into a complete group of configuration settings.
/// Nested fields determine Nix paths: a `services` field containing an `example`
/// field places that value under `services.example`. The derive implements both
/// `IntoConfig` and `IntoRusixValue`, so the type can also be nested in a parent.
/// It does not evaluate Nix or declare which NixOS options exist.
///
/// Fields must implement `IntoRusixValue`. Names default to lowerCamelCase;
/// `rename_all` accepts `lowerCamelCase`, `PascalCase`, or `preserve`, and field `rename`
/// overrides that convention. `skip` omits a field; `flatten` inserts a nested
/// record’s fields at the parent level.
///
/// Normally `None` becomes Nix `null`. `omit_none` on a field omits its absent
/// definition instead. On a named struct, it applies only to direct Option fields,
/// without inheritance or field opt-outs. It cannot combine with `flatten` or
/// an explicit field `skip`. Use the literal `Option<T>`, `std::option::Option<T>`
/// or `core::option::Option<T>` spelling; aliases are not inspected.
///
/// For local configuration trees, prefer the `config` attribute; use this derive
/// for reusable types outside that module.
#[proc_macro_derive(IntoConfig, attributes(rusix))]
pub fn into_config(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Convert a reusable Rust type into a nested configuration value.
/// Named structs become named Nix fields, a single-field newtype delegates to
/// its inner value, and unit enums become strings such as `"readOnly"` for
/// `ReadOnly`. The parent determines where the value appears in configuration.
///
/// Structs support the same mapping attributes as `IntoConfig`. Unit enums use
/// lowerCamelCase, with explicit variant `rename` and container `rename_all`
/// overrides. Enums carrying data need a custom conversion that decides what
/// their contents mean. Existing Nix expressions and package handles remain
/// expressions, not strings or evaluated Rust data.
/// To pass the derived value as one Nix function argument, call the trait's
/// `try_into_nix_value()` method. Invalid `flatten` values return a Rust error;
/// the derive does not guarantee that every field produces a structural record.
#[proc_macro_derive(IntoRusixValue, attributes(rusix))]
pub fn into_value(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone, Copy, Default)]
enum Naming {
    #[default]
    LowerCamel,
    Pascal,
    Preserve,
}

impl Naming {
    fn parse(name: &LitStr) -> syn::Result<Self> {
        match name.value().as_str() {
            "lowerCamelCase" => Ok(Self::LowerCamel),
            "PascalCase" => Ok(Self::Pascal),
            "preserve" => Ok(Self::Preserve),
            _ => Err(syn::Error::new_spanned(
                name,
                "rename_all must be \"lowerCamelCase\", \"PascalCase\", or \"preserve\"",
            )),
        }
    }

    fn mapped_field(self, field: &syn::Ident, rename: Option<LitStr>) -> syn::Result<LitStr> {
        let name =
            rename.unwrap_or_else(|| LitStr::new(&self.field(&field.to_string()), field.span()));
        if name.value().is_empty() || name.value().contains('\0') {
            return Err(syn::Error::new_spanned(
                name,
                "attribute name must be nonempty and NUL-free",
            ));
        }

        Ok(name)
    }

    fn variant(self, name: &str) -> String {
        let name = name.strip_prefix("r#").unwrap_or(name);
        if matches!(self, Self::Preserve) {
            return name.to_owned();
        }

        let chars: Vec<_> = name.chars().collect();
        let mut snake = String::new();

        for (index, &ch) in chars.iter().enumerate() {
            if ch.is_uppercase()
                && index > 0
                && (chars[index - 1].is_lowercase()
                    || chars[index - 1].is_numeric()
                    || (chars[index - 1].is_uppercase()
                        && chars.get(index + 1).is_some_and(|next| next.is_lowercase())))
            {
                snake.push('_');
            }
            snake.extend(ch.to_lowercase());
        }

        self.field(&snake)
    }

    fn field(self, name: &str) -> String {
        let name = name.strip_prefix("r#").unwrap_or(name);
        if matches!(self, Self::Preserve) {
            return name.to_owned();
        }

        let mut output = String::new();

        for (index, word) in name.split('_').filter(|word| !word.is_empty()).enumerate() {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                if index == 0 && matches!(self, Self::LowerCamel) {
                    output.extend(first.to_lowercase());
                } else {
                    output.extend(first.to_uppercase());
                }
                output.extend(chars);
            }
        }

        output
    }
}

// The self alias in rusix also works in its integration tests and examples,
// where `crate` names the test/example rather than the library.
fn library_ident() -> syn::Ident {
    let name = match proc_macro_crate::crate_name("rusix") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => name,
        Ok(proc_macro_crate::FoundCrate::Itself) | Err(_) => "rusix".into(),
    };
    syn::Ident::new(&name, proc_macro2::Span::call_site())
}

fn library_path() -> proc_macro2::TokenStream {
    let name = library_ident();
    quote!(::#name)
}

fn expand(input: DeriveInput, rooted: bool) -> syn::Result<proc_macro2::TokenStream> {
    let rusix = library_path();
    let mut rename_all = None;
    let mut omit_none = None;

    for attr in &input.attrs {
        if attr.path().is_ident("rusix") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("omit_none") {
                    if omit_none.is_some() {
                        return Err(meta.error("duplicate omit_none"));
                    }
                    omit_none = Some(meta.path.span());
                    return Ok(());
                }
                if !meta.path.is_ident("rename_all") {
                    return Err(meta.error(
                        "supported container mappings are rename_all and omit_none; nesting defines placement",
                    ));
                }
                if rename_all.is_some() {
                    return Err(meta.error("duplicate rename_all"));
                }
                let name: LitStr = meta.value()?.parse()?;
                let convention = Naming::parse(&name)?;
                rename_all = Some((convention, name));
                Ok(())
            })?;
        }
    }

    let naming = rename_all
        .as_ref()
        .map(|(naming, _)| *naming)
        .unwrap_or_default();

    if let Data::Enum(data) = &input.data {
        if let Some(span) = omit_none {
            return Err(syn::Error::new(
                span,
                "omit_none requires a named-field struct",
            ));
        }
        if rooted {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "IntoConfig requires a named struct",
            ));
        }
        return enum_value(&input, data, naming);
    }

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "derive supports structs and unit enums",
        ));
    };

    if let Some((_, name)) = &rename_all
        && !matches!(data.fields, Fields::Named(_))
    {
        return Err(syn::Error::new_spanned(
            name,
            "rename_all requires named fields; newtypes are transparent",
        ));
    }

    if let Some(span) = omit_none
        && !matches!(data.fields, Fields::Named(_))
    {
        return Err(syn::Error::new(
            span,
            "omit_none requires a named-field struct",
        ));
    }

    let mut generics = input.generics.clone();
    let body = match &data.fields {
        Fields::Named(fields) => {
            let mut values = Vec::new();

            for field in &fields.named {
                let mut rename: Option<LitStr> = None;
                let mut skip = false;
                let mut flatten = false;
                let mut field_omit_none = None;

                for attr in &field.attrs {
                    if !attr.path().is_ident("rusix") {
                        continue;
                    }
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("rename") {
                            if rename.is_some() {
                                return Err(meta.error("duplicate rename"));
                            }
                            let name: LitStr = meta.value()?.parse()?;
                            if name.value().is_empty() || name.value().contains('\0') {
                                return Err(meta.error("rename must be nonempty and NUL-free"));
                            }
                            rename = Some(name);
                        } else if meta.path.is_ident("skip") {
                            if skip {
                                return Err(meta.error("duplicate skip"));
                            }
                            skip = true;
                        } else if meta.path.is_ident("flatten") {
                            if flatten {
                                return Err(meta.error("duplicate flatten"));
                            }
                            flatten = true;
                        } else if meta.path.is_ident("omit_none") {
                            if field_omit_none.is_some() {
                                return Err(meta.error("duplicate omit_none"));
                            }
                            field_omit_none = Some(meta.path.span());
                        } else {
                            return Err(meta.error(
                                "supported field mappings are rename, skip, flatten, and omit_none",
                            ));
                        }
                        Ok(())
                    })?;
                }

                if (skip && (flatten || rename.is_some())) || (flatten && rename.is_some()) {
                    return Err(syn::Error::new_spanned(
                        field,
                        "skip, flatten, and rename cannot be combined on one field",
                    ));
                }

                let optional = option_inner(&field.ty);
                if let Some(span) = field_omit_none {
                    if optional.is_none() {
                        return Err(syn::Error::new(
                            span,
                            "omit_none requires an Option<T> field",
                        ));
                    }
                    if skip || flatten {
                        return Err(syn::Error::new(
                            span,
                            "omit_none cannot be combined with skip or flatten",
                        ));
                    }
                }
                if skip {
                    continue;
                }

                let omit = field_omit_none.is_some() || (omit_none.is_some() && optional.is_some());
                if omit && flatten {
                    return Err(syn::Error::new_spanned(
                        field,
                        "omit_none cannot be combined with flatten",
                    ));
                }

                let name = field.ident.as_ref().unwrap();
                let ty = if omit { optional.unwrap() } else { &field.ty };
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: #rusix::IntoRusixValue));

                let key = if flatten {
                    quote!(None)
                } else {
                    let logical = naming.mapped_field(name, rename)?;
                    quote!(Some(#logical))
                };

                values.push(if omit {
                    quote_spanned!(field.span()=>
                        if let ::core::option::Option::Some(__rusix_value) = self.#name {
                            __rusix_fields.push((#key, #rusix::IntoRusixValue::into_value(__rusix_value)));
                        }
                    )
                } else {
                    quote_spanned!(field.span()=>
                        __rusix_fields.push((#key, #rusix::IntoRusixValue::into_value(self.#name)));
                    )
                });
            }

            quote!({
                let mut __rusix_fields = ::std::vec::Vec::new();

                #(#values)*

                #rusix::RusixValue::__record(__rusix_fields)
            })
        }
        Fields::Unnamed(fields) if !rooted && fields.unnamed.len() == 1 => {
            let field = fields.unnamed.first().unwrap();
            if field.attrs.iter().any(|a| a.path().is_ident("rusix")) {
                return Err(syn::Error::new_spanned(
                    field,
                    "a newtype is transparent; mapping attributes require named fields",
                ));
            }

            let ty = &field.ty;
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(#ty: #rusix::IntoRusixValue));
            quote!(#rusix::IntoRusixValue::into_value(self.0))
        }
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "IntoConfig requires named fields; IntoRusixValue also supports single-field newtypes",
            ));
        }
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let config_impl = rooted.then(|| config_impl(&input.ident, &generics));

    Ok(quote!(
        impl #impl_generics #rusix::IntoRusixValue for #name #ty_generics #where_clause {
            #[track_caller]
            fn into_value(self) -> #rusix::RusixValue { #body }
        }

        #config_impl
    ))
}

// Attributes inspect spelling, not Rust type resolution; aliases remain explicit conversions.
fn option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(ty) = ty else { return None };
    if ty.qself.is_some() {
        return None;
    }

    let names: Vec<_> = ty
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if names != ["Option"]
        && names != ["std", "option", "Option"]
        && names != ["core", "option", "Option"]
    {
        return None;
    }

    let syn::PathArguments::AngleBracketed(args) = &ty.path.segments.last()?.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }

    match args.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn enum_value(
    input: &DeriveInput,
    data: &syn::DataEnum,
    naming: Naming,
) -> syn::Result<proc_macro2::TokenStream> {
    let rusix = library_path();
    let mut arms = Vec::new();
    let mut names = std::collections::BTreeSet::new();

    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "automatic enum lowering supports unit variants only; implement IntoRusixValue for data-carrying enums",
            ));
        }

        let mut rename: Option<LitStr> = None;

        for attr in &variant.attrs {
            if !attr.path().is_ident("rusix") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("rename") {
                    return Err(meta.error("supported variant mapping is rename"));
                }
                if rename.is_some() {
                    return Err(meta.error("duplicate rename"));
                }
                let name: LitStr = meta.value()?.parse()?;
                if name.value().is_empty() || name.value().contains('\0') {
                    return Err(meta.error("rename must be nonempty and NUL-free"));
                }
                rename = Some(name);
                Ok(())
            })?;
        }

        let name = rename.unwrap_or_else(|| {
            LitStr::new(
                &naming.variant(&variant.ident.to_string()),
                variant.ident.span(),
            )
        });
        if name.value().is_empty() || !names.insert(name.value()) {
            return Err(syn::Error::new_spanned(
                name,
                "enum variants must lower to distinct nonempty names",
            ));
        }

        let variant_name = &variant.ident;
        // Conditional variants must also condition their generated match arm.
        let mut gates = Vec::new();

        for attr in &variant.attrs {
            if let Some(gate) = config::cfg_gate(&attr.meta)? {
                gates.push(quote!(#[#gate]));
            }
        }

        arms.push(quote_spanned!(variant.span()=>
            #(#gates)*
            Self::#variant_name => #rusix::IntoRusixValue::into_value(#name)
        ));
    }

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote!(
        impl #impl_generics #rusix::IntoRusixValue for #name #ty_generics #where_clause {
            #[track_caller]
            fn into_value(self) -> #rusix::RusixValue {
                match self { #(#arms),* }
            }
        }
    ))
}

fn config_impl(name: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let rusix = library_path();
    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: #rusix::IntoRusixValue));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote!(
        impl #impl_generics #rusix::IntoConfig for #name #ty_generics #where_clause {
            #[track_caller]
            fn into_config(self) -> #rusix::Config {
                #rusix::Config::from_value(#rusix::IntoRusixValue::into_value(self))
            }
        }
    )
}

#[cfg(test)]
mod tests {
    use super::Naming;

    #[test]
    fn variant_names_split_pascal_words_and_acronyms() {
        for (rust, camel, pascal) in [
            ("Server", "server", "Server"),
            ("ReadOnly", "readOnly", "ReadOnly"),
            ("HTTPServer", "httpServer", "HttpServer"),
            ("TLS", "tls", "Tls"),
            ("r#type", "type", "Type"),
        ] {
            assert_eq!(Naming::LowerCamel.variant(rust), camel);
            assert_eq!(Naming::Pascal.variant(rust), pascal);
        }
    }

    #[test]
    fn lower_camel_joins_snake_case_words() {
        for (rust, nix) in [
            ("enable_feature", "enableFeature"),
            ("listen_port", "listenPort"),
            ("service_config", "serviceConfig"),
            ("http_port", "httpPort"),
            ("r#type", "type"),
            ("alreadyCamel", "alreadyCamel"),
            ("_local__value_", "localValue"),
        ] {
            assert_eq!(Naming::LowerCamel.field(rust), nix);
        }
    }

    #[test]
    fn pascal_capitalizes_each_snake_case_word() {
        for (rust, nix) in [
            ("exec_start", "ExecStart"),
            ("restart", "Restart"),
            ("user", "User"),
            ("working_directory", "WorkingDirectory"),
            ("r#type", "Type"),
        ] {
            assert_eq!(Naming::Pascal.field(rust), nix);
        }
    }
}
