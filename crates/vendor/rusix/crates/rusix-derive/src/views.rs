//! Shared finite symbolic views over final options or a supplied Nix record.
use super::Naming;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Fields, Ident, Item, ItemMod, LitStr, PathArguments, Type, spanned::Spanned};

struct View {
    /// Author-written struct whose fields declare Nix lookups and their expected Rust types.
    item: syn::ItemStruct,
    /// Whether this struct starts the declared NixOS option tree or supplied argument view.
    root: bool,
    /// Whether this view exposes its entire deferred Nix value in addition to field accessors.
    value: bool,
    /// Field-name conversion used to map Rust identifiers to literal Nix attribute names.
    naming: Naming,
}

enum Leaf {
    Scalar,
    Opaque,
    Expression,
    Branch(Ident),
}

fn classify(ty: &Type, locals: &BTreeSet<String>) -> syn::Result<Leaf> {
    if let Type::Path(ty) = ty
        && ty.qself.is_none()
    {
        let path = &ty.path;
        let names: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let last = path.segments.last().unwrap();

        if names.len() == 1 && locals.contains(&names[0]) {
            if !matches!(last.arguments, PathArguments::None) {
                return Err(syn::Error::new_spanned(
                    ty,
                    "symbolic views cannot have type arguments",
                ));
            }
            return Ok(Leaf::Branch(last.ident.clone()));
        }

        let qualified = names[..names.len() - 1].join("::");
        let qualified = qualified
            .strip_prefix(&format!("{}::", super::library_ident()))
            .map(|rest| format!("rusix::{rest}"))
            .unwrap_or(qualified);
        let expected_namespace = match last.ident.to_string().as_str() {
            "bool" | "i64" => "std::primitive",
            "String" => "std::string",
            "NixValue" => "rusix::interop::raw",
            "Package" | "NixCallable" | "NixAttrs" | "NixList" | "Stdenv" | "NixLibrary"
            | "PackageFunction" | "Overridable" | "Overlay" | "NixPath" | "NixNullable" => {
                "rusix::interop"
            }
            "Option" => "std::option",
            "Vec" => "std::vec",
            "BTreeMap" | "HashMap" => "std::collections",
            _ => "",
        };
        let accepted = qualified.is_empty() || qualified == expected_namespace;

        if accepted {
            match last.ident.to_string().as_str() {
                "bool" | "String" | "i64" if matches!(last.arguments, PathArguments::None) => {
                    return Ok(Leaf::Scalar);
                }
                "NixValue" if matches!(last.arguments, PathArguments::None) => {
                    return Ok(Leaf::Opaque);
                }
                "Package" | "Stdenv" | "NixLibrary" | "NixPath" | "Overlay"
                    if matches!(last.arguments, PathArguments::None) =>
                {
                    return Ok(Leaf::Expression);
                }
                "NixCallable" | "NixAttrs" | "NixList" | "PackageFunction" | "Overridable"
                | "NixNullable" => {
                    return Ok(Leaf::Expression);
                }
                "Option" | "Vec" | "BTreeMap" | "HashMap" => {
                    if let PathArguments::AngleBracketed(args) = &last.arguments {
                        let expected =
                            if matches!(last.ident.to_string().as_str(), "BTreeMap" | "HashMap") {
                                2
                            } else {
                                1
                            };
                        if args.args.len() == expected
                            && args
                                .args
                                .iter()
                                .all(|a| matches!(a, syn::GenericArgument::Type(_)))
                        {
                            return Ok(Leaf::Opaque);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Err(syn::Error::new_spanned(
        ty,
        "unsupported symbolic-view type; use bool, String, i64, NixValue, Option<T>, Vec<T>, BTreeMap<K, V>, HashMap<K, V>, or a local named view; external aliases require #[rusix(expression)] or explicit OptionRef or NixValue selections",
    ))
}

fn classify_field(field: &syn::Field, locals: &BTreeSet<String>) -> syn::Result<Leaf> {
    let mut expression = false;

    for attr in &field.attrs {
        if attr.path().is_ident("rusix") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("expression") {
                    if expression {
                        return Err(meta.error("duplicate expression"));
                    }
                    expression = true;
                } else if meta.path.is_ident("rename") {
                    let _: LitStr = meta.value()?.parse()?;
                } else {
                    return Err(meta.error("symbolic-view fields support rename and expression"));
                }
                Ok(())
            })?;
        }
    }

    if expression {
        Ok(Leaf::Expression)
    } else {
        classify(&field.ty, locals)
    }
}

fn parse_view(mut item: syn::ItemStruct) -> syn::Result<View> {
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "symbolic views do not support generics",
        ));
    }

    if !matches!(item.fields, Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &item,
            "symbolic views require named structs",
        ));
    }

    let mut root = false;
    let mut value = false;
    let mut naming = None;
    let mut attrs = Vec::new();

    for attr in item.attrs.drain(..) {
        if !attr.path().is_ident("rusix") {
            if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "conditional symbolic views are not supported; declare a separate view",
                ));
            }
            if !attr.path().is_ident("doc") {
                return Err(syn::Error::new_spanned(
                    attr,
                    "symbolic views are reference declarations; only documentation and rusix mapping attributes are supported",
                ));
            }

            attrs.push(attr);
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("root") || meta.path.is_ident("value") {
                let flag = if meta.path.is_ident("root") {
                    &mut root
                } else {
                    &mut value
                };
                if *flag {
                    return Err(meta.error("duplicate symbolic-view marker"));
                }
                *flag = true;
                Ok(())
            } else if meta.path.is_ident("rename_all") {
                if naming.is_some() {
                    return Err(meta.error("duplicate rename_all"));
                }
                naming = Some(Naming::parse(&meta.value()?.parse::<LitStr>()?)?);
                Ok(())
            } else {
                Err(meta.error("symbolic-view mappings are root, value, and rename_all"))
            }
        })?;
    }

    if root && value {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "root and value cannot be combined; mark a specific subtree with #[rusix(value)]",
        ));
    }

    item.attrs = attrs;

    Ok(View {
        item,
        root,
        value,
        naming: naming.unwrap_or_default(),
    })
}

fn key(field: &syn::Field, naming: Naming) -> syn::Result<LitStr> {
    let mut rename = None;

    for attr in &field.attrs {
        if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
            return Err(syn::Error::new_spanned(
                attr,
                "conditional symbolic-view fields are not supported",
            ));
        }

        if attr.path().is_ident("rusix") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("expression") {
                    return Ok(());
                }
                if !meta.path.is_ident("rename") {
                    return Err(meta.error("symbolic-view fields support rename and expression"));
                }
                if rename.is_some() {
                    return Err(meta.error("duplicate rename"));
                }
                rename = Some(meta.value()?.parse()?);
                Ok(())
            })?;
        }
    }

    naming.mapped_field(field.ident.as_ref().unwrap(), rename)
}

fn check_cycles(
    name: &str,
    graph: &BTreeMap<String, Vec<(String, proc_macro2::Span)>>,
    active: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> syn::Result<()> {
    if done.contains(name) {
        return Ok(());
    }

    active.insert(name.into());

    for (child, span) in &graph[name] {
        if active.contains(child) {
            return Err(syn::Error::new(
                *span,
                "recursive symbolic views are not supported; declare a finite set of paths",
            ));
        }

        check_cycles(child, graph, active, done)?;
    }

    active.remove(name);
    done.insert(name.into());

    Ok(())
}

#[derive(Clone, Copy)]
enum Source {
    Options,
    Arguments,
}

pub(super) fn expand(module: ItemMod) -> syn::Result<TokenStream> {
    expand_from(module, Source::Options)
}

pub(super) fn expand_args(module: ItemMod) -> syn::Result<TokenStream> {
    expand_from(module, Source::Arguments)
}

fn expand_from(mut module: ItemMod, source: Source) -> syn::Result<TokenStream> {
    let rusix = super::library_path();
    let macro_name = match source {
        Source::Options => "options",
        Source::Arguments => "args",
    };

    let Some((_, items)) = module.content.take() else {
        return Err(syn::Error::new_spanned(
            module,
            format!(
                "{macro_name} requires an inline module; external types use explicit OptionRef or NixValue selections"
            ),
        ));
    };

    let mut locals = BTreeSet::new();

    for item in &items {
        if let Item::Struct(item) = item {
            let name = item.ident.to_string();
            if matches!(
                name.as_str(),
                "String" | "NixValue" | "Option" | "Vec" | "BTreeMap" | "HashMap"
            ) || !locals.insert(name)
            {
                return Err(syn::Error::new_spanned(
                    &item.ident,
                    "ambiguous symbolic-view type name",
                ));
            }
        }
    }

    let mut views = Vec::new();
    let mut retained = Vec::new();

    for item in items {
        match item {
            Item::Struct(item) => views.push(parse_view(item)?),
            Item::Use(_) => retained.push(item),
            _ => {
                return Err(syn::Error::new_spanned(
                    item,
                    format!(
                        "{macro_name} modules contain named view structs and imports only; keep domain code outside the reference declarations"
                    ),
                ));
            }
        }
    }

    let roots: Vec<_> = views.iter().filter(|v| v.root).collect();
    if roots.len() != 1 {
        return Err(syn::Error::new_spanned(
            &module.ident,
            format!("{macro_name} requires exactly one #[rusix(root)] struct"),
        ));
    }

    let root = roots[0].item.ident.clone();
    let argument_names = if matches!(source, Source::Arguments) {
        let names = roots[0]
            .item
            .fields
            .iter()
            .map(|field| key(field, roots[0].naming))
            .collect::<syn::Result<Vec<_>>>()?;

        Some(quote!(
            /// Mapped names of this view's direct root fields, in declaration order.
            /// This describes the declared view only, not defaults or undeclared arguments.
            /// Use it for a native function interface when the view declares every argument.
            pub const fn argument_names() -> &'static [&'static str] {
                &[#(#names),*]
            }
        ))
    } else {
        None
    };
    let mut graph = BTreeMap::new();

    for view in &views {
        let whole_value = view.value || (view.root && matches!(source, Source::Arguments));
        let mut keys = BTreeSet::new();
        let mut children = Vec::new();

        for field in &view.item.fields {
            let name = field.ident.as_ref().unwrap();
            if matches!(
                name.to_string().as_str(),
                "__rusix_path" | "__rusix_at" | "__rusix_source"
            ) || (whole_value
                && (name == "as_value"
                    || (matches!(source, Source::Arguments) && name == "as_attrs")))
            {
                return Err(syn::Error::new_spanned(
                    name,
                    "field collides with a generated symbolic-view member",
                ));
            }

            let path_key = key(field, view.naming)?;
            if !keys.insert(path_key.value()) {
                return Err(syn::Error::new_spanned(
                    field,
                    "ambiguous symbolic path: fields map to the same attribute",
                ));
            }

            if let Leaf::Branch(child) = classify_field(field, &locals)? {
                if child == root {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "a root cannot be nested in a symbolic view",
                    ));
                }
                children.push((child.to_string(), field.ty.span()));
            }
        }

        graph.insert(view.item.ident.to_string(), children);
    }

    let mut done = BTreeSet::new();

    for name in graph.keys() {
        check_cycles(name, &graph, &mut BTreeSet::new(), &mut done)?;
    }

    let mut generated = Vec::new();

    for view in views {
        let whole_value = view.value || (view.root && matches!(source, Source::Arguments));
        let name = &view.item.ident;
        let attrs = &view.item.attrs;
        let description = match source {
            Source::Options => {
                "Access to declared NixOS fields after modules have been combined. Methods describe Nix lookups; Rust does not read the values."
            }
            Source::Arguments => {
                "Access to declared fields in a supplied Nix function argument set. Methods describe Nix lookups; Rust does not read the values."
            }
        };
        let type_docs = (!attrs.iter().any(|attr| attr.path().is_ident("doc")))
            .then(|| quote!(#[doc = #description]));
        let source_field = matches!(source, Source::Arguments).then(|| {
            quote!(
                /// Supplied Nix argument expression on which this view's field lookups operate.
                __rusix_source: #rusix::interop::raw::NixValue,
            )
        });
        let source_parameter = matches!(source, Source::Arguments).then(|| {
            quote!(
                source: #rusix::interop::raw::NixValue,
            )
        });
        let source_initialize = matches!(source, Source::Arguments).then(|| {
            quote!(
                __rusix_source: source,
            )
        });
        let child_source = matches!(source, Source::Arguments).then(|| quote!(source.clone(),));
        let mut branches = Vec::new();
        let mut initialize = Vec::new();
        let mut methods = Vec::new();

        for field in &view.item.fields {
            let field_name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            let literal = key(field, view.naming)?;
            let mut docs: Vec<syn::Attribute> = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .cloned()
                .collect();

            if docs.is_empty() {
                let description = match classify_field(field, &locals)? {
                    Leaf::Branch(_) => {
                        "Access the declared nested fields. Navigation constructs paths without evaluating Nix values."
                    }
                    Leaf::Scalar => match source {
                        Source::Options => {
                            "Refer to this field after NixOS combines the modules. Returns an expression with an expected Rust type, not a value read by Rust."
                        }
                        Source::Arguments => {
                            "Describe a lookup of this scalar field. Rust states the expected type; Nix evaluates and checks the actual value later."
                        }
                    },
                    _ => {
                        "Refer to this field as a Nix expression. Nix evaluates it later; Rust does not receive a concrete collection or value."
                    }
                };
                docs.push(syn::parse_quote!(#[doc = #description]));
            }

            match classify_field(field, &locals)? {
                Leaf::Branch(child) => {
                    branches
                        .push(quote_spanned!(field.span()=> #(#docs)* pub #field_name: #child,));
                    initialize.push(quote!(#field_name: {
                        let mut child_path = path.clone();
                        child_path.push(#literal.into());
                        #child::__rusix_at(#child_source child_path)
                    },));
                }
                leaf => {
                    let result = match leaf {
                        Leaf::Scalar => quote!(#rusix::Expr<#ty>),
                        Leaf::Expression => quote!(#ty),
                        _ => quote!(#rusix::interop::raw::NixValue),
                    };
                    let selection = match source {
                        Source::Options if matches!(leaf, Leaf::Scalar) => quote!(
                            #rusix::nixos::OptionRef::<#ty>::from_segments(path).into_expr()
                        ),
                        Source::Options => quote!(
                            #rusix::interop::raw::AsNixValue::as_value(
                                &#rusix::nixos::OptionRef::<#ty>::from_segments(path)
                            )
                        ),
                        Source::Arguments => {
                            let convert =
                                matches!(leaf, Leaf::Scalar).then(|| quote!(.into_expr::<#ty>()));
                            quote!({
                                let _: ::std::marker::PhantomData<#ty> = ::std::marker::PhantomData;
                                self.__rusix_source.clone().select_segments(path) #convert
                            })
                        }
                    };
                    let selection = if matches!(leaf, Leaf::Expression) {
                        quote!(#rusix::interop::raw::expect::<#ty>(#selection))
                    } else {
                        selection
                    };
                    methods.push(quote_spanned!(field.span()=>
                        #(#docs)*
                        #[track_caller]
                        pub fn #field_name(&self) -> #result {
                            let mut path = self.__rusix_path.clone();
                            path.push(#literal.into());
                            #selection
                        }
                    ));
                }
            }
        }

        let as_value = whole_value.then(|| {
            let selection = match source {
                Source::Options => {
                    quote!(
                        #rusix::interop::raw::AsNixValue::as_value(&#rusix::nixos::OptionRef::<
                            #rusix::interop::raw::NixValue,
                        >::from_segments(
                            self.__rusix_path.clone(),
                        ))
                    )
                }
                Source::Arguments => quote!({
                    if self.__rusix_path.is_empty() {
                        self.__rusix_source.clone()
                    } else {
                        self.__rusix_source
                            .clone()
                            .select_segments(self.__rusix_path.clone())
                    }
                }),
            };

            quote!(
                /// Refer to all fields in this declared subtree as one Nix value.
                /// Nix evaluates the lookup later; Rust does not read its contents.
                #[track_caller]
                #[doc(hidden)]
                pub fn as_value(&self) -> #rusix::interop::raw::NixValue {
                    #selection
                }
            )
        });

        let expression_impl = (whole_value && matches!(source, Source::Arguments)).then(|| {
            quote!(
                impl #name {
                    /// Retain this record as a deferred attribute set for shallow union.
                    #[track_caller]
                    pub fn as_attrs(&self) -> #rusix::interop::NixAttrs {
                        #rusix::interop::raw::expect(self.as_value())
                    }
                }

                impl #rusix::interop::raw::NixRepresentation for #name {
                    fn from_expression(value: #rusix::interop::raw::NixValue) -> Self {
                        Self::__rusix_at(value, ::std::vec::Vec::new())
                    }

                    fn as_expression(&self) -> #rusix::interop::raw::NixValue {
                        self.as_value()
                    }
                }

                impl #rusix::IntoRusixValue for #name {
                    #[track_caller]
                    fn into_value(self) -> #rusix::RusixValue {
                        #rusix::RusixValue::leaf(self.as_value())
                    }
                }
            )
        });

        generated.push(quote!(
            #(#attrs)*
            #type_docs
            #[derive(Clone)]
            pub struct #name {
                #source_field
                /// Literal names leading from the Nix source to the fields exposed by this view.
                __rusix_path: ::std::vec::Vec<::std::string::String>,
                #(#branches)*
            }

            impl #name {
                fn __rusix_at(#source_parameter path: ::std::vec::Vec<::std::string::String>) -> Self {
                    Self { #(#initialize)* #source_initialize __rusix_path: path }
                }

                #(#methods)*

                #as_value
            }

            #expression_impl
        ));
    }

    let attrs = &module.attrs;
    let visibility = &module.vis;
    let module_name = &module.ident;

    let constructor = match source {
        Source::Options => quote!(
            /// Begin referring to the declared final NixOS fields.
            /// Each leaf method records its own Rust call location without reading a value.
            pub fn root() -> #root {
                #root::__rusix_at(::std::vec::Vec::new())
            }
        ),
        Source::Arguments => quote!(
            /// Provide the Nix argument value whose declared fields these accessors refer to.
            /// Rust does not evaluate it; each leaf method records its own call location.
            pub fn from_value(value: #rusix::interop::raw::NixValue) -> #root {
                #root::__rusix_at(value, ::std::vec::Vec::new())
            }
        ),
    };

    Ok(quote!(
        #(#attrs)*
        #visibility mod #module_name {
            #(#retained)*

            #(#generated)*

            #constructor

            #argument_names
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::{expand, expand_args};

    #[test]
    fn argument_views_reuse_structural_validation_and_document_generated_members() {
        for source in [
            "mod args;",
            "mod args { struct MissingRoot { field: bool } }",
            "mod args { #[rusix(root)] struct Root<T> { field: T } }",
            "mod args { #[rusix(root)] struct Root { field: u16 } }",
            "mod args { #[rusix(root)] struct Root { a: A } struct A { a: A } }",
        ] {
            assert!(expand_args(syn::parse_str(source).unwrap()).is_err());
        }

        let output = expand_args(syn::parse_quote! {
            mod args {
                #[rusix(root)]
                struct Root {
                    // Nested supplied settings used to exercise generated field navigation.
                    settings: Settings,
                }

                #[rusix(value)]
                struct Settings {
                    // Integer field used to exercise a deferred Nix lookup.
                    port: i64,
                }
            }
        })
        .unwrap();
        let module: syn::ItemMod = syn::parse2(output).unwrap();

        for item in module.content.unwrap().1 {
            let attrs = match item {
                syn::Item::Struct(item) => item.attrs,
                syn::Item::Fn(item) => item.attrs,
                syn::Item::Impl(item) => {
                    for member in item.items {
                        if let syn::ImplItem::Fn(method) = member
                            && matches!(method.vis, syn::Visibility::Public(_))
                        {
                            assert!(method.attrs.iter().any(|attr| attr.path().is_ident("doc")));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            assert!(attrs.iter().any(|attr| attr.path().is_ident("doc")));
        }
    }

    #[test]
    fn public_views_are_documented_and_preserve_authored_docs() {
        let output = expand(syn::parse_quote! {
            mod options {
                #[rusix(root)]
                struct Root {
                    // Nested supplied settings used to exercise generated field navigation.
                    settings: Settings,
                }

                /// Settings explicitly used by this adapter.
                #[rusix(value)]
                struct Settings {
                    /// Port selected by ordinary NixOS merging.
                    port: i64,
                    // Undocumented boolean input used to check generated fallback documentation.
                    enabled: bool,
                }
            }
        })
        .unwrap();
        let module: syn::ItemMod = syn::parse2(output).unwrap();
        let mut documented = 0;
        let mut authored = Vec::new();
        let mut check = |attrs: &[syn::Attribute]| {
            let docs: Vec<_> = attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"))
                .collect();
            assert!(!docs.is_empty());
            documented += 1;
            authored.extend(
                docs.into_iter()
                    .map(|attr| quote::quote!(#attr).to_string()),
            );
        };

        for item in module.content.unwrap().1 {
            match item {
                syn::Item::Struct(item) => {
                    check(&item.attrs);
                    for field in item.fields {
                        if matches!(field.vis, syn::Visibility::Public(_)) {
                            check(&field.attrs);
                        }
                    }
                }
                syn::Item::Impl(item) => {
                    for member in item.items {
                        if let syn::ImplItem::Fn(method) = member
                            && matches!(method.vis, syn::Visibility::Public(_))
                        {
                            check(&method.attrs);
                        }
                    }
                }
                syn::Item::Fn(item) => check(&item.attrs),
                _ => {}
            }
        }

        assert_eq!(documented, 7);
        assert!(
            authored
                .iter()
                .any(|doc| doc.contains("Settings explicitly used by this adapter."))
        );
        assert!(
            authored
                .iter()
                .any(|doc| doc.contains("Port selected by ordinary NixOS merging."))
        );
    }

    fn rejection(source: &str) -> String {
        expand(syn::parse_str(source).unwrap())
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn external_modules_and_missing_roots_are_rejected() {
        assert!(rejection("mod options;").contains("inline module"));
        assert!(rejection("mod options { struct Value { port: i64 } }").contains("exactly one"));
    }

    #[test]
    fn invalid_naming_and_reserved_members_are_rejected() {
        assert!(rejection(r#"mod options { #[rusix(root, rename_all = "kebab-case")] struct Root { port: i64 } }"#).contains("rename_all must be"));
        assert!(
            rejection(
                r#"mod options { #[rusix(root)] struct Root { #[rusix(rename = "")] port: i64 } }"#
            )
            .contains("nonempty")
        );
        assert!(
            rejection("mod options { #[rusix(root)] struct Root { __rusix_path: i64 } }")
                .contains("collides")
        );
    }

    #[test]
    fn attribute_accessors_cannot_be_shadowed() {
        for source in [
            "mod arguments { #[rusix(root)] struct Root { value: Value } #[rusix(value)] struct Value { as_attrs: String } }",
            "mod arguments { #[rusix(root)] struct Root { as_attrs: String } }",
            "mod arguments { #[rusix(root)] struct Root { as_value: String } }",
        ] {
            let arguments: syn::ItemMod = syn::parse_str(source).unwrap();
            assert!(
                super::expand_args(arguments)
                    .unwrap_err()
                    .to_string()
                    .contains("collides")
            );
        }
    }

    #[test]
    fn indirect_recursion_and_ambiguous_type_names_are_rejected() {
        assert!(rejection("mod options { #[rusix(root)] struct Root { a: A } struct A { b: B } struct B { a: A } }").contains("recursive"));
        assert!(rejection("mod options { #[rusix(root)] struct Root { value: String } struct String { field: i64 } }").contains("ambiguous"));
    }

    #[test]
    fn generic_conditional_and_flattened_views_are_rejected() {
        assert!(
            rejection("mod options { #[rusix(root)] struct Root<T> { value: T } }")
                .contains("generics")
        );
        assert!(
            rejection("mod options { #[rusix(root)] struct Root { #[cfg(unix)] value: i64 } }")
                .contains("conditional")
        );
        assert!(
            rejection(
                "mod options { #[rusix(root)] struct Root { #[rusix(flatten)] value: i64 } }"
            )
            .contains("rename and expression")
        );
    }

    #[test]
    fn external_expression_fields_support_mapping_and_reject_duplicate_markers() {
        let view: syn::ItemMod = syn::parse_quote! {
            mod options {
                #[rusix(root)]
                struct Root {
                    #[rusix(expression, rename = "external-value")]
                    // External expression interface retained on the renamed Nix field.
                    value: external::Record,
                }
            }
        };
        assert!(expand(view.clone()).is_ok());
        assert!(expand_args(view).is_ok());
        assert!(rejection("mod options { #[rusix(root)] struct Root { #[rusix(expression, expression)] value: external::Record } }").contains("duplicate expression"));
        assert!(rejection("mod options { #[rusix(root)] struct Root { #[rusix(expression = true)] value: external::Record } }").contains("expected"));
    }

    #[test]
    fn default_derives_cannot_create_unrooted_reference_views() {
        assert!(
            rejection(
                "mod options { #[rusix(root)] #[derive(Default)] struct Root { value: i64 } }"
            )
            .contains("reference declarations")
        );
    }
}
