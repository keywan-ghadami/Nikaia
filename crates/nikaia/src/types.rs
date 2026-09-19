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

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Block, Item, Span, Stmt, Type};
use crate::check::{Finding, Severity};
use crate::contracts::Ledger;
use crate::parser::Parsed;

/// Every type name written in this unit that nothing accounts for.
pub fn check(parsed: &Parsed, own: &Ledger, library: &Ledger) -> Vec<Finding> {
    let mut found = Vec::new();
    declared_once(parsed, &mut found);
    let known = known_names(parsed, own, library);
    let traits = known_traits(parsed, own, library);
    for item in &parsed.program.items {
        let mut here = known.clone();
        here.extend(parameters_of(parsed, &item.node));
        bounds_of(parsed, &item.node, &traits, &item.span, &mut found);
        item_types(parsed, &item.node, &here, &item.span, &mut found);
    }
    found
}

/// The names a **bound** may have: a `trait` this unit declares, or one either
/// ledger records ([ADR-106](../../../docs/specification/adr/adr-106.md)).
///
/// A set of its own and not `known_names`, because the two questions are not
/// the same one: `[T: Summary]` asks for a trait where `x: Summary` asks for a
/// type, and a `struct` is not an answer to the first.
fn known_traits(parsed: &Parsed, own: &Ledger, library: &Ledger) -> BTreeSet<String> {
    let mut known: BTreeSet<String> = BTreeSet::new();
    for ledger in [own, library] {
        for key in ledger.traits.keys() {
            known.insert(key.clone());
            if let Some((_, last)) = key.rsplit_once("::") {
                known.insert(last.to_string());
            }
        }
    }
    // Declared in this very unit, which the ledger handed to this walk may not
    // carry - `check` runs per unit and a ledger is the program's.
    for item in &parsed.program.items {
        if let Item::Trait { name, .. } = &item.node {
            known.insert(parsed.text(*name).to_string());
        }
    }
    known
}

/// `NK1135` one position over: **a bound naming a trait nothing declares**.
///
/// `fn describe[T: Struct](value: T)` lowered to `fn describe<T: Struct>(…)` and
/// came back from the backend as *cannot find trait `Struct` in this scope*,
/// which is [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s
/// class and exactly what `NK1135` was built to close for a written type.
///
/// **Qualified names are left alone**, for that refusal's own reason: whether
/// this build can see the package a bound names is a question with its own
/// message.
fn bounds_of(
    parsed: &Parsed,
    item: &Item,
    traits: &BTreeSet<String>,
    span: &Span,
    out: &mut Vec<Finding>,
) {
    let generics = match item {
        Item::Fn { generics, .. } | Item::Struct { generics, .. } => generics.as_slice(),
        _ => return,
    };
    for parameter in generics {
        for bound in &parameter.bounds {
            let name = parsed.text(*bound);
            if !name.contains("::") && !traits.contains(name) {
                out.push(nothing_declares_a_trait(name, span));
            }
        }
    }
}

/// The message [`bounds_of`] raises, under `NK1135`'s code because it is the
/// same claim: a name written where a declaration has to exist, and none does.
fn nothing_declares_a_trait(name: &str, span: &Span) -> Finding {
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1135",
        message: format!("nothing declares the trait `{name}`"),
        notes: vec![
            "a bound names a trait - what a caller's type has to implement (Part I, 4.7) -              and this name is not one this program declares with `trait`, nor one a ledger              records (ADR-106)"
                .to_string(),
        ],
        help: Some(format!(
            "declare `{name}` with `trait`, or leave the bound off - a parameter without              one may be moved and passed and nothing else (ADR-074 D5)"
        )),
    }
}

/// **`NK1148`: a name declared twice in one file**
/// ([ADR-144](../../../docs/specification/adr/adr-144.md) D1).
///
/// Across files this is `modules::one_namespace`, which has something this does
/// not — two paths to name ([ADR-047](../../../docs/specification/adr/adr-047.md)
/// D1). Inside one file nothing said it, unless the build happened to go through
/// a manifest: `nikaia --input` skips the module layer, and that is the path the
/// corpus, the specification's blocks and a reader's first program all take. So
/// `struct Foo` beside `fn Foo` lowered, and `rustc` answered `E0428` about a
/// file nobody wrote ([Part III C.1](../../../docs/specification/30-nikaia-tooling.md)).
///
/// **Here and not in the module layer**, because every build runs the checker
/// and only some builds run that.
fn declared_once(parsed: &Parsed, out: &mut Vec<Finding>) {
    let mut seen: BTreeMap<String, &'static str> = BTreeMap::new();
    for item in &parsed.program.items {
        let Some((name, kind)) = declares(parsed, &item.node) else {
            continue;
        };
        match seen.get(&name) {
            // **The caret is on the second**, because that is the one that
            // arrived and the one to move; the note names the first.
            Some(first) => out.push(declared_twice(&name, first, kind, &item.span)),
            None => {
                seen.insert(name, kind);
            }
        }
    }
}

/// What declares a name, and what it is called in the message
/// ([ADR-144](../../../docs/specification/adr/adr-144.md) D2).
///
/// Five items and no more. A **method** belongs to its type and two types may
/// each have a `len`; a **rule** belongs to its grammar and is reached as
/// `Json::value`; a field, a variant, a type parameter and an `impl` block
/// declare nothing at this level.
fn declares(parsed: &Parsed, item: &Item) -> Option<(String, &'static str)> {
    match item {
        Item::Fn { name, .. } => name.map(|name| (parsed.text(name).to_string(), "fn")),
        Item::Struct { name, .. } => Some((parsed.text(*name).to_string(), "struct")),
        Item::Enum { name, .. } => Some((parsed.text(*name).to_string(), "enum")),
        Item::Trait { name, .. } => Some((parsed.text(*name).to_string(), "trait")),
        Item::Grammar(def) => Some((parsed.text(def.name).to_string(), "grammar")),
        _ => None,
    }
}

/// The message [`declared_once`] raises. Two of a kind get the same sentence as
/// two of different kinds, because the reader is doing the same thing either
/// way: finding out which of two declarations a line means.
fn declared_twice(name: &str, first: &str, second: &str, span: &Span) -> Finding {
    let kinds = match first == second {
        true => format!("both as a `{first}`"),
        false => format!("as a `{first}` and as a `{second}`"),
    };
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1148",
        message: format!("`{name}` is declared twice in this file: {kinds}"),
        notes: vec![
            "a name denotes one thing (ADR-144 D1), so a line that writes it has one \
             meaning and no rule is needed about which declaration wins - the files of a \
             package share one namespace for the same reason (Part I, 9.1)"
                .to_string(),
        ],
        help: Some(format!(
            "rename one of the two - a `struct` and its anonymous constructor are already \
             one name, so `{name}` cannot also be a function of its own"
        )),
    }
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
    // **`Array[T, N]`** ([ADR-152](../../../docs/specification/adr/adr-152.md)
    // D1), which is a name here for the same reason `Vec` is: nothing declares
    // it in a `.nika` file and the emitter writes it (`[T; N]`).
    "Array",
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
        // ([ADR-124](../../docs/specification/adr/adr-124.md) D1), and the
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
    // **A count is not a name either**
    // ([ADR-152](../../../docs/specification/adr/adr-152.md) D1): the `3` of
    // `Array[f64, 3]` is an argument of the type and nothing declares a type
    // called `3`. The walk over the arguments below reaches it, so the silence
    // has to be here rather than at the one position that writes one.
    if ty.count.is_some() {
        return;
    }
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
