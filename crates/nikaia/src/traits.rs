// crates/nikaia/src/traits.rs
//
// Kap 4.7: what an `impl` owes the `trait` it names.
//
// **A walk of its own, and the reason is the ledger's `sync` column.** The type
// checker runs *before* `sync::infer` — that order is what keeps ADR-027 sound,
// because the checker resolves method calls and the inference reads the result —
// so a rule that needs the finished `sync` cannot live inside it. This runs
// afterwards, from `check_program`, beside `dsl::check` and `views::check` and
// for the same reason each of those is separate: it asks a question the type
// walk does not have the answer to.
//
// **Both rules here are refusals that cost nothing today.** Nothing in
// `examples/`, `tests/samples/` or `crates/nikaia-std/src/` declares a trait at
// all — [ADR-078](../../../docs/specification/adr/adr-078.md) made the
// declaration possible one commit ago — so this is `open-work.md` §2's own
// principle at its cheapest moment: *a refusal is free before programs exist and
// breaking afterwards*.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Item, Span};
use crate::check::{Finding, Severity};
use crate::contracts::Ledger;
use crate::parser::Parsed;

/// Every way an `impl` can fail the `trait` it names.
pub fn check(parsed: &Parsed, own: &Ledger) -> Vec<Finding> {
    let mut found = Vec::new();
    for item in &parsed.program.items {
        let Item::Impl {
            trait_name: Some(trait_name),
            target,
            methods,
        } = &item.node
        else {
            continue;
        };
        let named = parsed.text(*trait_name).to_string();
        // A trait this unit does not declare is one nothing here can check
        // against: `impl Error for ConfigError` names a trait the compiler reads
        // rather than one a `.nika` file wrote (ADR-023 D3), and a trait a
        // package publishes cannot be reached at all yet (ADR-078 §4). Silence
        // is the only correct answer about a declaration that is not here.
        let Some(declared) = own.traits.get(&named) else {
            continue;
        };
        let target_name = parsed.text(target.name).to_string();

        let mut given: BTreeMap<String, &Span> = BTreeMap::new();
        for method in methods {
            let Item::Fn { name: Some(name), .. } = &method.node else {
                continue;
            };
            given.insert(parsed.text(*name).to_string(), &method.span);
        }

        for (name, span) in &given {
            if !declared.contains(name) {
                found.push(not_in_the_trait(&named, &target_name, name, span));
                continue;
            }
            found.extend(pausing(own, &named, &target_name, name, span));
        }
        let missing: BTreeSet<&String> = declared
            .iter()
            .filter(|m| !given.contains_key(*m))
            .collect();
        if !missing.is_empty() {
            found.push(incomplete(&named, &target_name, &missing, &item.span));
        }
    }
    found.sort_by_key(|f| f.span.start);
    found
}

/// **`NK1129`: the implementation pauses and the declaration cannot say so.**
///
/// [ADR-078](../../../docs/specification/adr/adr-078.md) D4 asserts `sync` for a
/// trait's methods, and had to: a declaration has no body for `sync::infer` to
/// read, so `No` would have stood — and `No` means *pauses*, which made every
/// call through a bound an `.await` and `fn shout[T: Summarize]` an `async fn`
/// awaiting a `String`.
///
/// That is right for every trait whose methods do not pause and wrong for one
/// whose method does: the declaration lowers to `fn load(&self) -> …;` and the
/// `impl` to `async fn load(&self) -> …`, and the language below answers
/// *"method `load` has an incompatible type for trait"* about a file nobody
/// wrote — [Part III C.1](../../../docs/specification/30-nikaia-tooling.md)'s
/// class.
///
/// **Refused rather than lowered**, because `async fn` in a trait is a thing the
/// emitter has no way to ask for, and a message in this language's words about
/// the line the author wrote is the whole of what C.1 is asking for. What it
/// costs is a trait that describes I/O, which is named as the reopening
/// condition rather than left to be rediscovered.
fn pausing(
    own: &Ledger,
    trait_name: &str,
    target: &str,
    method: &str,
    span: &Span,
) -> Option<Finding> {
    let contract = own.functions.get(&format!("{target}::{method}"))?;
    if contract.sync.is_sync() {
        return None;
    }
    Some(Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1129",
        message: format!(
            "`{target}::{method}` can pause, and `{trait_name}` declares it as a method that \
             cannot"
        ),
        notes: vec![
            "a trait's methods are `sync` because a declaration has no body to read one \
             from, and a method that pauses would have to be declared as one - which this \
             compiler has no way to write (ADR-078 D4). Refused here rather than in the \
             language below, where it is a mismatch about a file nobody wrote"
                .to_string(),
        ],
        help: Some(format!(
            "give the body nothing that pauses - a file read, a sleep, a `.join()` - or take \
             `{method}` out of `{trait_name}` and call it on the type directly"
        )),
    })
}

/// **`NK1130`: the `impl` and the `trait` do not agree on which methods exist.**
///
/// The half [ADR-078](../../../docs/specification/adr/adr-078.md) §4 left open:
/// a trait existed to be checked against and nothing checked, so a method the
/// trait does not declare, or one it declares and the `impl` leaves out, went to
/// `rustc` — `E0407` and `E0046`, about the generated file.
///
/// Two messages rather than one code per direction, because a reader is doing
/// two different things: adding a method to the trait or taking one out of the
/// `impl`; and finishing an `impl` that is not done.
fn not_in_the_trait(trait_name: &str, target: &str, method: &str, span: &Span) -> Finding {
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1130",
        message: format!("`{trait_name}` declares no method `{method}`"),
        notes: vec![format!(
            "an `impl {trait_name} for {target}` gives the type that trait's behaviour, and \
             only that (Part I, 4.7) - a method of the type's own belongs in a plain \
             `impl {target}` beside it"
        )],
        help: Some(format!(
            "move it into `impl {target}`, or declare `{method}` in `{trait_name}` if every \
             type that implements it should have one"
        )),
    }
}

fn incomplete(
    trait_name: &str,
    target: &str,
    missing: &BTreeSet<&String>,
    span: &Span,
) -> Finding {
    let names: Vec<String> = missing.iter().map(|m| format!("`{m}`")).collect();
    Finding {
        severity: Severity::Error,
        span: span.clone(),
        code: "NK1130",
        message: format!(
            "`{target}` does not implement {} of `{trait_name}`",
            names.join(", ")
        ),
        notes: vec![
            "a trait is a promise that every one of its methods is there, which is what \
             lets a bound call them (Part I, 4.7)"
                .to_string(),
        ],
        help: Some(format!(
            "write {} in this `impl`, or take {} out of `{trait_name}`",
            names.join(", "),
            match names.len() {
                1 => "it".to_string(),
                _ => "them".to_string(),
            }
        )),
    }
}
