//! A `use` brings no name in, for `std` as for a package
//! ([ADR-140](../../../docs/specification/adr/adr-140.md) D5).
//!
//! [ADR-046](../../../docs/specification/adr/adr-046.md) D2's rule is the
//! language's and `std` was the one place it was not followed:
//! `use std::collections::HashMap` parsed, and what it did was **nothing** —
//! `HashMap` works with no `use` at all, because it is a name this compiler
//! already knows. A line that reads like an import and does nothing is what
//! this closes.

use nikaia::contracts::{Ledger, STD};
use nikaia::parser::parse_to_ast;

fn findings(source: &str) -> Vec<nikaia::check::Finding> {
    let parsed = parse_to_ast(source).expect("the source parses");
    let own = Ledger::infer(&parsed);
    let library = Ledger::parse(STD).expect("std's ledger");
    nikaia::check::check(&parsed, &own, &library).findings
}

/// **A `use` that names a type is refused** (D5), and the way out is to drop
/// the line.
#[test]
fn a_use_that_names_a_type_is_refused() {
    let found: Vec<_> = findings("use std::collections::HashMap\nfn main() { }\n")
        .into_iter()
        .filter(|f| f.code == "NK1156")
        .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0]
        .help
        .as_deref()
        .unwrap_or_default()
        .contains("drop the line"));
}

/// **And the name works with no `use` at all**, which is the prelude and is
/// unchanged: what goes is the `use` *acting differently* depending on what
/// follows it.
#[test]
fn the_prelude_needs_no_use() {
    let source = "fn main() {\n\
                  \x20   let mut scores = HashMap()\n\
                  \x20   scores[\"a\"] = 1\n\
                  \x20   println(f\"{scores.len()}\")\n\
                  }\n";
    assert!(findings(source).is_empty(), "{:#?}", findings(source));
}

/// **A module is left alone**, which is what a `use` names: `use std::fs` and
/// then `fs::read_to_string`.
#[test]
fn a_use_that_names_a_module_is_left_alone() {
    let found: Vec<_> =
        findings("use std::fs\nuse std::io\nuse std::cli\nuse std::html\nfn main() { }\n")
            .into_iter()
            .filter(|f| f.code == "NK1156")
            .collect();
    assert!(found.is_empty(), "{found:#?}");
}

/// **A module nothing describes yet is left alone too**, which is
/// [Part III C.4](../../../docs/specification/30-nikaia-tooling.md): refusing
/// on a surface that does not exist is a correct program refused.
#[test]
fn a_module_no_ledger_describes_is_not_refused() {
    let found: Vec<_> = findings("use std::db::postgres\nuse std::backend::x86\nfn main() { }\n")
        .into_iter()
        .filter(|f| f.code == "NK1156")
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}

/// **A package's `use` is not this rule's**, and its own refusal is the module
/// layer's ([ADR-046](../../../docs/specification/adr/adr-046.md) D2).
#[test]
fn a_packages_use_is_untouched() {
    let found: Vec<_> = findings("use http\nfn main() { }\n")
        .into_iter()
        .filter(|f| f.code == "NK1156")
        .collect();
    assert!(found.is_empty(), "{found:#?}");
}

/// **And nothing in the tree writes the form any more**, which is the half a
/// refusal cannot check for itself.
#[test]
fn no_nika_source_in_the_tree_brings_a_name_in() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut left = Vec::new();
    let mut walk = vec![root];
    while let Some(dir) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                if !matches!(name.as_str(), "target" | ".git" | "vendor" | "node_modules") {
                    walk.push(path);
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("nika") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("use std::collections::") {
                    left.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(left.is_empty(), "still brought in:\n{}", left.join("\n"));
}
