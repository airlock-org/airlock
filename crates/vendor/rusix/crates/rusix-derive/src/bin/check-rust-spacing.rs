//! Repository style check, separate from stable rustfmt. Macro token bodies and
//! Rust snippets inside strings/comments are not parsed as Rust item lists.
use proc_macro2::Span;
use std::{collections::BTreeSet, fs, path::Path};
use syn::{Item, Stmt, spanned::Spanned, visit::Visit};

struct Spacing<'a> {
    /// Rust source being inspected, used to distinguish blank lines from lines inside comments.
    source: &'a str,
    /// Source line positions where distinct item declarations need a separating blank line.
    missing: BTreeSet<usize>,
}

impl Spacing<'_> {
    fn pair(&mut self, previous: Span, next: Span) {
        let end = previous.end();
        let start = next.start();
        // Only whitespace and comments occur between these AST items. Track
        // nested block comments so an empty line inside a comment doesn't count.
        let lines: Vec<_> = self.source.lines().collect();
        let mut depth = 0_usize;
        let mut insertion = end.line;
        let mut trailing_comment = false;

        for line in end.line..start.line {
            let text = lines[line - 1];
            if line > end.line && depth == 0 && text.trim().is_empty() {
                return;
            }
            let text = if line == end.line {
                &text[end.column..]
            } else {
                text
            };
            let mut bytes = text.as_bytes();
            while bytes.len() >= 2 {
                match &bytes[..2] {
                    b"//" if depth == 0 => break,
                    b"/*" => {
                        depth += 1;
                        bytes = &bytes[2..];
                    }
                    b"*/" if depth > 0 => {
                        depth -= 1;
                        bytes = &bytes[2..];
                    }
                    _ => bytes = &bytes[1..],
                }
            }
            if line == end.line {
                trailing_comment = depth > 0;
            } else if trailing_comment && depth == 0 {
                insertion = line;
                trailing_comment = false;
            }
        }

        // Insert before the following item's comments/attributes, never within
        // its attached prefix. --fix expects rustfmt's one-item-per-line layout.
        self.missing.insert(insertion);
    }

    fn items(&mut self, items: &[Item]) {
        for pair in items.windows(2) {
            if !plain_use(&pair[0]) || !plain_use(&pair[1]) {
                self.pair(pair[0].span(), pair[1].span());
            }
        }
    }
}

fn plain_use(item: &Item) -> bool {
    matches!(item, Item::Use(item) if item.attrs.is_empty())
}

impl<'ast> Visit<'ast> for Spacing<'_> {
    fn visit_file(&mut self, file: &'ast syn::File) {
        self.items(&file.items);
        syn::visit::visit_file(self, file);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if let Some((_, items)) = &module.content {
            self.items(items);
        }
        syn::visit::visit_item_mod(self, module);
    }

    fn visit_item_impl(&mut self, implementation: &'ast syn::ItemImpl) {
        for pair in implementation.items.windows(2) {
            self.pair(pair[0].span(), pair[1].span());
        }
        syn::visit::visit_item_impl(self, implementation);
    }

    fn visit_item_trait(&mut self, definition: &'ast syn::ItemTrait) {
        for pair in definition.items.windows(2) {
            self.pair(pair[0].span(), pair[1].span());
        }
        syn::visit::visit_item_trait(self, definition);
    }

    fn visit_item_foreign_mod(&mut self, module: &'ast syn::ItemForeignMod) {
        for pair in module.items.windows(2) {
            self.pair(pair[0].span(), pair[1].span());
        }
        syn::visit::visit_item_foreign_mod(self, module);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        for pair in block.stmts.windows(2) {
            if let [Stmt::Item(previous), Stmt::Item(next)] = pair
                && (!plain_use(previous) || !plain_use(next))
            {
                self.pair(previous.span(), next.span());
            }
        }
        syn::visit::visit_block(self, block);
    }
}

fn missing(source: &str) -> syn::Result<BTreeSet<usize>> {
    let file = syn::parse_file(source)?;
    let mut checker = Spacing {
        source,
        missing: BTreeSet::new(),
    };
    checker.visit_file(&file);
    Ok(checker.missing)
}

fn rust_files(root: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir()
            && entry.file_name() != "target"
            && entry.file_name() != ".git"
            && !entry.path().join(".git").exists()
        {
            files.extend(rust_files(&entry.path())?);
        } else if kind.is_file() && entry.path().extension().is_some_and(|s| s == "rs") {
            files.push(entry.path());
        }
    }

    files.sort();
    Ok(files)
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let fix = args == ["--fix"];
    if !args.is_empty() && !fix {
        return Err("usage: check-rust-spacing [--fix] (run rustfmt before --fix)".into());
    }

    let root = root();
    let files = rust_files(&root)?;
    let mut violations = 0;

    for path in &files {
        let source = fs::read_to_string(path)?;
        let gaps = missing(&source).map_err(|error| format!("{}: {error}", path.display()))?;
        violations += gaps.len();
        if fix && !gaps.is_empty() {
            let mut output = String::new();
            for (line, text) in source.split_inclusive('\n').enumerate() {
                output.push_str(text);
                if gaps.contains(&(line + 1)) {
                    output.push('\n');
                }
            }

            if !missing(&output)?.is_empty() {
                return Err(format!("{}: run rustfmt before --fix", path.display()).into());
            }

            fs::write(path, output)?;
        } else {
            for line in gaps {
                eprintln!(
                    "{}:{line}: separate adjacent Rust items with a blank line",
                    path.strip_prefix(&root)?.display()
                );
            }
        }
    }

    if violations > 0 && !fix {
        return Err(format!("{violations} declaration-spacing violations").into());
    }

    println!(
        "Rust spacing: {} files checked; {violations} gaps {}",
        files.len(),
        if fix { "inserted" } else { "missing" }
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_submodule_rust_files_are_excluded() {
        let root = root();
        let files = rust_files(&root).unwrap();
        assert!(!files.is_empty());
        assert!(
            files
                .iter()
                .all(|path| !path.starts_with(root.join("vendor")))
        );
    }

    #[test]
    fn modules_impls_traits_and_foreign_items() {
        let source = "mod local {\nstruct A;\nstruct B;\n}\n\nimpl A {\nconst N: u8 = 1;\nfn f() {}\n}\n\ntrait T {\n type X;\n fn f();\n}\n\nunsafe extern \"C\" {\n fn a();\n fn b();\n}\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([2, 7, 12, 17]));
    }

    #[test]
    fn ordinary_statements_fields_and_import_groups_stay_compact() {
        let source = "use a::A;\nuse b::B;\n\nstruct S { a: u8, b: u8 }\n\nfn f() {\nlet a = 1;\nlet b = 2;\nprintln!(\"{a} {b}\");\n}\n";
        assert!(missing(source).unwrap().is_empty());
    }

    #[test]
    fn block_local_items_are_checked_but_statements_are_not() {
        let source = "fn f() {\nstruct A;\nstruct B;\nlet a = 1;\nlet b = 2;\nconst C: u8 = 3;\nconst D: u8 = 4;\n}\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([2, 6]));
    }

    #[test]
    fn comments_and_attributes_stay_with_the_following_item() {
        let source = "struct A;\n// about B\n/// B docs\n#[allow(dead_code)]\nstruct B;\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([1]));
        assert!(
            missing(&source.replacen("A;\n", "A;\n\n", 1))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn blank_lines_inside_nested_block_comments_do_not_count() {
        let source = "struct A; /* trailing */\n/* B /* nested */\n\n docs */\nstruct B;\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([1]));
    }

    #[test]
    fn separator_follows_a_multiline_trailing_comment() {
        let source = "struct A; /* trailing\n comment */\n// B docs\nstruct B;\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([2]));
        assert!(
            missing(&source.replacen(" comment */\n", " comment */\n\n", 1))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn same_line_items_and_documented_imports_require_separation() {
        assert_eq!(missing("struct A; struct B;").unwrap(), BTreeSet::from([1]));
        let source = "use a::A;\n/// Reexported API.\npub use b::B;\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([1]));
    }

    #[test]
    fn macro_definitions_are_items_but_token_templates_are_opaque() {
        let source = "macro_rules! m { () => { struct A; struct B; } }\nconst X: u8 = 1;\n";
        assert_eq!(missing(source).unwrap(), BTreeSet::from([1]));
    }

    #[test]
    fn repository_declarations_are_separated() {
        for path in rust_files(&root()).unwrap() {
            let source = fs::read_to_string(&path).unwrap();
            let gaps =
                missing(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert!(
                gaps.is_empty(),
                "{}: missing gaps after lines {gaps:?}",
                path.display()
            );
        }
    }
}
