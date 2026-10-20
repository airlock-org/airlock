//! Check blank lines between Rust declarations that stable rustfmt leaves alone.
//! Macro token bodies and Rust snippets in strings or documentation need review.
//
// Adapted from Rusix's crates/rusix-derive/src/bin/check-rust-spacing.rs.
// Copyright (c) 2026 Rusix contributors; MIT, see ../LICENSE.rusix.
use proc_macro2::Span;
use std::{collections::BTreeSet, fs, path::Path};
use syn::{Item, Stmt, spanned::Spanned, visit::Visit};

struct Spacing<'a> {
    /// Source lines used to distinguish separators from blank lines inside comments.
    lines: Vec<&'a str>,
    /// Source line positions where distinct item declarations need a separating blank line.
    missing: BTreeSet<usize>,
}

impl Spacing<'_> {
    fn pair(&mut self, previous: Span, next: Span) {
        let end = previous.end();
        let start = next.start();

        // Only whitespace and comments occur between these AST items. Track
        // nested block comments so an empty line inside a comment doesn't count.
        let mut depth = 0_usize;
        let mut insertion = end.line;
        let mut trailing_comment = false;

        for line in end.line..start.line {
            let text = self.lines[line - 1];
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
        lines: source.lines().collect(),
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
            && !excluded_directory(&entry.file_name())
            && !entry.path().join(".git").exists()
        {
            files.extend(rust_files(&entry.path())?);
        } else if kind.is_file() && entry.path().extension().is_some_and(|s| s == "rs") {
            let source = fs::read_to_string(entry.path())?;
            if !source.lines().take(5).any(|line| {
                let line = line.to_ascii_lowercase();
                line.contains("@generated") || line.contains("automatically generated")
            }) {
                files.push(entry.path());
            }
        }
    }

    files.sort();
    Ok(files)
}

// Do not follow symlinks, build trees, third-party code or independent checkouts.
fn excluded_directory(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.starts_with('.')
        || name == "result"
        || name.starts_with("result-")
        || matches!(
            name.as_ref(),
            "target" | "vendor" | "vendored" | "generated" | "node_modules" | "dist" | "build"
        )
}

fn root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|path| path.join("AGENTS.md").is_file() && path.join(".git").exists())
        .map(Path::to_owned)
        .ok_or_else(|| "cannot find Airlock repository root".into())
}

fn parse_error(path: &Path, error: syn::Error) -> String {
    let location = error.span().start();
    format!(
        "{}:{}:{}: {error}",
        path.display(),
        location.line,
        location.column + 1
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut fix = false;
    let mut selected_root = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--fix" if !fix => fix = true,
            "--root" if selected_root.is_none() => {
                selected_root = Some(std::path::PathBuf::from(
                    args.next().ok_or("--root needs a directory")?,
                ));
            }
            _ => return Err("usage: check-rust-spacing [--fix] [--root DIRECTORY]".into()),
        }
    }

    let root = selected_root.map(Ok).unwrap_or_else(root)?.canonicalize()?;
    let files = rust_files(&root)?;

    // Parse everything before modifying anything, and let stable rustfmt split
    // same-line items before inserting separators. It must be installed explicitly.
    for path in &files {
        syn::parse_file(&fs::read_to_string(path)?).map_err(|error| parse_error(path, error))?;
    }

    if fix {
        for path in &files {
            let status = std::process::Command::new("rustfmt")
                .args(["--edition", "2024", "--config", "skip_children=true", "--"])
                .arg(path)
                .status()?;
            if !status.success() {
                return Err(format!("rustfmt failed: {}", path.display()).into());
            }
        }
    }

    let mut violations = 0;

    for path in &files {
        let source = fs::read_to_string(path)?;
        let gaps = missing(&source).map_err(|error| parse_error(path, error))?;
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
                return Err(format!("{}: cannot safely insert separators", path.display()).into());
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
