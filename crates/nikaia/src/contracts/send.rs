// crates/nikaia/src/contracts/send.rs
//
// Whether a value may cross a thread (ADR-005 §1 Group B, `NK25xx`).
//
// One question, asked of a **type** and not of a place:
//
//     may a value of this type be on a thread other than the one that built it?
//
// ADR-038 D7 states it as "a value may only cross into a foreign thread if it
// may cross any thread", and the reason it is a property of the type rather than
// of the crossing is ADR-005 Group B: the answer has to be **the same at both
// settings of `user_parallelism`**, so that a library written at one setting
// cannot turn out to be un-compilable where it is used. A check that consulted
// the switch would answer `yes` for a type whose expansion is safe at one
// setting and not at the other, and `no` for the same type at the other - which
// is exactly the asymmetry Group B, `NK25xx` and ADR-037 §3 were all written to
// prevent. So **no switch reaches this file, and none may** - the same sentence
// `order.rs` opens with, for the same reason.
//
// ## What ADR-037 D6 changed here, and what it did not
//
// `Shared` was that type, and it was the file's only `MayNot`. D6 removed the
// cause rather than the symptom: `Shared` expands to an atomic count at **both**
// settings, so its expansion no longer moves with the switch and there is
// nothing for the verdict to take the worse of. `Shared` therefore joins
// `CONTAINERS` and is answered by what it holds.
//
// **The rule above is untouched.** The verdict still never reads
// `user_parallelism`, and it is still a property of the type. What changed is
// one row of a table, and what follows from it is that **no type answers
// `MayNot` today** - see [`Crossing::MayNot`], which says why the arm stays.
//
// ## Three answers and not two
//
// [`Crossing`] has a third arm, and it is the whole design. `Undecided` is not
// `May`: ADR-010 D1 says an analysis that fails open is a vulnerability
// generator, and "nothing is written down about this type" is the absence of an
// answer rather than permission. But it is not a refusal either, because the
// one thing this compiler may never do is reject a program that is correct
// (Part III C.4), and Stage 0 knows the type of rather less than half of what a
// program writes.
//
// What the two non-`May` answers cost therefore depends on **who chose the
// crossing**, and the split is the reason the polarity and the no-false-refusal
// promise can both be kept:
//
//   * A crossing **the compiler chose** - the `task::both` that statement
//     overlapping emits (ADR-033) - is a step the compiler was never obliged to
//     take. Anything but `May` means it does not take it: the statements keep
//     the order they were written in. Nothing is refused and only speed is
//     spent, which is `order.rs`'s own polarity ("every `false` is either a real
//     dependency or an admission of ignorance, and the two are deliberately
//     worth the same").
//   * A crossing **the program wrote** - `spawn`, a value handed to a call this
//     compiler cannot see the end of - is refused on `MayNot` and left to the
//     backend on `Undecided`. A refusal on `Undecided` would reject correct
//     programs; silence is not what it buys instead, because `rustc` still
//     type-checks the emitted crate (ADR-002 §3) and ADR-005 D7's `E0277`
//     translation now reports that refusal against the `.nika` line. So an
//     undecided crossing is never *accepted* here - it is handed on.
//
// ## What the table knows
//
// A closed list, like `touch::KINDS` and for the same reason: a name this file
// does not know is answered `Undecided`, and a name it knows wrongly would be a
// typo that bought a crossing. The list grows when a type needs it, never
// speculatively (ADR-028 D5).
//
// Every name in [`PLAIN`] and [`CONTAINERS`] is safe both to **move** to
// another thread and to be **looked at** from one, which is why a view (`&T`)
// asks the same question as the value. The day a name belongs in one column and
// not the other, this needs a second column; stating that here is cheaper than
// discovering it.

use std::collections::BTreeSet;

use crate::ast::Expr;
use crate::parser::Parsed;

use super::{ty::Ty, Ledger};

/// Plain data: a value of one of these carries nothing that a second thread
/// could be wrong about.
const PLAIN: &[&str] = &[
    "bool", "char", "str", "String", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16",
    "u32", "u64", "u128", "usize", "f32", "f64",
];

/// A container is whatever it holds: it may cross exactly when every one of its
/// arguments may.
///
/// **`Shared` is in this list since
/// [ADR-037](../../../../docs/specification/adr/adr-037.md) D6**, and that is the
/// whole of step 1. It used to be the one type the records made `MayNot`,
/// because D3 expanded it from `user_parallelism` - a plain count at `no`, an
/// atomic one at `yes` - and a verdict that may not consult the switch has to
/// take the worse of the two settings for a type whose expansion moves with it.
/// D6 stops the expansion moving: `Shared` is atomic at both settings, so it is
/// a container like any other and is answered by what it holds.
///
/// Which is also why the single-column invariant in the module header matters
/// here rather than being a note for later. An atomic count may cross only
/// where the value under it may both **move** to another thread and be **looked
/// at** from one - `Arc<T>` in the language below - and that is exactly the
/// property every name in [`PLAIN`] and this list is required to have.
const CONTAINERS: &[&str] = &[
    "BTreeMap", "BTreeSet", "HashMap", "HashSet", "List", "Option", "Result", "Shared", "Vec",
];

/// How far into a type this walks before giving up. A struct that holds itself
/// through a container is a shape the ledger can hold, and this must terminate
/// on one.
const DEPTH: usize = 16;

/// What crossing a thread would mean for a value of one type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crossing {
    /// Every part of it may cross, and this compiler can see every part.
    May,
    /// A part of it may not, and the records say which.
    ///
    /// **No type answers this today, and that is a consequence of
    /// [ADR-037](../../../../docs/specification/adr/adr-037.md) D6 rather than an
    /// oversight.** `Shared` was the one type the records made non-crossable,
    /// and D6 made it crossable by giving it one representation. The arm stays
    /// because the reason it existed has not gone away - a type whose expansion
    /// still moves with `user_parallelism` has to take the worse setting - and
    /// `Locked` is the candidate D3's second half is about. Nothing here decides
    /// that.
    ///
    /// So `NK2501` and `NK2502` are built, reachable, and fire on no program
    /// this language can write. The walk below still has to be able to produce
    /// this, which is why the arm is exercised directly in the tests rather than
    /// through a source.
    MayNot {
        /// The part the answer is about - the whole type where the type itself
        /// is the problem, and the field's type where a field is.
        part: String,
        /// Where in the type it was found, for a note that can be read: empty
        /// for the value itself, `"its field `counts`"` for a field.
        at: Option<String>,
    },
    /// Nothing written down decides it. **Not permission** (ADR-010 D1).
    Undecided {
        /// The part nothing is written down about.
        part: String,
    },
}

impl Crossing {
    /// Whether a value of the type may cross - which is true of [`Crossing::May`]
    /// and of nothing else.
    pub fn may(&self) -> bool {
        matches!(self, Crossing::May)
    }

    /// The refusal, where the answer is one.
    pub fn refused(&self) -> Option<(&str, Option<&str>)> {
        match self {
            Crossing::MayNot { part, at } => Some((part, at.as_deref())),
            _ => None,
        }
    }

    /// One concrete way out, as Part III C.2 requires of every diagnostic.
    /// `None` where there is nothing to get out of.
    ///
    /// One sentence, because a refusal has one honest remedy: a value that may
    /// not cross stays where it was built. It used to have a second - "write
    /// `Vec[i64]` instead of `Shared[Vec[i64]]` and give each thread its own" -
    /// which went with [`Crossing::MayNot`]'s only producer (ADR-037 D6).
    pub fn way_out(&self) -> Option<String> {
        self.refused()?;
        Some("keep the value on the thread that built it".to_string())
    }

    /// One line saying why a value of this type may not cross, for a
    /// diagnostic's note. `None` where it may.
    ///
    /// Part III C.2: no Rust vocabulary, and the sentence has to mean something
    /// to a reader who has never heard of a reference count either. Which is why
    /// the `MayNot` line no longer names one: the count was `Shared`'s reason and
    /// `Shared` is not this answer any more, so the sentence says what the walk
    /// actually knows - the records name this part, and they name it at both
    /// settings.
    pub fn note(&self) -> Option<String> {
        match self {
            Crossing::May => None,
            Crossing::MayNot { part, at } => Some(format!(
                "`{part}`{} is one the records say may not be on a thread other than the one \
                 that built it, at either setting of `user_parallelism` (Part III, C.5)",
                match at {
                    Some(at) => format!(", {at},"),
                    None => String::new(),
                }
            )),
            Crossing::Undecided { part } => Some(format!(
                "nothing written down says whether `{part}` may cross a thread, so this \
                 compiler does not assume it may"
            )),
        }
    }
}

/// Every name a task's body mentions.
///
/// The ordering analysis's walk, asked a second question: it is **total and
/// over-approximate on purpose** (a field access `a.b` contributes `a`, a
/// method name counts as a name, the words of a `dsl` body count), and both
/// properties are what this needs. Over-approximate is the safe direction here
/// too - a name that is not a variable is not in scope, so it is looked up,
/// not found, and says nothing.
pub fn names_used(parsed: &Parsed, body: &Expr) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    super::order::names_in(parsed, body, &mut out);
    out
}

/// Whether a value of `ty` may cross a thread.
///
/// `own` is the program's ledger and `library` is `std`'s: between them they
/// hold the fields of every type that is written down (ADR-024), which is what
/// makes this **structural and transitive** as Group B requires - a struct with
/// one `Shared` field is no more crossable than the `Shared` itself.
pub fn crossing(ty: &Ty, own: &Ledger, library: &Ledger) -> Crossing {
    let mut seen = BTreeSet::new();
    walk(ty, own, library, &mut seen, DEPTH)
}

fn walk(
    ty: &Ty,
    own: &Ledger,
    library: &Ledger,
    seen: &mut BTreeSet<String>,
    depth: usize,
) -> Crossing {
    if depth == 0 {
        return Crossing::Undecided { part: ty.text() };
    }
    match ty {
        // The absence of a claim, and a claim is what this would need.
        Ty::Unknown => Crossing::Undecided {
            part: "?".to_string(),
        },
        // A library signature's variable that nothing bound. `substitute` turns
        // a bound one into the type it bound to before this is ever asked, so
        // one that arrives here is the absence of an answer (ADR-031).
        Ty::Var { .. } => Crossing::Undecided { part: ty.text() },
        // A lambda is its captures, and nothing writes those down.
        Ty::Fn { .. } => Crossing::Undecided { part: ty.text() },
        // `()` holds nothing, so there is nothing to be wrong about; a pair is
        // its parts.
        Ty::Tuple(parts) => join(parts.iter().map(|p| walk(p, own, library, seen, depth - 1))),
        Ty::Named { name, args, .. } => {
            if PLAIN.contains(&name.as_str()) {
                // A plain type with arguments is not the plain type: `String[T]`
                // is a name this compiler does not know.
                return match args.is_empty() {
                    true => Crossing::May,
                    false => Crossing::Undecided { part: ty.text() },
                };
            }
            if CONTAINERS.contains(&name.as_str()) {
                // A container with no arguments said nothing about what it
                // holds, and what it holds is the whole question.
                if args.is_empty() {
                    return Crossing::Undecided { part: ty.text() };
                }
                return join(args.iter().map(|a| walk(a, own, library, seen, depth - 1)));
            }
            let Some(contract) = described(name, own, library) else {
                return Crossing::Undecided { part: ty.text() };
            };
            // A type whose parts are Rust says so in one line, exactly as a
            // Rust function says `sync = true` about a body this compiler does
            // not read. The claim is about the type and not about its
            // arguments, which is why a `crosses` type with arguments is not
            // read further: nothing in `std` has one, and the day something
            // does, it is the arguments that want a rule.
            if contract.crosses && args.is_empty() {
                return Crossing::May;
            }
            Some(contract.fields.as_slice())
                .filter(|fields| !fields.is_empty())
                .map(|fields| {
                    // A type that holds itself is answered by its other fields:
                    // the cycle adds no new part to be wrong about, and walking
                    // it again would not terminate.
                    if !seen.insert(name.clone()) {
                        return Crossing::May;
                    }
                    let answer = join(fields.iter().map(|(field, ty)| {
                        name_the_field(field, walk(ty, own, library, seen, depth - 1))
                    }));
                    seen.remove(name);
                    answer
                })
                .unwrap_or_else(|| Crossing::Undecided { part: ty.text() })
        }
    }
}

/// What the ledgers say about a type, by the name a program writes.
///
/// The program's own ledger first, then `std`'s - the order every other
/// resolution in this compiler uses. A library writes the module in front of a
/// type (`fs::Mapped`) and a signature does not carry one, so the suffix is what
/// matches: name-for-name resolution (ADR-011 D2), as everywhere else.
///
/// **A type with no fields recorded is not a type with no fields.** `fs::Mapped`
/// has an entry and an empty `fields`, because its fields are Rust. So an empty
/// list is answered as "nothing is written down" rather than as "it holds
/// nothing", and `crosses` is the line that answers for such a type.
fn described<'a>(
    name: &str,
    own: &'a Ledger,
    library: &'a Ledger,
) -> Option<&'a crate::contracts::TypeContract> {
    let suffix = format!("::{name}");
    [own, library].into_iter().find_map(|ledger| {
        ledger.types.get(name).or_else(|| {
            ledger
                .types
                .iter()
                .find(|(key, _)| key.ends_with(&suffix))
                .map(|(_, contract)| contract)
        })
    })
}

/// Say which field an answer came from, where it is not `May`.
fn name_the_field(field: &str, answer: Crossing) -> Crossing {
    match answer {
        Crossing::MayNot { part, at: None } => Crossing::MayNot {
            part,
            at: Some(format!("which its field `{field}` holds")),
        },
        other => other,
    }
}

/// The worst answer of several, with `MayNot` worse than `Undecided`.
///
/// A refusal a reader can act on beats an admission of ignorance, which is
/// ADR-033 D9's rule for the ordering report applied to the same kind of choice:
/// of two true answers, print the one somebody can do something about.
fn join(answers: impl Iterator<Item = Crossing>) -> Crossing {
    let mut worst = Crossing::May;
    for answer in answers {
        match (&worst, &answer) {
            (Crossing::MayNot { .. }, _) => return worst,
            (_, Crossing::MayNot { .. }) => worst = answer,
            (Crossing::May, Crossing::Undecided { .. }) => worst = answer,
            _ => {}
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::TypeContract;

    fn ledgers() -> (Ledger, Ledger) {
        (
            Ledger::empty(),
            Ledger::parse(super::super::STD).expect("std ships a ledger"),
        )
    }

    fn of(text: &str) -> Crossing {
        let (own, library) = ledgers();
        crossing(&Ty::parse(text), &own, &library)
    }

    /// Plain data crosses, and so does a container of it - at any depth.
    #[test]
    fn plain_data_and_containers_of_it_may_cross() {
        for text in [
            "i64",
            "String",
            "&str",
            "bool",
            "Vec[i64]",
            "HashMap[String, Vec[i64]]",
            "(i64, String)",
            "()",
            "Option[Vec[(String, f64)]]",
        ] {
            assert_eq!(of(text), Crossing::May, "{text}");
        }
    }

    /// `Shared` is answered by what it holds, at either setting - ADR-037 D6,
    /// and the one row of the table step 1 moved.
    ///
    /// The count is atomic at both settings now, so there is nothing for the
    /// verdict to take the worse of and `Shared` is a container like `Vec`.
    #[test]
    fn a_shared_is_answered_by_what_it_holds() {
        assert_eq!(of("Shared[String]"), Crossing::May);
        assert_eq!(of("&Shared[String]"), Crossing::May);
        assert_eq!(of("Vec[Shared[i64]]"), Crossing::May);
        assert_eq!(of("(i64, Shared[i64])"), Crossing::May);

        // …and it is answered by what it holds in the other direction too: a
        // `Shared` of something nothing describes is undecided, not permitted.
        // Part II 12.2's counter is this case, and `Locked` is what decides it -
        // which ADR-037 D3's second half leaves open and D6 does not touch.
        assert!(matches!(
            of("Shared[Locked[i32]]"),
            Crossing::Undecided { .. }
        ));
        assert!(matches!(
            of("HashMap[String, Shared[Locked[i64]]]"),
            Crossing::Undecided { .. }
        ));
        // A bare `Shared` said nothing about what it holds, and what it holds is
        // the whole question.
        assert!(matches!(of("Shared"), Crossing::Undecided { .. }));
    }

    /// No type this walk can be asked about answers `MayNot`, and the arm is
    /// still whole.
    ///
    /// Two assertions, because the pair is the honest state of the check after
    /// ADR-037 D6: there is no program that reaches `NK2501` or `NK2502`, and
    /// the sentences those two would print are not allowed to rot while there is
    /// not. A type whose expansion still moves with `user_parallelism` is what
    /// would fill it, and that is D3's second half.
    #[test]
    fn no_type_answers_may_not_today_and_the_arm_is_still_whole() {
        let (own, library) = ledgers();
        for (name, contract) in own.types.iter().chain(library.types.iter()) {
            let answer = crossing(&Ty::named(name), &own, &library);
            assert!(answer.refused().is_none(), "`{name}`: {answer:?}");
            let _ = contract;
        }
        for text in [
            "Shared[String]",
            "Shared[Locked[i32]]",
            "Vec[Shared[i64]]",
            "Mapped",
            "?",
        ] {
            assert!(of(text).refused().is_none(), "{text}");
        }

        let refused = Crossing::MayNot {
            part: "Locked[i32]".to_string(),
            at: Some("which its field `hits` holds".to_string()),
        };
        assert_eq!(
            refused.refused(),
            Some(("Locked[i32]", Some("which its field `hits` holds")))
        );
        let note = refused.note().expect("a refusal has a note");
        assert!(note.contains("`Locked[i32]`"), "{note}");
        assert!(note.contains("field `hits`"), "{note}");
        assert!(note.contains("either setting"), "{note}");
        // Part III C.2: no Rust vocabulary in a diagnostic, ever.
        for word in ["Rc", "Arc", "Send", "E0277", "lifetime", "borrow"] {
            assert!(!note.contains(word), "`{word}` in: {note}");
        }
        assert_eq!(
            refused.way_out().as_deref(),
            Some("keep the value on the thread that built it")
        );
    }

    /// A type nothing describes is undecided, and undecided is not `May`.
    #[test]
    fn an_undescribed_type_is_undecided_and_not_permission() {
        for text in ["?", "Mapped", "Locked[i64]", "fn(&Stats)", "$V", "Vec[?]"] {
            let answer = of(text);
            assert!(
                matches!(answer, Crossing::Undecided { .. }),
                "{text}: {answer:?}"
            );
            assert!(!answer.may(), "{text}");
        }
    }

    /// A struct is its fields, through the ledger - which is what makes the
    /// rule structural as Group B requires.
    ///
    /// The field that decides is an **undecided** one since ADR-037 D6, because
    /// no type is refused any more. The walk is the same walk: what changed is
    /// which of the two non-`May` answers it finds at the bottom of it.
    #[test]
    fn a_struct_is_its_fields() {
        let (mut own, library) = ledgers();
        own.types.insert(
            "Reading".to_string(),
            TypeContract {
                fields: vec![
                    ("name".to_string(), Ty::named("String")),
                    ("temp".to_string(), Ty::named("f64")),
                ],
                ..TypeContract::default()
            },
        );
        own.types.insert(
            "Counter".to_string(),
            TypeContract {
                fields: vec![("hits".to_string(), Ty::parse("Shared[Locked[i64]]"))],
                ..TypeContract::default()
            },
        );
        // And a struct of structs, which is the transitive case.
        own.types.insert(
            "Report".to_string(),
            TypeContract {
                fields: vec![("counter".to_string(), Ty::named("Counter"))],
                ..TypeContract::default()
            },
        );

        assert_eq!(
            crossing(&Ty::named("Reading"), &own, &library),
            Crossing::May
        );
        let undecided = crossing(&Ty::named("Counter"), &own, &library);
        assert!(
            matches!(undecided, Crossing::Undecided { .. }),
            "{undecided:?}"
        );
        assert!(
            undecided
                .note()
                .expect("a non-`May` answer has a note")
                .contains("Locked[i64]"),
            "the note names the part nothing describes: {undecided:?}"
        );
        assert!(matches!(
            crossing(&Ty::named("Report"), &own, &library),
            Crossing::Undecided { .. }
        ));
    }

    /// A type that holds itself terminates, and is answered by its other
    /// fields.
    #[test]
    fn a_type_that_holds_itself_terminates() {
        let (mut own, library) = ledgers();
        own.types.insert(
            "Node".to_string(),
            TypeContract {
                fields: vec![
                    ("value".to_string(), Ty::named("i64")),
                    ("next".to_string(), Ty::parse("Option[Node]")),
                ],
                ..TypeContract::default()
            },
        );
        assert_eq!(crossing(&Ty::named("Node"), &own, &library), Crossing::May);

        own.types.insert(
            "Ring".to_string(),
            TypeContract {
                fields: vec![
                    ("held".to_string(), Ty::parse("Shared[Locked[i64]]")),
                    ("next".to_string(), Ty::parse("Option[Ring]")),
                ],
                ..TypeContract::default()
            },
        );
        assert!(matches!(
            crossing(&Ty::named("Ring"), &own, &library),
            Crossing::Undecided { .. }
        ));
    }

    /// A refusal beats an admission of ignorance, because a reader can act on
    /// it (ADR-033 D9's rule for the same kind of choice).
    ///
    /// Asked of [`join`] directly, because no type produces a refusal to put on
    /// one side of it any more (ADR-037 D6). The rule is still the rule, and the
    /// day a type needs `MayNot` again this is what decides which of two true
    /// answers gets printed.
    #[test]
    fn a_refusal_is_reported_over_an_undecided_part() {
        let refused = Crossing::MayNot {
            part: "Held".to_string(),
            at: None,
        };
        let unknown = Crossing::Undecided {
            part: "?".to_string(),
        };
        for order in [
            vec![unknown.clone(), refused.clone()],
            vec![refused.clone(), unknown.clone()],
            vec![Crossing::May, unknown.clone(), refused.clone()],
        ] {
            assert_eq!(join(order.into_iter()), refused);
        }
        // …and an undecided part still beats a `May` one.
        assert_eq!(join([Crossing::May, unknown.clone()].into_iter()), unknown);
    }

    /// `std`'s own opaque types are undecided and not refused: their fields are
    /// Rust, and an entry with no fields is "nothing is written down" rather
    /// than "it holds nothing".
    #[test]
    fn a_library_type_whose_fields_are_rust_is_undecided() {
        assert!(matches!(of("Mapped"), Crossing::Undecided { .. }));
        assert!(matches!(of("Lines"), Crossing::Undecided { .. }));
    }
}
