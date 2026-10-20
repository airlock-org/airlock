//! Named interpolation expands into the existing deferred text operations.
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Expr, Ident, LitStr, Path, Token, parse::Parse, spanned::Spanned};

pub(super) struct Input {
    /// Crate path supplied by the public wrapper so expansion resolves Rusix APIs correctly.
    crate_path: Path,
    /// Rust string literal containing text and named holes for deferred Nix interpolation.
    template: LitStr,
    /// Named Rust expressions substituted into template holes without evaluating Nix.
    arguments: Vec<(Ident, Expr)>,
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let crate_path = input.parse()?;
        input.parse::<Token![;]>()?;
        let template = input.parse()?;

        let mut arguments = Vec::new();

        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let name = input.parse()?;
            input.parse::<Token![=]>()?;
            arguments.push((name, input.parse()?));
        }

        Ok(Self {
            crate_path,
            template,
            arguments,
        })
    }
}

enum Part {
    Literal(String),
    Argument(usize),
}

fn dedent(text: &str) -> String {
    let Some(body) = text.strip_prefix('\n') else {
        return text.to_owned();
    };
    let indentation_only = |line: &str| line.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    let body = match body.rsplit_once('\n') {
        Some((_, last)) if indentation_only(last) => &body[..body.len() - last.len()],
        None if indentation_only(body) => return String::new(),
        _ => body,
    };

    // Compare literal prefixes, not visual columns: a tab never equals spaces.
    let mut common: Option<&str> = None;

    for line in body.split('\n').filter(|line| !indentation_only(line)) {
        let prefix = &line[..line.len() - line.trim_start_matches([' ', '\t']).len()];
        common = Some(match common {
            None => prefix,
            Some(old) => {
                let length = old
                    .bytes()
                    .zip(prefix.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                &old[..length]
            }
        });
    }

    let common = common.unwrap_or("");
    let mut output = String::with_capacity(body.len());

    for line in body.split_inclusive('\n') {
        // Short whitespace-only lines retain their newline and any extra spacing.
        let length = line
            .bytes()
            .zip(common.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        output.push_str(&line[length..]);
    }

    output
}

pub(super) fn expand(input: Input) -> syn::Result<TokenStream> {
    let Input {
        crate_path,
        template,
        arguments,
    } = input;

    let mut names = BTreeMap::new();

    for (index, (name, _)) in arguments.iter().enumerate() {
        if names.insert(name.to_string(), index).is_some() {
            return Err(syn::Error::new(name.span(), "duplicate nix_text argument"));
        }
    }

    let fail = |message| syn::Error::new(template.span(), message);
    let text = dedent(&template.value());
    let mut chars = text.chars().peekable();
    let mut literal = String::new();
    let mut parts = Vec::new();
    let mut used = BTreeSet::new();

    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                literal.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                literal.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') => return Err(fail("nested brace in nix_text placeholder")),
                        Some(ch) => name.push(ch),
                        None => return Err(fail("unclosed nix_text placeholder")),
                    }
                }

                if syn::parse_str::<Ident>(&name).is_err() {
                    return Err(fail(
                        "nix_text supports only named placeholders like {name}; formatting syntax is unsupported",
                    ));
                }

                let Some(&index) = names.get(&name) else {
                    return Err(fail(&format!("unknown nix_text placeholder {{{name}}}")));
                };

                used.insert(index);
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(&mut literal)));
                }
                parts.push(Part::Argument(index));
            }
            '}' => {
                return Err(fail(
                    "unmatched closing brace in nix_text; use }} for a literal brace",
                ));
            }
            ch => literal.push(ch),
        }
    }

    if !literal.is_empty() {
        parts.push(Part::Literal(literal));
    }

    for (index, (name, _)) in arguments.iter().enumerate() {
        if !used.contains(&index) {
            return Err(syn::Error::new(name.span(), "unused nix_text argument"));
        }
    }

    let bindings: Vec<_> = (0..arguments.len())
        .map(|index| format_ident!("__rusix_text_arg_{index}", span = Span::mixed_site()))
        .collect();
    let declarations = arguments.iter().zip(&bindings).map(|((_, expr), binding)| {
        quote_spanned! {expr.span()=> let #binding = #crate_path::interop::ToNixText::to_nix_text(#expr); }
    });
    let values = parts.into_iter().map(|part| match part {
        Part::Literal(text) => {
            let text = LitStr::new(&text, template.span());
            quote! { #crate_path::Expr::<String>::from(#text) }
        }
        Part::Argument(index) => {
            let binding = &bindings[index];
            quote! { #binding.clone() }
        }
    });

    Ok(quote! {{
        #(#declarations)*
        #crate_path::Expr::<String>::concat([#(#values),*])
    }})
}

#[cfg(test)]
mod tests {
    use super::dedent;

    #[test]
    fn content_first_templates_remain_verbatim() {
        for text in ["single line", "    first\n  second\n", "", " \n  body\n"] {
            assert_eq!(dedent(text), text);
        }
    }

    #[test]
    fn block_templates_keep_relative_indentation_and_blank_lines() {
        assert_eq!(
            dedent("\n    first\n      nested\n\n    last\n    "),
            "first\n  nested\n\nlast\n",
        );
        assert_eq!(dedent("\n    first\n  \n    last\n    "), "first\n\nlast\n");
        assert_eq!(
            dedent("\n    first\n      \n    last\n    "),
            "first\n  \nlast\n"
        );
    }

    #[test]
    fn least_indented_content_line_limits_dedent() {
        assert_eq!(dedent("\n    first\n  second\n    "), "  first\nsecond\n");
        assert_eq!(dedent("\n    first\nsecond\n    "), "    first\nsecond\n");
    }

    #[test]
    fn tabs_and_spaces_use_an_exact_common_prefix() {
        assert_eq!(dedent("\n\tfirst\n\t\tnested\n\t"), "first\n\tnested\n");
        assert_eq!(dedent("\n\t first\n\t   nested\n\t "), "first\n  nested\n");
        assert_eq!(
            dedent("\n\tfirst\n    second\n    "),
            "\tfirst\n    second\n"
        );
    }

    #[test]
    fn trailing_newlines_are_neither_added_nor_lost() {
        assert_eq!(dedent("\n    first"), "first");
        assert_eq!(dedent("\n    first\n    "), "first\n");
        assert_eq!(dedent("\n    first\n\n    "), "first\n\n");
        assert_eq!(dedent("\n    first  "), "first  ");
    }

    #[test]
    fn empty_blocks_and_leading_blank_lines_are_preserved() {
        assert_eq!(dedent("\n    "), "");
        assert_eq!(dedent("\n"), "");
        assert_eq!(dedent("\n\n"), "\n");
        assert_eq!(dedent("\n\n    first\n    "), "\nfirst\n");
    }
}
