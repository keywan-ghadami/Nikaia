// crates/nikaia/src/diagnostics/mod.rs
//
// Where an error is reported.
//
// Stage 0 emits Rust and hands it to `rustc`, which then reports everything -
// its own type errors and the parser backend's frame check alike - against a
// file the user never wrote. That is not a small annoyance: it is the whole
// difference between a compiler and a code generator with a compiler bolted on.
//
// The fix is not a second checker (ADR-011 D3). It is a map: the emitter
// records which `.nika` span produced which byte range of the emitted Rust
// (`emit::SourceMap`), rustc reports byte offsets into that same text with
// `--error-format=json`, and this module trades one for the other.
//
// What it deliberately does not do is rewrite the message. Because the lowering
// is name-for-name (ADR-011 D2), "`until(…)` in rule `NAME` can run through the
// boundary" is already a sentence about the Nikaia source - only its position
// was wrong. Correcting a position is a lookup; translating a text would be a
// second compiler.

use serde_json::Value;

use crate::ast::Span;
use crate::emit::SourceMap;

/// One message, placed in the `.nika` file where that was possible.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    /// Where in the `.nika` source, if the map knew.
    pub location: Option<Location>,
    /// Where in the emitted Rust it was reported, kept whether or not the map
    /// knew: an unmapped error still has to be reportable, and a mapped one is
    /// worth being able to check against its origin.
    pub generated_line: Option<usize>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Location {
    /// 1-based, as an editor counts.
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

/// Turn `rustc --error-format=json` output about the emitted Rust into
/// messages about the Nikaia source.
///
/// Lines that are not JSON, and diagnostics that carry no information about a
/// place (rustc's own "aborting due to N previous errors"), are dropped.
pub fn translate(rustc_json: &str, map: &SourceMap, source: &str) -> Vec<Diagnostic> {
    let lines = LineIndex::new(source);
    let mut out = Vec::new();

    for line in rustc_json.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };

        let level = value["level"].as_str().unwrap_or_default();
        if !matches!(level, "error" | "warning") {
            continue;
        }

        let message = value["message"].as_str().unwrap_or_default().to_string();
        if message.starts_with("aborting due to") {
            continue;
        }

        let primary = value["spans"].as_array().and_then(|spans| {
            spans
                .iter()
                .find(|s| s["is_primary"].as_bool() == Some(true))
        });

        let generated_line = primary
            .and_then(|s| s["line_start"].as_u64())
            .map(|l| l as usize);
        let location = primary
            .and_then(|s| s["byte_start"].as_u64())
            .and_then(|offset| map.source_span(offset as usize))
            .map(|span| lines.locate(span));

        let notes = value["children"]
            .as_array()
            .map(|children| {
                children
                    .iter()
                    .filter(|c| matches!(c["level"].as_str(), Some("note") | Some("help")))
                    .filter_map(|c| c["message"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        out.push(Diagnostic {
            level: level.to_string(),
            message,
            location,
            generated_line,
            notes,
        });
    }

    out
}

/// Render a diagnostic the way a compiler does: the place, the message, the
/// line it is about, and what part of it.
pub fn render(diagnostic: &Diagnostic, path: &str, source: &str, generated_path: &str) -> String {
    let mut out = String::new();

    match &diagnostic.location {
        Some(location) => {
            out.push_str(&format!(
                "{}: {}:{}:{}: {}\n",
                diagnostic.level, path, location.line, location.column, diagnostic.message
            ));

            if let Some(text) = source.lines().nth(location.line - 1) {
                let gutter = format!("{:>4} | ", location.line);
                out.push_str(&gutter);
                out.push_str(text);
                out.push('\n');

                // The caret sits under the span, counting characters so that a
                // station name in the source does not shift it.
                let width = source[diagnostic
                    .location
                    .as_ref()
                    .map(|l| l.span.clone())
                    .unwrap_or(0..0)]
                .chars()
                .take_while(|c| *c != '\n')
                .count()
                .max(1);
                out.push_str(&" ".repeat(gutter.len() + location.column - 1));
                out.push_str(&"^".repeat(width));
                out.push('\n');
            }
        }
        None => {
            // Nothing in the map covers it: the emitted line is all there is,
            // and saying so is better than pointing somewhere plausible.
            let line = diagnostic
                .generated_line
                .map(|l| format!("{generated_path}:{l}"))
                .unwrap_or_else(|| generated_path.to_string());
            out.push_str(&format!(
                "{}: {}: {} (no Nikaia source maps to this)\n",
                diagnostic.level, line, diagnostic.message
            ));
        }
    }

    for note in &diagnostic.notes {
        out.push_str(&format!("     = {note}\n"));
    }

    out
}

/// A finding of the compiler's own, reported the way it reports rustc's.
///
/// The `.nika` file, the line it is about with a caret under it, then the
/// reasons and a way out. `--explain` has shown rustc's diagnostics in this
/// shape since ADR-012; a rule the compiler checks itself should not look
/// different from one it relays.
pub fn render_sync_violation(
    violation: &crate::contracts::sync::Violation,
    path: &str,
    source: &str,
) -> String {
    let (line, column) = winnow_grammar::span::line_column(source, violation.span.start);
    let ledger = if violation.from_library {
        "`std`'s ledger"
    } else {
        "this program's contracts"
    };

    let mut out = String::new();
    out.push_str(&format!(
        "error[NK2202]: `{}` is `sync`, and `{}` can pause\n",
        violation.caller, violation.callee
    ));
    out.push_str(&format!("  --> {path}:{line}:{column}\n"));
    out.push_str(&winnow_grammar::span::caret(
        source,
        violation.span.start,
        1,
    ));
    out.push('\n');
    out.push_str(
        "     = a `sync` function promises it cannot pause and does no I/O (Part II, 12.1)\n",
    );
    out.push_str(&format!(
        "     = `{}` carries no `sync` in {ledger}\n",
        violation.callee
    ));
    out.push_str(&format!(
        "     help: drop `sync` from `{}`, or move the call out of it\n",
        violation.caller
    ));
    out
}

/// One of the type checker's findings, on the `.nika` line it is about.
///
/// The same shape `NK2202` and every relayed `rustc` message use, because a
/// rule the compiler checks itself should not look different from one it
/// relays (ADR-012). Part III C.2 asks for a headline, the reason, and one
/// concrete way out; a `Finding` carries all three and this writes them down.
pub fn render_finding(finding: &crate::check::Finding, path: &str, source: &str) -> String {
    let (line, column) = winnow_grammar::span::line_column(source, finding.span.start);

    let mut out = String::new();
    out.push_str(&format!("error[{}]: {}\n", finding.code, finding.message));
    out.push_str(&format!("  --> {path}:{line}:{column}\n"));
    out.push_str(&winnow_grammar::span::caret(source, finding.span.start, 1));
    out.push('\n');
    for note in &finding.notes {
        out.push_str(&format!("     = {note}\n"));
    }
    if let Some(help) = &finding.help {
        out.push_str(&format!("     help: {help}\n"));
    }
    out
}

/// Byte offset -> line and column, computed once per file.
struct LineIndex {
    /// Byte offset of the start of each line.
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .char_indices()
                .filter(|(_, c)| *c == '\n')
                .map(|(i, _)| i + 1),
        );
        Self { starts }
    }

    fn locate(&self, span: Span) -> Location {
        let line = match self.starts.binary_search(&span.start) {
            Ok(i) => i,
            Err(i) => i - 1,
        };

        Location {
            line: line + 1,
            column: span.start - self.starts[line] + 1,
            span,
        }
    }
}
