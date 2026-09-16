// crates/nikaia/src/types.rs
//
// Part I 2.2 and Part III C.1: **a type nothing declares is refused here rather
// than lowered** ([ADR-096](../../../docs/specification/adr/adr-096.md)).
//
// A *value* nothing declares has had `NK1117` since
// [ADR-051](../../../docs/specification/adr/adr-051.md) — *"nothing declares
// `q`"*. A type had nothing, so `let x: Widgit = 3` lowered verbatim and came
// back as `rustc`'s *"cannot find type `Widgit` in this scope"*, about a file
// nobody wrote. Part III C.1's rule held for one half of this language's names
// and not the other.
//
// **A walk of its own**, beside `dsl::check`, `views::check` and
// `traits::check`, and for the reason each of those is separate: it asks a
// question about a *written name* rather than about a value's type, so it needs
// the item tree and neither the scope stack nor the inference.

use std::collections::BTreeSet;

use crate::ast::{Block, Item, Span, Stmt, Type};
use crate::check::{Finding, Severity};
use crate::contracts::Ledger;
use crate::parser::Parsed;

/// Every type name written in this unit that nothing accounts for.
pub fn check(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<Finding> {
    let mut found = Vec::new();
    let known = known_names(parsed, own, library);
    for item in &parsed.program.items {
        let mut here = known.clone();
        here.extend(parameters_of(parsed, &item.node));
        item_types(parsed, &item.node, &here, &item.span, &mut found);
    }
    found
}

/// The names a written type may have.
///
/// **Built from what the compiler already knows rather than from a list**, and
/// that is the whole of why this can be answered at all: `OFFERED` is Part I
/// 2.2's own set, the ledger's `types` map is every type this program and
/// `std` declare, and the rest are the names the emitter itself writes. A list
/// maintained beside those would be a second thing to forget.
fn known_names(parsed: &Parsed, own: &Ledger, library: &Ledger) -> BTreeSet<String> {
    let mut known: BTreeSet<String> = BUILT_IN.iter().map(|n| n.to_string()).collect();
    for ledger in [own, library] {
        for key in ledger.types.keys() {
            known.insert(key.clone());
            // A library writes the module in front of its types
            // (`fs::Mapped`), and a program writes the suffix where the module
            // is in scope - name-for-name resolution, the same rule
            // `Ledger::method` uses.
            if let Some((_, last)) = key.rsplit_once("::") {
                known.insert(last.to_string());
            }
        }
    }
    // A type declared in this very unit, which the ledger handed to this walk
    // may not carry: `check` runs per unit and a ledger is the program's.
    known.extend(crate::contracts::declared_types(parsed));
    known
}

/// The names the emitter writes that no ledger declares.
///
/// Part I 2.2's numeric surface beyond the four `OFFERED` names
/// ([ADR-054](../../../docs/specification/adr/adr-054.md) D1 is about what `as`
/// may *name*, which is a smaller set than what may be *written*), the
/// collections the prelude provides, and the hulls.
const BUILT_IN: &[&str] = &[
    "i8",
    "i16",
    "i32",
    "i64",
    "i128",
    "isize",
    "u8",
    "u16",
    "u32",
    "u64",
    "u128",
    "usize",
    "f32",
    "f64",
    "bool",
    "char",
    "String",
    "str",
    "Self",
    "Vec",
    "HashMap",
    "BTreeMap",
    "HashSet",
    "BTreeSet",
    "Shared",
    "SharedMut",
    "Locked",
    "TaskHandle",
];

/// The type parameters an item brings into scope for its own body.
fn parameters_of(parsed: &Parsed, item: &Item) -> BTreeSet<String> {
    match item {
        Item::Fn { generics, .. } | Item::Struct { generics, .. } => generics
            .iter()
            .map(|g| parsed.text(g.name).to_string())
            .collect(),
        Item::Impl { target, .. } => {
            let declared = crate::contracts::declared_types(parsed);
            crate::contracts::impl_parameters(parsed, target, &declared)
                .into_iter()
                .collect()
        }
        _ => BTreeSet::new(),
    }
}

fn item_types(
    parsed: &Parsed,
    item: &Item,
    known: &BTreeSet<String>,
    span: &Span,
    out: &mut Vec<Finding>,
) {
    match item {
        Item::Fn {
            args,
            ret_type,
            body,
            ..
        } => {
            for arg in args {
                written_at(parsed, &arg.ty, known, Position::Parameter, span, out);
            }
            if let Some(ret) = ret_type {
                written(parsed, ret, known, span, out);
            }
            block_types(parsed, body, known, out);
        }
        Item::Struct { fields, .. } => {
            for field in fields {
                written(parsed, &field.ty, known, span, out);
            }
        }
        Item::Enum { variants, .. } => {
            for variant in variants {
                match &variant.fields {
                    crate::ast::VariantFields::Unit => {}
                    crate::ast::VariantFields::Tuple(parts) => {
                        for ty in parts {
                            written(parsed, ty, known, span, out);
                        }
                    }
                    crate::ast::VariantFields::Named(fields) => {
                        for field in fields {
                            written(parsed, &field.ty, known, span, out);
                        }
                    }
                }
            }
        }
        // **An `extern "C"` declaration's types are written types**
        // ([ADR-121](../../docs/specification/adr/adr-121.md) D1), and the
        // reason this arm has to exist is Part III 15.1's own example:
        // `Pointer[u8]` is a type nothing declares, and without this it reached
        // `rustc` intact and came back about a file nobody wrote — which is the
        // whole class `NK1135` was built for.
        //
        // `Position::Parameter` for the arguments, as a function's are: a
        // declaration is a signature, and the run-or-kept question `NK1142`
        // asks is about where the type stands rather than about whose body it
        // is in.
        Item::Extern { declarations, .. } => {
            for declaration in declarations {
                for arg in &declaration.node.args {
                    written_at(
                        parsed,
                        &arg.ty,
                        known,
                        Position::Parameter,
                        &declaration.span,
                        out,
                    );
                }
                if let Some(ret) = &declaration.node.ret_type {
                    written(parsed, ret, known, &declaration.span, out);
                }
            }
        }
        Item::Trait { methods, .. } => {
            for method in methods {
                let mut here = known.clone();
                here.extend(
                    method
                        .node
                        .generics
                        .iter()
                        .map(|g| parsed.text(g.name).to_string()),
                );
                for arg in &method.node.args {
                    written_at(
                        parsed,
                        &arg.ty,
                        &here,
                        Position::Parameter,
                        &method.span,
                        out,
                    );
                }
                if let Some(ret) = &method.node.ret_type {
                    written(parsed, ret, &here, &method.span, out);
                }
            }
        }
        Item::Impl { methods, .. } => {
            for method in methods {
                let mut here = known.clone();
                here.extend(parameters_of(parsed, &method.node));
                item_types(parsed, &method.node, &here, &method.span, out);
            }
        }
        _ => {}
    }
}

fn block_types(parsed: &Parsed, block: &Block, known: &BTreeSet<String>, out: &mut Vec<Finding>) {
    for stmt in &block.stmts {
        if let Stmt::Let {
            ty: Some(ty),
            value,
            ..
        } = &stmt.node
        {
            written(parsed, ty, known, &stmt.span, out);
            let _ = value;
        }
        let mut inner: Vec<&Block> = Vec::new();
        crate::contracts::sync::visit_stmt_blocks(&stmt.node, &mut |b| inner.push(b));
        for b in inner {
            block_types(parsed, b, known, out);
        }
    }
}

/// One written type, and the arguments inside it.
///
/// **A tuple has no name**, so it is skipped and its parts are walked — the AST
/// puts them where the arguments go, and reading `name` there would read a
/// token nobody wrote as a type.
/// Where a written type stands, for the one rule that cares
/// ([ADR-102](../../../docs/specification/adr/adr-102.md) D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    /// A parameter of a function, a method or a trait method.
    Parameter,
    /// A struct field, an enum variant's part, a result, a `let`'s annotation —
    /// every position a *kept* function type would stand in.
    Elsewhere,
}

fn written(
    parsed: &Parsed,
    ty: &Type,
    known: &BTreeSet<String>,
    span: &Span,
    out: &mut Vec<Finding>,
) {
    written_at(parsed, ty, known, Position::Elsewhere, span, out)
}

fn written_at(
    parsed: &Parsed,
    ty: &Type,
    known: &BTreeSet<String>,
    at: Position,
    span: &Span,
    out: &mut Vec<Finding>,
) {
    // **A function type is not a name**
    // ([ADR-102](../../../docs/specification/adr/adr-102.md) D1), the same way
    // a tuple is not: what `fn` holds is a shape, and its parameters are in
    // `generics` where a tuple's parts are. Reading `name` here would report
    // that nothing declares a type called `fn`.
    if !ty.is_tuple && ty.code.is_none() {
        let name = parsed.text(ty.name);
        if !known.contains(name) && !name.contains("::") {
            out.push(nothing_declares(name, span));
        }
    }
    for argument in &ty.generics {
        written(parsed, argument, known, span, out);
    }
    if let Some(code) = &ty.code {
        if let Some(result) = &code.result {
            written(parsed, result, known, span, out);
        }
        if at == Position::Elsewhere {
            out.push(only_a_parameter_yet(span));
        }
    }
}

/// `NK1142`: a function type outside a parameter, which is
/// [ADR-102](../../../docs/specification/adr/adr-102.md) D5's **kept** lowering
/// and is not built.
///
/// D1 says a function type may stand wherever a type may — a parameter, a
/// struct field, a result — and D5 says the two cases lower differently: a
/// **run** parameter is a closure argument, which is what `std`'s own
/// higher-order entries are, and a **kept** one is a boxed closure over a boxed
/// future. Only the first is built.
///
/// **It is refused rather than emitted**, which is the choice
/// [Part III C.1](../../../docs/specification/30-nikaia-tooling.md) makes for
/// this compiler: a field written `impl Fn(…)` is not Rust, and what the reader
/// would get is the backend's words about a file nobody wrote. A refusal in
/// this compiler's own words, naming what is missing, is the honest half of a
/// record that is built in steps.
fn only_a_parameter_yet(span: &Span) -> Finding {
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1142",
        message: "a function type may only be a parameter in this compiler".to_string(),
        notes: vec![
            "a parameter the callee **runs** lowers to a closure argument, which is what \
             `std`'s own `map` and `access` take; a function type in a field, a result or \
             a `let` is one the callee **keeps**, and that lowering is not built \
             (ADR-102 D5)"
                .to_string(),
            "it is refused here rather than handed to the language below, because what \
             comes back from there is about the generated file (Part III, C.1)"
                .to_string(),
        ],
        help: Some(
            "take the code in as a parameter and call it during the call - or hold it \
             behind a type of your own until the kept lowering lands"
                .to_string(),
        ),
    }
}

/// `NK1135`: a written type name nothing accounts for.
///
/// **Qualified names are left alone**, and that is deliberate rather than
/// unfinished: `http::Response` names a package's type, and whether this build
/// can see that package is [ADR-046](../../../docs/specification/adr/adr-046.md)
/// D2's question with its own message. Saying *"nothing declares it"* about a
/// name a dependency does declare would be
/// [Part III C.4](../../../docs/specification/30-nikaia-tooling.md) — a correct
/// program refused — which is the one thing this may not do.
fn nothing_declares(name: &str, span: &Span) -> Finding {
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1135",
        message: format!("nothing declares the type `{name}`"),
        notes: vec![
            "a type is one Part I 2.2 offers, one this program declares with `struct` or \
             `enum`, one `std` publishes, or a parameter the declaration around it names \
             (Part I, 2.2)"
                .to_string(),
        ],
        help: Some(format!(
            "declare `{name}` with `struct` or `enum`, or write a type that exists - a \
             misspelling is the usual cause"
        )),
    }
}
