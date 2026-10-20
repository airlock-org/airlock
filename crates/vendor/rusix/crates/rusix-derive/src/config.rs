//! Module sugar: inject the existing derives, retaining their mapping rules.
use quote::quote;
use std::collections::BTreeSet;
use syn::{
    Attribute, Fields, Item, ItemMod, Path, Token, Type, parse_quote, punctuated::Punctuated,
};

fn derives(attrs: &[Attribute]) -> syn::Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();

    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        for path in attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)? {
            names.insert(path.segments.last().unwrap().ident.to_string());
        }
    }

    Ok(names)
}

// Copy conditional existence to a generated adapter, not unrelated attributes
// such as cfg_attr(..., derive(Debug)) which are invalid on an impl.
pub(super) fn cfg_gate(meta: &syn::Meta) -> syn::Result<Option<syn::Meta>> {
    if meta.path().is_ident("cfg") {
        return Ok(Some(meta.clone()));
    }

    if let syn::Meta::List(list) = meta
        && list.path.is_ident("cfg_attr")
    {
        let mut entries = list
            .parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?
            .into_iter();
        let condition = entries.next();
        let gates: Vec<_> = entries
            .map(|entry| cfg_gate(&entry))
            .collect::<syn::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        if let Some(condition) = condition
            && !gates.is_empty()
        {
            return Ok(Some(parse_quote!(cfg_attr(#condition, #(#gates),*))));
        }
    }

    Ok(None)
}

// Remove only root. Other mappings remain for the normal derive to validate.
fn take_root(attrs: &mut Vec<Attribute>) -> syn::Result<bool> {
    let mut root = false;
    let mut retained = Vec::new();

    for attr in attrs.drain(..) {
        if !attr.path().is_ident("rusix") {
            retained.push(attr);
            continue;
        }

        let entries = attr.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;
        let mut mappings = Vec::new();

        for entry in entries {
            if entry.path().is_ident("root") {
                if root {
                    return Err(syn::Error::new_spanned(
                        entry,
                        "duplicate rusix root marker",
                    ));
                }
                if !matches!(entry, syn::Meta::Path(_)) {
                    return Err(syn::Error::new_spanned(entry, "root takes no arguments"));
                }
                root = true;
            } else {
                mappings.push(entry);
            }
        }

        if !mappings.is_empty() {
            retained.push(parse_quote!(#[rusix(#(#mappings),*)]));
        }
    }

    // Existing derives must introduce helper attributes before Rust sees them.
    retained.sort_by_key(|attr| attr.path().is_ident("rusix"));
    *attrs = retained;

    Ok(root)
}

pub(super) fn expand(mut module: ItemMod) -> syn::Result<proc_macro2::TokenStream> {
    let rusix = super::library_path();
    let Some((_, items)) = &mut module.content else {
        return Err(syn::Error::new_spanned(
            &module,
            "config requires an inline module; use fine-grained derives in external files",
        ));
    };

    // An explicit impl is user control. Do not duplicate it or inspect its body.
    let mut implementations = BTreeSet::new();

    for item in items.iter() {
        if let Item::Impl(item) = item
            && let Some((trait_path, _)) = &item.trait_
            && let Type::Path(ty) = &*item.self_ty
            && ty.qself.is_none()
            && (ty.path.segments.len() == 1
                || (ty.path.segments.len() == 2 && ty.path.segments[0].ident == "self"))
        {
            implementations.insert((
                ty.path.segments.last().unwrap().ident.to_string(),
                trait_path.segments.last().unwrap().ident.to_string(),
            ));
        }
    }

    let mut roots = 0;
    let mut adapters = Vec::new();

    for item in items.iter_mut() {
        match item {
            Item::Struct(item) => {
                let root = take_root(&mut item.attrs)?;
                if root && !matches!(item.fields, Fields::Named(_)) {
                    return Err(syn::Error::new_spanned(
                        &item.ident,
                        "root requires a named struct",
                    ));
                }

                roots += usize::from(root);
                let derives = derives(&item.attrs)?;
                let name = item.ident.to_string();
                let existing = |trait_name: &str| {
                    derives.contains(trait_name)
                        || implementations.contains(&(name.clone(), trait_name.into()))
                };
                let has_config = existing("IntoConfig");
                let has_value = existing("IntoRusixValue") || has_config;

                if root && !has_config && has_value {
                    // A custom/explicit nested conversion can also become a root.
                    let adapter = super::config_impl(&item.ident, &item.generics);
                    let mut cfg = Vec::new();
                    for attr in &item.attrs {
                        if let Some(gate) = cfg_gate(&attr.meta)? {
                            cfg.push(quote!(#[#gate]));
                        }
                    }
                    adapters.push(quote!(#(#cfg)* #adapter));
                } else if !has_value {
                    item.attrs.insert(
                        0,
                        if root {
                            parse_quote!(#[derive(#rusix::IntoConfig)])
                        } else {
                            parse_quote!(#[derive(#rusix::IntoRusixValue)])
                        },
                    );
                }
            }
            Item::Enum(item) => {
                if take_root(&mut item.attrs)? {
                    return Err(syn::Error::new_spanned(
                        &item.ident,
                        "root requires a named struct",
                    ));
                }

                let explicit = derives(&item.attrs)?.contains("IntoRusixValue")
                    || implementations.contains(&(item.ident.to_string(), "IntoRusixValue".into()));

                if !explicit
                    && item
                        .variants
                        .iter()
                        .all(|variant| matches!(variant.fields, Fields::Unit))
                {
                    item.attrs
                        .insert(0, parse_quote!(#[derive(#rusix::IntoRusixValue)]));
                }
                // Data-carrying enums retain user-defined conversion semantics.
            }
            _ => {}
        }
    }

    if roots == 0 {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "config needs at least one named struct marked #[rusix(root)]",
        ));
    }

    items.extend(adapters.into_iter().map(Item::Verbatim));

    Ok(quote!(#module))
}

#[cfg(test)]
mod tests {
    #[test]
    fn out_of_line_modules_are_rejected_without_loading_files() {
        let module = syn::parse_quote!(
            #[path = "/rusix-definitely-not-a-source-file.rs"]
            mod config;
        );
        assert!(
            super::expand(module)
                .unwrap_err()
                .to_string()
                .contains("inline module")
        );
    }
}
